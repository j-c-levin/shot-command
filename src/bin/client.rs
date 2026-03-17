use bevy::{asset::AssetMetaCheck, prelude::*};
use bevy_replicon::prelude::*;
use bevy_replicon_renet::RepliconRenetPlugins;
use clap::Parser;

use nebulous_shot_command::camera::CameraPlugin;
use nebulous_shot_command::fleet::FleetPlugin;
use nebulous_shot_command::fog::FogClientPlugin;
use nebulous_shot_command::game::{GamePlugin, GameState};
use nebulous_shot_command::input::InputPlugin;
use nebulous_shot_command::net::client::{ClientConnectAddress, ClientNetPlugin};
use nebulous_shot_command::net::SharedReplicationPlugin;
use nebulous_shot_command::net::LocalTeam;
use nebulous_shot_command::radar::RadarClientPlugin;
use nebulous_shot_command::ship::ShipVisualsPlugin;
use nebulous_shot_command::ui::FleetUiPlugin;

#[derive(Parser, Debug)]
#[command(name = "nebulous-client")]
struct Cli {
    /// Server address to connect to
    #[arg(long, default_value = "127.0.0.1:5000")]
    connect: String,
}

fn main() {
    let cli = Cli::parse();

    App::new()
        .add_plugins((
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
                .set(ImagePlugin::default_nearest()),
            MeshPickingPlugin,
            RepliconPlugins,
            RepliconRenetPlugins,
            SharedReplicationPlugin,
            GamePlugin,
            FleetPlugin,
            CameraPlugin,
            ShipVisualsPlugin,
            FogClientPlugin,
            InputPlugin,
            FleetUiPlugin,
            RadarClientPlugin,
            ClientNetPlugin,
        ))
        .insert_resource(ClientConnectAddress(cli.connect))
        .init_resource::<LocalTeam>()
        .add_systems(Startup, set_connecting)
        .run();
}

/// Transition from the default Setup state to Connecting on startup.
fn set_connecting(mut next_state: ResMut<NextState<GameState>>) {
    next_state.set(GameState::Connecting);
}
