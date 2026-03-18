use std::env;

use bevy::{app::ScheduleRunnerPlugin, prelude::*, state::app::StatesPlugin};
use bevy_replicon::prelude::*;
use bevy_replicon_renet::RepliconRenetPlugins;
use clap::Parser;

use nebulous_shot_command::control_point::ControlPointPlugin;
use nebulous_shot_command::fleet::FleetPlugin;
use nebulous_shot_command::fleet::lobby::LobbyPlugin;
use nebulous_shot_command::game::{GamePlugin, GameState};
use nebulous_shot_command::net::server::{ServerBindAddress, ServerMapPath, ServerNetPlugin};
use nebulous_shot_command::net::SharedReplicationPlugin;
use nebulous_shot_command::ship::ShipPhysicsPlugin;
use nebulous_shot_command::radar::RadarPlugin;
use nebulous_shot_command::weapon::damage::DamagePlugin;
use nebulous_shot_command::weapon::missile::MissilePlugin;
use nebulous_shot_command::weapon::pd::PdPlugin;
use nebulous_shot_command::weapon::projectile::ProjectilePlugin;

#[derive(Parser, Debug)]
#[command(name = "nebulous-server")]
struct Cli {
    /// Address to bind the server to (overridden by ARBITRIUM_PORT_GAMEPORT_INTERNAL env var)
    #[arg(long, default_value = "127.0.0.1:5000")]
    bind: String,

    /// Path to a map file (RON) in assets/maps/. If omitted, uses random generation.
    #[arg(long)]
    map: Option<String>,
}

/// Resolve the bind address: Edgegap env var takes priority, then CLI arg.
fn resolve_bind_address(cli_bind: &str) -> String {
    if let Ok(port) = env::var("ARBITRIUM_PORT_GAMEPORT_INTERNAL") {
        let addr = format!("0.0.0.0:{port}");
        info!("Using Edgegap port: {addr}");
        addr
    } else {
        cli_bind.to_string()
    }
}

/// Resolve map: GAME_MAP env var (from Edgegap) takes priority, then CLI arg.
fn resolve_map(cli_map: Option<String>) -> Option<String> {
    cli_map.or_else(|| env::var("GAME_MAP").ok())
}

/// Resource holding Edgegap self-termination info, if running in Edgegap.
#[derive(Resource)]
struct EdgegapTermination {
    delete_url: String,
    delete_token: String,
}

fn main() {
    let cli = Cli::parse();
    let bind_address = resolve_bind_address(&cli.bind);

    if let Ok(request_id) = env::var("ARBITRIUM_REQUEST_ID") {
        info!("Running on Edgegap deployment: {request_id}");
    }

    let mut app = App::new();

    app.add_plugins((
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
    ))
    .add_plugins((
        RadarPlugin,
        DamagePlugin,
        ControlPointPlugin,
        ServerNetPlugin,
    ))
    .insert_resource(ServerBindAddress(bind_address))
    .insert_resource(ServerMapPath(resolve_map(cli.map)))
    .add_systems(
        OnEnter(GameState::WaitingForPlayers),
        || info!("Waiting for players..."),
    )
    .add_systems(Startup, set_waiting_for_players);

    // If running on Edgegap, insert termination resource and schedule self-destruct on GameOver.
    if let (Ok(delete_url), Ok(delete_token)) = (
        env::var("ARBITRIUM_DELETE_URL"),
        env::var("ARBITRIUM_DELETE_TOKEN"),
    ) {
        info!("Edgegap self-termination configured");
        app.insert_resource(EdgegapTermination {
            delete_url,
            delete_token,
        });
        app.add_systems(OnEnter(GameState::GameOver), edgegap_self_terminate);
    }

    app.run();
}

/// Transition from the default Setup state to WaitingForPlayers on startup.
fn set_waiting_for_players(mut next_state: ResMut<NextState<GameState>>) {
    next_state.set(GameState::WaitingForPlayers);
}

/// Request Edgegap to destroy this container. Fires on GameOver after a short delay
/// to let clients receive the GameResult.
fn edgegap_self_terminate(termination: Res<EdgegapTermination>) {
    info!("Match over — requesting Edgegap container termination");

    let url = termination.delete_url.clone();
    let token = termination.delete_token.clone();

    // Spawn a thread so we don't block the game loop waiting for HTTP.
    std::thread::spawn(move || {
        // Brief delay to let final network messages flush to clients.
        std::thread::sleep(std::time::Duration::from_secs(3));

        let client = reqwest::blocking::Client::new();
        match client
            .delete(&url)
            .header("Authorization", &token)
            .send()
        {
            Ok(resp) => info!("Edgegap termination response: {}", resp.status()),
            Err(e) => error!("Failed to request Edgegap termination: {e}"),
        }
    });
}
