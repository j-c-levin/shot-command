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
use crate::input::on_ground_clicked;
use crate::map::{Asteroid, AsteroidSize, GroundPlane, MapBounds};
use crate::net::commands::{
    ClearTargetCommand, FacingLockCommand, FacingUnlockCommand, GameResult, MoveCommand,
    TargetCommand, TeamAssignment,
};
use crate::net::{LocalTeam, PROTOCOL_ID};
use crate::ship::{
    FacingLocked, FacingTarget, Ship, ShipClass, ShipSecrets, ShipSecretsOwner, TargetDesignation,
    WaypointQueue,
};
use crate::weapon::Mounts;

/// Resource containing the server address to connect to.
#[derive(Resource, Debug, Clone)]
pub struct ClientConnectAddress(pub String);

pub struct ClientNetPlugin;

impl Plugin for ClientNetPlugin {
    fn build(&self, app: &mut App) {
        // Register replicated components (must mirror server exactly, minus server-only ones)
        // NOTE: WaypointQueue, FacingTarget, FacingLocked are NOT replicated on Ship entities.
        // They arrive via ShipSecrets child entities with per-team visibility.
        app.replicate::<Ship>()
            .replicate::<ShipClass>()
            .replicate::<Team>()
            .replicate::<Transform>()
            .replicate::<Health>()
            .replicate::<Mounts>()
            .replicate::<Asteroid>()
            .replicate::<AsteroidSize>();

        // ShipSecrets child entity components (team-private state)
        app.replicate::<ShipSecrets>()
            .replicate::<ShipSecretsOwner>()
            .replicate::<WaypointQueue>()
            .replicate::<FacingTarget>()
            .replicate::<FacingLocked>()
            .replicate::<TargetDesignation>();

        // Register client→server triggers (same types as server)
        app.add_mapped_client_event::<MoveCommand>(Channel::Ordered)
            .add_mapped_client_event::<FacingLockCommand>(Channel::Ordered)
            .add_mapped_client_event::<FacingUnlockCommand>(Channel::Ordered)
            .add_mapped_client_event::<TargetCommand>(Channel::Ordered)
            .add_mapped_client_event::<ClearTargetCommand>(Channel::Ordered);

        // Register server→client triggers
        app.add_server_event::<TeamAssignment>(Channel::Ordered);
        app.add_server_event::<GameResult>(Channel::Ordered);

        // Systems
        app.add_systems(OnEnter(GameState::Connecting), setup_renet_client);
        app.add_systems(OnEnter(GameState::Playing), client_setup_scene);
        app.add_systems(
            Update,
            (
                super::materializer::materialize_ships,
                super::materializer::materialize_asteroids,
            )
                .run_if(in_state(GameState::Playing)),
        );

        // Observer for team assignment from server
        app.add_observer(on_team_assignment);

        // Observer for game result from server
        app.add_observer(on_game_result);

        // Game over UI
        app.add_systems(OnEnter(GameState::GameOver), show_game_over_ui);
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

/// Sets up the client scene when entering Playing state:
/// ground plane (for click-to-move picking) and MapBounds resource.
/// Note: Camera, ambient light, and directional light are handled by CameraPlugin.
fn client_setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Insert MapBounds resource (same values as MapPlugin)
    let bounds = MapBounds {
        half_extents: Vec2::splat(500.0),
    };
    commands.insert_resource(bounds.clone());

    // Ground plane for click-to-move picking
    let size = bounds.size();
    commands.spawn((
        GroundPlane,
        Mesh3d(meshes.add(Plane3d::new(Vec3::Y, Vec2::new(size.x / 2.0, size.y / 2.0)))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.02, 0.02, 0.05),
            perceptual_roughness: 1.0,
            ..default()
        })),
        Transform::from_xyz(0.0, 0.0, 0.0),
        Pickable::default(),
    ));

    // Register the global ground-click observer for move commands
    commands.add_observer(on_ground_clicked);

    info!("Client scene setup complete (ground plane + map bounds + observers)");
}

/// Stores the game outcome once the server announces it.
#[derive(Resource, Debug)]
pub struct GameOutcome(pub Team);

/// Observer that fires when the server sends a GameResult event.
fn on_game_result(
    trigger: On<GameResult>,
    mut commands: Commands,
    mut next_state: ResMut<NextState<GameState>>,
) {
    let result = &*trigger;
    info!("Game result received: Team {} wins!", result.winning_team.0);
    commands.insert_resource(GameOutcome(result.winning_team));
    next_state.set(GameState::GameOver);
}

/// Display Victory/Defeat UI text when entering GameOver state.
fn show_game_over_ui(
    mut commands: Commands,
    game_outcome: Option<Res<GameOutcome>>,
    local_team: Res<LocalTeam>,
) {
    let Some(outcome) = game_outcome else {
        warn!("GameOver entered but no GameOutcome resource found");
        return;
    };

    let is_victory = local_team
        .0
        .map(|t| t == outcome.0)
        .unwrap_or(false);

    let (text, color) = if is_victory {
        ("Victory!", Color::srgb(0.2, 1.0, 0.2))
    } else {
        ("Defeat", Color::srgb(1.0, 0.2, 0.2))
    };

    commands.spawn((
        Text::new(text),
        TextFont {
            font_size: 48.0,
            ..default()
        },
        TextColor(color),
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
    ));

    info!("Game over: {}", text);
}

