use bevy::prelude::*;
use bevy::time::Timer;
use serde::{Deserialize, Serialize};

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
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Team(pub u8);

impl Team {
    pub const PLAYER: Self = Self(0);
    pub const ENEMY: Self = Self(1);
}

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
}
