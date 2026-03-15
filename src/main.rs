// Disable console on Windows for non-dev builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use bevy::log::tracing_subscriber::{field::MakeExt, fmt};
use bevy::{app::App, asset::AssetMetaCheck, ecs::error::error, log, prelude::*};

use nebulous_shot_command::game::{GameState, Team};
use nebulous_shot_command::input::{on_ground_clicked, on_ship_clicked};
use nebulous_shot_command::ship::{spawn_ship, ShipClass};
use nebulous_shot_command::{camera, fog, game, input, map, ship};

const NAME: &str = env!("CARGO_PKG_NAME");

fn main() {
    let mut app = App::new();
    app.set_error_handler(error);

    app.add_plugins((
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Nebulous Shot Command".to_string(),
                    fit_canvas_to_parent: true,
                    ..default()
                }),
                ..default()
            })
            .set(AssetPlugin {
                meta_check: AssetMetaCheck::Never,
                ..default()
            })
            .set(log::LogPlugin {
                level: log::Level::TRACE,
                filter: format!(
                    "info,{}=debug,bevy_math=error,{},",
                    NAME.replace("-", "_"),
                    bevy::log::DEFAULT_FILTER
                ),
                fmt_layer: |_| {
                    Some(Box::new(
                        fmt::Layer::default()
                            .without_time()
                            .map_fmt_fields(MakeExt::debug_alt)
                            .with_writer(std::io::stderr),
                    ))
                },
                ..default()
            })
            .set(ImagePlugin::default_nearest()),
        MeshPickingPlugin,
    ));

    info!("Starting {}", NAME);

    app.add_plugins((
        game::GamePlugin,
        map::MapPlugin,
        ship::ShipPlugin,
        camera::CameraPlugin,
        input::InputPlugin,
        fog::FogPlugin,
    ));

    app.add_systems(OnEnter(GameState::Setup), setup_game);

    app.run();
}

fn setup_game(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    commands.add_observer(on_ground_clicked);

    let player_color = Color::srgb(0.2, 0.6, 1.0);

    // Player fleet near bottom-left corner
    let battleship = spawn_ship(
        &mut commands, &mut meshes, &mut materials,
        Vec2::new(-300.0, -300.0), Team::PLAYER, player_color, ShipClass::Battleship,
    );
    commands.entity(battleship).observe(on_ship_clicked);

    let destroyer = spawn_ship(
        &mut commands, &mut meshes, &mut materials,
        Vec2::new(-330.0, -260.0), Team::PLAYER, player_color, ShipClass::Destroyer,
    );
    commands.entity(destroyer).observe(on_ship_clicked);

    let scout = spawn_ship(
        &mut commands, &mut meshes, &mut materials,
        Vec2::new(-280.0, -330.0), Team::PLAYER, player_color, ShipClass::Scout,
    );
    commands.entity(scout).observe(on_ship_clicked);

    // Enemy ships scattered around the map
    let enemy_color = Color::srgb(1.0, 0.2, 0.2);
    let enemy_positions = [
        (Vec2::new(300.0, 300.0), ShipClass::Battleship),
        (Vec2::new(-200.0, 350.0), ShipClass::Destroyer),
        (Vec2::new(350.0, -100.0), ShipClass::Destroyer),
        (Vec2::new(0.0, 300.0), ShipClass::Scout),
        (Vec2::new(250.0, -300.0), ShipClass::Scout),
    ];

    for (pos, class) in enemy_positions {
        let enemy = spawn_ship(
            &mut commands, &mut meshes, &mut materials,
            pos, Team::ENEMY, enemy_color, class,
        );
        commands.entity(enemy).observe(on_ship_clicked);
    }

    next_state.set(GameState::Playing);
    info!("Game setup complete — entering Playing state");
}
