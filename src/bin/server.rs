use bevy::{app::ScheduleRunnerPlugin, prelude::*, state::app::StatesPlugin};
use bevy_replicon::prelude::*;
use bevy_replicon_renet::RepliconRenetPlugins;
use clap::Parser;

use nebulous_shot_command::fleet::FleetPlugin;
use nebulous_shot_command::fleet::lobby::LobbyPlugin;
use nebulous_shot_command::game::{GamePlugin, GameState};
use nebulous_shot_command::net::server::{ServerBindAddress, ServerNetPlugin};
use nebulous_shot_command::net::SharedReplicationPlugin;
use nebulous_shot_command::ship::ShipPhysicsPlugin;
use nebulous_shot_command::weapon::damage::DamagePlugin;
use nebulous_shot_command::weapon::missile::MissilePlugin;
use nebulous_shot_command::weapon::pd::PdPlugin;
use nebulous_shot_command::weapon::projectile::ProjectilePlugin;

#[derive(Parser, Debug)]
#[command(name = "nebulous-server")]
struct Cli {
    /// Address to bind the server to
    #[arg(long, default_value = "127.0.0.1:5000")]
    bind: String,
}

fn main() {
    let cli = Cli::parse();

    App::new()
        .add_plugins((
            MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(
                std::time::Duration::from_secs_f64(1.0 / 60.0),
            )),
            StatesPlugin,
            bevy::log::LogPlugin::default(),
            RepliconPlugins,
            RepliconRenetPlugins,
            SharedReplicationPlugin,
            GamePlugin,
            FleetPlugin,
            LobbyPlugin,
            ShipPhysicsPlugin,
            ProjectilePlugin,
            MissilePlugin,
            PdPlugin,
            DamagePlugin,
            ServerNetPlugin,
        ))
        .insert_resource(ServerBindAddress(cli.bind))
        .add_systems(
            OnEnter(GameState::WaitingForPlayers),
            || info!("Waiting for players..."),
        )
        .add_systems(Startup, set_waiting_for_players)
        .run();
}

/// Transition from the default Setup state to WaitingForPlayers on startup.
fn set_waiting_for_players(mut next_state: ResMut<NextState<GameState>>) {
    next_state.set(GameState::WaitingForPlayers);
}
