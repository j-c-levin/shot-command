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
    /// Client: main menu — browse/create/join games
    MainMenu,
    /// Client: in a game lobby — see players, build fleet, wait for launch
    GameLobby,
    /// Server: waiting for all clients to connect
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
    /// Array index for this team (for score `HashMap`s, etc.).
    pub fn index(&self) -> usize {
        self.0 as usize
    }

    /// Whether this team matches the local player's team.
    pub fn is_friendly(&self, local: &crate::net::LocalTeam) -> bool {
        local.0.map(|lt| lt == *self).unwrap_or(false)
    }

    /// Display color for this team (up to 4 teams).
    pub fn color(&self) -> Color {
        match self.0 {
            0 => Color::srgb(0.2, 0.6, 1.0),  // Blue
            1 => Color::srgb(1.0, 0.2, 0.2),  // Red
            2 => Color::srgb(0.2, 1.0, 0.3),  // Green
            3 => Color::srgb(1.0, 0.8, 0.1),  // Yellow
            _ => Color::srgb(0.5, 0.5, 0.5),  // Gray fallback
        }
    }
}

/// Configuration for the current game's team/player structure.
/// Inserted by server at startup; replicated to clients.
#[derive(Resource, Clone, Debug, Serialize, Deserialize)]
pub struct GameConfig {
    pub team_count: u8,
    pub players_per_team: u8,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self { team_count: 2, players_per_team: 1 }
    }
}

impl GameConfig {
    pub fn max_players(&self) -> usize {
        self.team_count as usize * self.players_per_team as usize
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
    fn game_config_default() {
        let config = GameConfig::default();
        assert_eq!(config.team_count, 2);
        assert_eq!(config.players_per_team, 1);
    }

    #[test]
    fn game_config_max_players() {
        let config = GameConfig { team_count: 3, players_per_team: 2 };
        assert_eq!(config.max_players(), 6);
    }

    #[test]
    fn team_color_distinct() {
        let colors: Vec<Color> = (0..4).map(|i| Team(i).color()).collect();
        for i in 0..colors.len() {
            for j in (i + 1)..colors.len() {
                assert_ne!(colors[i], colors[j], "Team {} and {} should have different colors", i, j);
            }
        }
    }

    #[test]
    fn default_game_state_is_setup() {
        assert_eq!(GameState::default(), GameState::Setup);
    }

    #[test]
    fn game_states_are_distinct() {
        let states = [
            GameState::Setup,
            GameState::MainMenu,
            GameState::GameLobby,
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
    fn team_index() {
        assert_eq!(Team(0).index(), 0);
        assert_eq!(Team(1).index(), 1);
    }
}
