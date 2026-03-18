use bevy::prelude::*;
use bevy::time::Timer;
use serde::{Deserialize, Serialize};

/// Despawn an entity via direct World access. Safe if entity is already gone —
/// `World::despawn` returns `false` instead of panicking. Use this instead of
/// `commands.entity(e).despawn()` when multiple systems might race to despawn
/// the same entity in a single frame (e.g. CWIS + missile collision).
pub fn try_despawn(commands: &mut Commands, entity: Entity) {
    commands.queue(move |world: &mut World| {
        world.despawn(entity);
    });
}

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>();
    }
}

#[derive(Clone, Copy, Default, Eq, PartialEq, Debug, Hash, States, Serialize, Deserialize)]
pub enum GameState {
    #[default]
    Setup,
    /// Server: waiting for both clients to connect
    WaitingForPlayers,
    /// Client: connecting to server, waiting for team assignment
    Connecting,
    /// Both: fleet composition screen (loadout selection before game starts)
    FleetComposition,
    Playing,
    GameOver,
    /// Map editor mode (standalone, no server/client networking)
    Editor,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Team(pub u8);

impl Team {
    pub const PLAYER: Self = Self(0);
    pub const ENEMY: Self = Self(1);

    /// The opposing team (assumes 2-team game).
    pub fn opponent(&self) -> Self {
        Team(1 - self.0)
    }

    /// Array index for this team (for `[f32; 2]` score arrays, etc.).
    pub fn index(&self) -> usize {
        self.0 as usize
    }

    /// Whether this team matches the local player's team.
    pub fn is_friendly(&self, local: &crate::net::LocalTeam) -> bool {
        local.0.map(|lt| lt == *self).unwrap_or(false)
    }
}

/// Identifies which connected client owns/controls a ship.
/// Same-team players can see each other's ships but only command their own.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Player(pub Entity);

impl bevy::ecs::entity::MapEntities for Player {
    fn map_entities<M: bevy::ecs::entity::EntityMapper>(&mut self, mapper: &mut M) {
        self.0 = mapper.get_mapped(self.0);
    }
}

/// Marker: this ship's engines are offline (hp == 0, timer counting down).
/// Inserted by damage system, removed by repair system.
#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct EngineOffline;

/// Marker: this enemy is currently in line of sight of a player ship.
#[derive(Component, Clone, Debug, Default)]
pub struct Detected;

/// Tracks fade opacity for enemy ships. Spawned on all enemies.
#[derive(Component, Clone, Debug)]
pub struct EnemyVisibility {
    pub opacity: f32,
}

impl Default for EnemyVisibility {
    fn default() -> Self {
        Self { opacity: 0.0 }
    }
}

#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct Health {
    pub hp: u16,
}

/// Marker: this ship has been destroyed (hp reached 0). Server-only.
#[derive(Component, Clone, Debug)]
pub struct Destroyed;

/// Timer that delays despawn after destruction. Server-only.
#[derive(Component, Clone, Debug)]
pub struct DestroyTimer(pub Timer);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn team_constants_are_distinct() {
        assert_ne!(Team::PLAYER, Team::ENEMY);
    }

    #[test]
    fn team_equality() {
        assert_eq!(Team(0), Team::PLAYER);
        assert_eq!(Team(1), Team::ENEMY);
    }

    #[test]
    fn default_game_state_is_setup() {
        assert_eq!(GameState::default(), GameState::Setup);
    }

    #[test]
    fn game_states_are_distinct() {
        let states = [
            GameState::Setup,
            GameState::WaitingForPlayers,
            GameState::Connecting,
            GameState::FleetComposition,
            GameState::Playing,
            GameState::GameOver,
            GameState::Editor,
        ];
        for (i, a) in states.iter().enumerate() {
            for (j, b) in states.iter().enumerate() {
                if i != j {
                    assert_ne!(a, b);
                }
            }
        }
    }

    #[test]
    fn game_state_has_fleet_composition() {
        // FleetComposition is a distinct state between Connecting and Playing
        let fc = GameState::FleetComposition;
        assert_ne!(fc, GameState::Connecting);
        assert_ne!(fc, GameState::Playing);
        assert_ne!(fc, GameState::Setup);
    }

    #[test]
    fn enemy_visibility_defaults_to_zero_opacity() {
        let ev = EnemyVisibility::default();
        assert_eq!(ev.opacity, 0.0);
    }

    #[test]
    fn health_takes_damage() {
        let mut h = Health { hp: 200 };
        h.hp -= 15;
        assert_eq!(h.hp, 185);
    }

    #[test]
    fn health_saturates_at_zero() {
        let h = Health { hp: 0u16 };
        assert_eq!(h.hp.saturating_sub(1), 0);
    }

    #[test]
    fn team_opponent() {
        assert_eq!(Team(0).opponent(), Team(1));
        assert_eq!(Team(1).opponent(), Team(0));
    }

    #[test]
    fn team_index() {
        assert_eq!(Team(0).index(), 0);
        assert_eq!(Team(1).index(), 1);
    }
}
