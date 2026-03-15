use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::time::SystemTime;

use bevy::prelude::*;
use bevy_replicon::prelude::*;
use bevy_replicon_renet::{
    RenetChannelsExt, RenetClient,
    netcode::{ClientAuthentication, NetcodeClientTransport},
    renet::ConnectionConfig,
};

use crate::game::{GameState, Health, Team};
use crate::net::commands::{
    FacingLockCommand, FacingUnlockCommand, MoveCommand, TeamAssignment,
};
use crate::net::{LocalTeam, PROTOCOL_ID};
use crate::ship::{FacingLocked, FacingTarget, Ship, ShipClass, Velocity, WaypointQueue};

/// Resource containing the server address to connect to.
#[derive(Resource, Debug, Clone)]
pub struct ClientConnectAddress(pub String);

pub struct ClientNetPlugin;

impl Plugin for ClientNetPlugin {
    fn build(&self, app: &mut App) {
        // Register replicated components (must mirror server exactly)
        app.replicate::<Ship>()
            .replicate::<ShipClass>()
            .replicate::<Team>()
            .replicate::<Transform>()
            .replicate::<Velocity>()
            .replicate::<WaypointQueue>()
            .replicate::<FacingTarget>()
            .replicate::<FacingLocked>()
            .replicate::<Health>();

        // Register client→server triggers (same types as server)
        app.add_mapped_client_event::<MoveCommand>(Channel::Ordered)
            .add_mapped_client_event::<FacingLockCommand>(Channel::Ordered)
            .add_mapped_client_event::<FacingUnlockCommand>(Channel::Ordered);

        // Register server→client trigger
        app.add_server_event::<TeamAssignment>(Channel::Ordered);

        // Systems
        app.add_systems(OnEnter(GameState::Connecting), setup_renet_client);

        // Observer for team assignment from server
        app.add_observer(on_team_assignment);
    }
}

/// Sets up the renet client and transport when entering Connecting state.
fn setup_renet_client(
    mut commands: Commands,
    channels: Res<RepliconChannels>,
    connect_address: Res<ClientConnectAddress>,
) {
    let server_addr: SocketAddr = connect_address
        .0
        .parse()
        .expect("Invalid server address format");

    let server_channels_config = channels.server_configs();
    let client_channels_config = channels.client_configs();

    let client = RenetClient::new(ConnectionConfig {
        server_channels_config,
        client_channels_config,
        ..Default::default()
    });

    let current_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();

    // Use current time as a unique client ID
    let client_id = current_time.as_millis() as u64;

    let socket =
        UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).expect("Failed to bind client UDP socket");

    let authentication = ClientAuthentication::Unsecure {
        client_id,
        protocol_id: PROTOCOL_ID,
        server_addr,
        user_data: None,
    };

    let transport = NetcodeClientTransport::new(current_time, authentication, socket)
        .expect("Failed to create client transport");

    commands.insert_resource(client);
    commands.insert_resource(transport);

    info!("Client connecting to {}...", server_addr);
}

/// Observer that fires when the server sends a TeamAssignment event.
fn on_team_assignment(
    trigger: On<TeamAssignment>,
    mut local_team: ResMut<LocalTeam>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    let assignment = &*trigger;
    let team = assignment.team;

    info!("Received team assignment: Team({})", team.0);
    local_team.0 = Some(team);

    next_state.set(GameState::Playing);
    info!("Transitioning to Playing state");
}
