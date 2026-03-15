use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::time::SystemTime;

use bevy::prelude::*;
use bevy_replicon::prelude::*;
use bevy_replicon_renet::{
    RenetChannelsExt, RenetServer,
    netcode::{NetcodeServerTransport, ServerAuthentication, ServerConfig as NetcodeServerConfig},
    renet::ConnectionConfig,
};

use crate::game::{GameState, Health, Team};
use crate::map::MapBounds;
use crate::net::commands::{
    FacingLockCommand, FacingUnlockCommand, MoveCommand, TeamAssignment,
};
use crate::ship::{
    FacingLocked, FacingTarget, Ship, ShipClass, Velocity, WaypointQueue, spawn_server_ship,
};

/// Protocol ID for our game -- used to reject connections from other applications.
const PROTOCOL_ID: u64 = 0x4E45_4255_4C41_0001; // "NEBULA" + version

/// Resource containing the bind address string, inserted before the plugin runs.
#[derive(Resource, Debug, Clone)]
pub struct ServerBindAddress(pub String);

/// Maps connected client entities to their assigned team.
#[derive(Resource, Debug, Default)]
pub struct ClientTeams {
    pub map: HashMap<Entity, Team>,
}

pub struct ServerNetPlugin;

impl Plugin for ServerNetPlugin {
    fn build(&self, app: &mut App) {
        // Register replicated components
        app.replicate::<Ship>()
            .replicate::<ShipClass>()
            .replicate::<Team>()
            .replicate::<Transform>()
            .replicate::<Velocity>()
            .replicate::<WaypointQueue>()
            .replicate::<FacingTarget>()
            .replicate::<FacingLocked>()
            .replicate::<Health>();

        // Register client→server triggers (events with entity mapping)
        app.add_mapped_client_event::<MoveCommand>(Channel::Ordered)
            .add_mapped_client_event::<FacingLockCommand>(Channel::Ordered)
            .add_mapped_client_event::<FacingUnlockCommand>(Channel::Ordered);

        // Register server→client trigger
        app.add_server_event::<TeamAssignment>(Channel::Ordered);

        // Init resources
        app.init_resource::<ClientTeams>();

        // Systems
        app.add_systems(
            OnEnter(GameState::WaitingForPlayers),
            setup_renet_server,
        );

        // Server game setup: spawn fleets when entering Playing state
        app.add_systems(OnEnter(GameState::Playing), server_setup_game);

        // Observer for new client connections
        app.add_observer(on_client_connected);
    }
}

/// Sets up the renet server and transport when entering WaitingForPlayers state.
fn setup_renet_server(
    mut commands: Commands,
    channels: Res<RepliconChannels>,
    bind_address: Res<ServerBindAddress>,
) {
    let addr: SocketAddr = bind_address
        .0
        .parse()
        .expect("Invalid bind address format");

    let server_channels_config = channels.server_configs();
    let client_channels_config = channels.client_configs();

    let server = RenetServer::new(ConnectionConfig {
        server_channels_config,
        client_channels_config,
        ..Default::default()
    });

    let current_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();

    let server_config = NetcodeServerConfig {
        current_time,
        max_clients: 2,
        protocol_id: PROTOCOL_ID,
        public_addresses: vec![addr],
        authentication: ServerAuthentication::Unsecure,
    };

    let socket = UdpSocket::bind(addr).expect("Failed to bind UDP socket");
    let transport =
        NetcodeServerTransport::new(server_config, socket).expect("Failed to create transport");

    commands.insert_resource(server);
    commands.insert_resource(transport);

    info!("Server listening on {}", addr);
}

/// Observer that fires when a new ConnectedClient component is added to an entity.
fn on_client_connected(
    trigger: On<Add, ConnectedClient>,
    mut commands: Commands,
    mut client_teams: ResMut<ClientTeams>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    let client_entity = trigger.entity;
    let team_id = client_teams.map.len() as u8;
    let team = Team(team_id);

    client_teams.map.insert(client_entity, team);

    info!(
        "Client {:?} connected, assigned Team({}). Total clients: {}",
        client_entity,
        team_id,
        client_teams.map.len()
    );

    // Send team assignment to the newly connected client
    commands.server_trigger(ToClients {
        mode: SendMode::Direct(ClientId::Client(client_entity)),
        message: TeamAssignment { team },
    });

    // After 2 clients connected, transition to Playing
    if client_teams.map.len() >= 2 {
        info!("Both players connected, transitioning to Playing");
        next_state.set(GameState::Playing);
    }
}

/// Spawn symmetric fleets for each team when entering Playing state.
/// Also inserts MapBounds so physics systems can read it.
fn server_setup_game(mut commands: Commands) {
    // Insert MapBounds resource (server doesn't use MapPlugin which spawns visual elements)
    commands.insert_resource(MapBounds {
        half_extents: Vec2::splat(500.0),
    });

    // Team 0 fleet near (-300, -300)
    let team0 = Team(0);
    let team0_offsets = [
        (Vec2::new(-300.0, -300.0), ShipClass::Battleship),
        (Vec2::new(-270.0, -280.0), ShipClass::Destroyer),
        (Vec2::new(-330.0, -280.0), ShipClass::Scout),
    ];

    for (pos, class) in &team0_offsets {
        let entity = spawn_server_ship(&mut commands, *pos, team0, *class);
        info!("Spawned {:?} for Team 0: {:?}", class, entity);
    }

    // Team 1 fleet mirrored near (300, 300)
    let team1 = Team(1);
    let team1_offsets = [
        (Vec2::new(300.0, 300.0), ShipClass::Battleship),
        (Vec2::new(270.0, 280.0), ShipClass::Destroyer),
        (Vec2::new(330.0, 280.0), ShipClass::Scout),
    ];

    for (pos, class) in &team1_offsets {
        let entity = spawn_server_ship(&mut commands, *pos, team1, *class);
        info!("Spawned {:?} for Team 1: {:?}", class, entity);
    }

    info!("Server: spawned symmetric fleets for 2 teams");
}
