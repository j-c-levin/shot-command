use bevy::prelude::*;

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

#[derive(Component)]
pub struct Revealed;

fn check_victory(
    query: Query<&Team, With<Revealed>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    for team in &query {
        if *team == Team::ENEMY {
            next_state.set(GameState::Victory);
            return;
        }
    }
}

fn spawn_victory_ui(mut commands: Commands) {
    commands.spawn((
        Text::new("ENEMY LOCATED — VICTORY!"),
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
}
