use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::time::SystemTime;

use bevy::prelude::*;
use bevy_replicon::prelude::*;
use bevy_replicon::shared::replication::registry::{
    ReplicationRegistry,
    ctx::DespawnCtx,
};
use bevy_replicon_renet::{
    RenetChannelsExt, RenetClient,
    netcode::{ClientAuthentication, NetcodeClientTransport},
    renet::ConnectionConfig,
};

use crate::fog::FadingOut;
use crate::game::{EnemyVisibility, GameState, Health, Team};
use crate::input::on_ground_clicked;
use crate::map::{Asteroid, AsteroidSize, GroundPlane, MapBounds};
use crate::net::commands::{
    FacingLockCommand, FacingUnlockCommand, MoveCommand, TeamAssignment,
};
use crate::net::{LocalTeam, PROTOCOL_ID};
use crate::ship::{
    FacingLocked, FacingTarget, Ship, ShipClass, ShipSecrets, ShipSecretsOwner, WaypointQueue,
};

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
            .replicate::<Asteroid>()
            .replicate::<AsteroidSize>();

        // ShipSecrets child entity components (team-private state)
        app.replicate::<ShipSecrets>()
            .replicate::<ShipSecretsOwner>()
            .replicate::<WaypointQueue>()
            .replicate::<FacingTarget>()
            .replicate::<FacingLocked>();

        // Register client→server triggers (same types as server)
        app.add_mapped_client_event::<MoveCommand>(Channel::Ordered)
            .add_mapped_client_event::<FacingLockCommand>(Channel::Ordered)
            .add_mapped_client_event::<FacingUnlockCommand>(Channel::Ordered);

        // Register server→client trigger
        app.add_server_event::<TeamAssignment>(Channel::Ordered);

        // Override replicon's despawn function to fade out ships instead of instant removal
        let mut registry = app.world_mut().resource_mut::<ReplicationRegistry>();
        registry.despawn = custom_replicon_despawn;

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

/// Custom despawn function for bevy_replicon's [`ReplicationRegistry`].
///
/// Instead of immediately despawning ship entities when the server removes visibility,
/// inserts a [`FadingOut`] marker so `fade_client_enemies` can fade the ship out over
/// `FADE_DURATION` before actually despawning it. Non-ship entities are despawned normally.
fn custom_replicon_despawn(_ctx: &DespawnCtx, mut entity: EntityWorldMut) {
    if entity.contains::<Ship>() {
        // Insert FadingOut marker; ensure EnemyVisibility exists so fade system works
        // even if the entity was still waiting to be tagged.
        if !entity.contains::<EnemyVisibility>() {
            entity.insert(EnemyVisibility { opacity: 1.0 });
        }
        entity.insert(FadingOut);
        // Remove Replicated so replicon no longer tracks this entity
        entity.remove::<Replicated>();
    } else {
        entity.despawn();
    }
}
