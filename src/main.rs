// Disable console on Windows for non-dev builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use bevy::log::tracing_subscriber::{field::MakeExt, fmt};
use bevy::{app::App, asset::AssetMetaCheck, ecs::error::error, log, prelude::*};

use game::{GameState, Team};
use input::{on_ground_clicked, on_ship_clicked};
use ship::spawn_ship;

mod camera;
mod fog;
mod game;
mod input;
mod map;
mod ship;

const NAME: &str = env!("CARGO_PKG_NAME");

fn main() {
    let mut app = App::new();
    app.set_error_handler(error);

    app.add_plugins(
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
    );

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
    ground_query: Query<Entity, With<map::GroundPlane>>,
) {
    // Attach ground plane click observer for move commands
    for ground in &ground_query {
        commands.entity(ground).observe(on_ground_clicked);
    }
    // Spawn player ship at one corner
    let player = spawn_ship(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec2::new(-350.0, -350.0),
        Team::PLAYER,
        Color::srgb(0.2, 0.6, 1.0),
    );
    // Attach picking observers to player ship
    commands
        .entity(player)
        .observe(on_ship_clicked);

    // Spawn enemy ship at opposite corner (stationary)
    let enemy = spawn_ship(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec2::new(350.0, 350.0),
        Team::ENEMY,
        Color::srgb(1.0, 0.2, 0.2),
    );
    // Enemy also needs click observer for selection feedback
    commands
        .entity(enemy)
        .observe(on_ship_clicked);

    next_state.set(GameState::Playing);
    info!("Game setup complete — entering Playing state");
}
