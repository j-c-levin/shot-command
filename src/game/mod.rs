use bevy::prelude::*;

use crate::ship::Ship;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>()
            .add_systems(
                Update,
                check_victory.run_if(in_state(GameState::Playing)),
            )
            .add_systems(OnEnter(GameState::Victory), spawn_victory_ui);
    }
}

#[derive(Clone, Copy, Default, Eq, PartialEq, Debug, Hash, States)]
pub enum GameState {
    #[default]
    Setup,
    Playing,
    Victory,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
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

#[derive(Component, Clone, Debug)]
pub struct Health {
    pub hp: u8,
}

fn check_victory(
    query: Query<&Team, With<Ship>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    // Don't trigger victory if no ships exist yet (commands not flushed)
    if query.is_empty() {
        return;
    }
    let any_enemy_alive = query.iter().any(|team| *team == Team::ENEMY);
    if !any_enemy_alive {
        next_state.set(GameState::Victory);
    }
}

fn spawn_victory_ui(mut commands: Commands) {
    commands.spawn((
        Text::new("ENEMY DESTROYED — VICTORY!"),
        TextFont {
            font_size: 60.0,
            ..default()
        },
        TextColor(Color::srgb(0.2, 1.0, 0.3)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Percent(40.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        },
    ));
}

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
    fn enemy_visibility_defaults_to_zero_opacity() {
        let ev = EnemyVisibility::default();
        assert_eq!(ev.opacity, 0.0);
    }

    #[test]
    fn health_takes_damage() {
        let mut h = Health { hp: 3 };
        h.hp -= 1;
        assert_eq!(h.hp, 2);
    }

    #[test]
    fn health_saturates_at_zero() {
        let h = Health { hp: 0 };
        assert_eq!(h.hp.saturating_sub(1), 0);
    }
}
