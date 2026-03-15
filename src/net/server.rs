use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::time::SystemTime;

use bevy::prelude::*;
use rand::Rng;
use bevy_replicon::prelude::*;
use bevy_replicon::server::visibility::{
    client_visibility::ClientVisibility,
    filters_mask::FilterBit,
    registry::FilterRegistry,
};
use bevy_replicon::shared::message::client_message::FromClient;
use bevy_replicon::shared::replication::registry::ReplicationRegistry;
use bevy_replicon_renet::{
    RenetChannelsExt, RenetServer,
    netcode::{NetcodeServerTransport, ServerAuthentication, ServerConfig as NetcodeServerConfig},
    renet::ConnectionConfig,
};

use crate::fog::is_in_los;
use crate::game::{GameState, Health, Team};
use crate::map::{Asteroid, AsteroidSize, MapBounds};
use crate::net::commands::{
    FacingLockCommand, FacingUnlockCommand, MoveCommand, TeamAssignment,
};
use crate::net::PROTOCOL_ID;
use crate::ship::{
    FacingLocked, FacingTarget, Ship, ShipClass, ShipSecrets, ShipSecretsOwner, WaypointQueue,
    ship_xz_position, spawn_server_ship,
};
use crate::weapon::Mounts;

/// Resource containing the bind address string, inserted before the plugin runs.
#[derive(Resource, Debug, Clone)]
pub struct ServerBindAddress(pub String);

/// Maps connected client entities to their assigned team.
#[derive(Resource, Debug, Default)]
pub struct ClientTeams {
    pub map: HashMap<Entity, Team>,
}

/// A [`FilterBit`] for entity-level LOS visibility.
///
/// Registered via [`FilterRegistry::register_scope`] so we can manually call
/// [`ClientVisibility::set`] each frame based on line-of-sight calculations.
#[derive(Resource, Deref)]
pub struct LosBit(FilterBit);

impl FromWorld for LosBit {
    fn from_world(world: &mut World) -> Self {
        let bit = world.resource_scope(|world, mut filter_registry: Mut<FilterRegistry>| {
            world.resource_scope(|world, mut registry: Mut<ReplicationRegistry>| {
                filter_registry.register_scope::<Entity>(world, &mut registry)
            })
        });
        Self(bit)
    }
}

pub struct ServerNetPlugin;

impl Plugin for ServerNetPlugin {
    fn build(&self, app: &mut App) {
        // Register replicated components (Velocity is server-only, not replicated)
        // NOTE: WaypointQueue, FacingTarget, FacingLocked are NOT replicated on Ship entities.
        // They are replicated via ShipSecrets child entities with per-team visibility.
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
            .replicate::<FacingLocked>();

        // Register client→server triggers (events with entity mapping)
        app.add_mapped_client_event::<MoveCommand>(Channel::Ordered)
            .add_mapped_client_event::<FacingLockCommand>(Channel::Ordered)
            .add_mapped_client_event::<FacingUnlockCommand>(Channel::Ordered);

        // Register server→client trigger
        app.add_server_event::<TeamAssignment>(Channel::Ordered);

        // Init resources
        app.init_resource::<ClientTeams>();
        app.init_resource::<LosBit>();

        // Systems
        app.add_systems(
            OnEnter(GameState::WaitingForPlayers),
            setup_renet_server,
        );

        // Server game setup: spawn fleets when entering Playing state
        app.add_systems(OnEnter(GameState::Playing), server_setup_game);

        // Sync ship state to ShipSecrets children, then update visibility
        app.add_systems(
            Update,
            (sync_ship_secrets, server_update_visibility)
                .chain()
                .run_if(in_state(GameState::Playing)),
        );

        // Observer for new client connections
        app.add_observer(on_client_connected);

        // Command handler observers
        app.add_observer(handle_move_command);
        app.add_observer(handle_facing_lock_command);
        app.add_observer(handle_facing_unlock_command);

        // Disconnection observer
        app.add_observer(on_client_disconnected);
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

/// Observer that fires when a client is authorized (protocol check passed).
/// We use `AuthorizedClient` instead of `ConnectedClient` because the client
/// can only receive messages and replication after authorization.
fn on_client_connected(
    trigger: On<Add, AuthorizedClient>,
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

    // Spawn asteroids (data-only, no mesh — clients materialize visuals)
    let bounds = MapBounds {
        half_extents: Vec2::splat(500.0),
    };
    let mut rng = rand::rng();
    let asteroid_count = 12;
    let min_distance_from_edge = 50.0;
    let min_distance_from_center = 100.0;

    for _ in 0..asteroid_count {
        let radius = rng.random_range(15.0..40.0);

        let pos = loop {
            let candidate = Vec2::new(
                rng.random_range(
                    (-bounds.half_extents.x + min_distance_from_edge)
                        ..(bounds.half_extents.x - min_distance_from_edge),
                ),
                rng.random_range(
                    (-bounds.half_extents.y + min_distance_from_edge)
                        ..(bounds.half_extents.y - min_distance_from_edge),
                ),
            );
            if candidate.length() > min_distance_from_center {
                break candidate;
            }
        };

        commands.spawn((
            Asteroid,
            AsteroidSize { radius },
            Transform::from_xyz(pos.x, 0.0, pos.y),
            Replicated,
        ));
    }

    info!(
        "Server: spawned symmetric fleets for 2 teams and {} asteroids",
        asteroid_count
    );
}

/// Resolves a `ClientId` to the connected client entity for team lookup.
fn client_entity(client_id: ClientId) -> Option<Entity> {
    match client_id {
        ClientId::Client(entity) => Some(entity),
        ClientId::Server => None,
    }
}

/// Validates that the given client owns a ship (same team). Returns the team if valid.
fn validate_ownership(
    client_id: ClientId,
    ship_entity: Entity,
    client_teams: &ClientTeams,
    ship_query: &Query<&Team, With<Ship>>,
    command_name: &str,
) -> Option<()> {
    let Some(entity) = client_entity(client_id) else {
        warn!("{command_name} from server client (no entity), ignoring");
        return None;
    };

    let Some(client_team) = client_teams.map.get(&entity) else {
        warn!("{command_name} from unknown client {entity:?}");
        return None;
    };

    let Ok(ship_team) = ship_query.get(ship_entity) else {
        warn!("{command_name} for invalid ship {ship_entity:?}");
        return None;
    };

    if ship_team != client_team {
        warn!(
            "{command_name} rejected: client Team({}) tried to control Team({}) ship",
            client_team.0, ship_team.0
        );
        return None;
    }

    Some(())
}

/// Observer: handle `MoveCommand` from clients.
fn handle_move_command(
    trigger: On<FromClient<MoveCommand>>,
    client_teams: Res<ClientTeams>,
    team_query: Query<&Team, With<Ship>>,
    mut waypoint_query: Query<&mut WaypointQueue, With<Ship>>,
) {
    let from = trigger.event();
    let cmd = &from.message;

    if validate_ownership(from.client_id, cmd.ship, &client_teams, &team_query, "MoveCommand")
        .is_none()
    {
        return;
    }

    let Ok(mut waypoints) = waypoint_query.get_mut(cmd.ship) else {
        return;
    };

    if cmd.append {
        waypoints.waypoints.push_back(cmd.destination);
        waypoints.braking = false;
    } else {
        waypoints.waypoints.clear();
        waypoints.waypoints.push_back(cmd.destination);
        waypoints.braking = false;
    }

    info!(
        "MoveCommand applied: ship {:?} -> ({}, {}), append={}",
        cmd.ship, cmd.destination.x, cmd.destination.y, cmd.append
    );
}

/// Observer: handle `FacingLockCommand` from clients.
fn handle_facing_lock_command(
    trigger: On<FromClient<FacingLockCommand>>,
    mut commands: Commands,
    client_teams: Res<ClientTeams>,
    team_query: Query<&Team, With<Ship>>,
) {
    let from = trigger.event();
    let cmd = &from.message;

    if validate_ownership(
        from.client_id,
        cmd.ship,
        &client_teams,
        &team_query,
        "FacingLockCommand",
    )
    .is_none()
    {
        return;
    }

    commands.entity(cmd.ship).insert((
        FacingTarget {
            direction: cmd.direction,
        },
        FacingLocked,
    ));

    info!(
        "FacingLockCommand applied: ship {:?} facing ({}, {})",
        cmd.ship, cmd.direction.x, cmd.direction.y
    );
}

/// Observer: handle `FacingUnlockCommand` from clients.
fn handle_facing_unlock_command(
    trigger: On<FromClient<FacingUnlockCommand>>,
    mut commands: Commands,
    client_teams: Res<ClientTeams>,
    team_query: Query<&Team, With<Ship>>,
) {
    let from = trigger.event();
    let cmd = &from.message;

    if validate_ownership(
        from.client_id,
        cmd.ship,
        &client_teams,
        &team_query,
        "FacingUnlockCommand",
    )
    .is_none()
    {
        return;
    }

    commands.entity(cmd.ship).remove::<FacingLocked>();

    info!("FacingUnlockCommand applied: ship {:?}", cmd.ship);
}

/// Observer that fires when a ConnectedClient is removed (client disconnects).
/// Ships belonging to the disconnected team remain in the world — physics keeps
/// running, so they will drift and brake to a stop naturally.
fn on_client_disconnected(
    trigger: On<Remove, ConnectedClient>,
    mut client_teams: ResMut<ClientTeams>,
) {
    let client_entity = trigger.entity;
    if let Some(team) = client_teams.map.remove(&client_entity) {
        info!(
            "Client {:?} (Team {}) disconnected. Ships will drift and brake.",
            client_entity, team.0
        );
    } else {
        info!("Unknown client {:?} disconnected", client_entity);
    }
}

/// Each frame, copy WaypointQueue/FacingTarget/FacingLocked from Ship entities
/// to their ShipSecrets child entities. Physics reads from Ship; replication reads
/// from ShipSecrets.
fn sync_ship_secrets(
    mut commands: Commands,
    ship_query: Query<
        (&WaypointQueue, Option<&FacingTarget>, Option<&FacingLocked>),
        With<Ship>,
    >,
    mut secrets_query: Query<
        (Entity, &ShipSecretsOwner, &mut WaypointQueue),
        (With<ShipSecrets>, Without<Ship>),
    >,
) {
    for (secrets_entity, owner, mut secrets_waypoints) in &mut secrets_query {
        let Ok((ship_waypoints, ship_facing, ship_locked)) = ship_query.get(owner.0) else {
            continue;
        };

        // Sync WaypointQueue
        *secrets_waypoints = ship_waypoints.clone();

        // Sync FacingTarget: insert or remove on the ShipSecrets entity
        if let Some(facing) = ship_facing {
            commands.entity(secrets_entity).insert(facing.clone());
        } else {
            commands.entity(secrets_entity).remove::<FacingTarget>();
        }

        // Sync FacingLocked: insert or remove on the ShipSecrets entity
        if ship_locked.is_some() {
            commands.entity(secrets_entity).insert(FacingLocked);
        } else {
            commands.entity(secrets_entity).remove::<FacingLocked>();
        }
    }
}

/// Each frame, compute LOS per-client and update replicon visibility.
///
/// For each connected client (which has an assigned team):
/// - Friendly ships are always visible to that client.
/// - Enemy ships are only visible if at least one friendly ship has LOS on them.
///
/// Per-component visibility is handled via ShipSecrets child entities:
/// WaypointQueue/FacingTarget/FacingLocked replicate only to the owning team.
fn server_update_visibility(
    los_bit: Res<LosBit>,
    client_teams: Res<ClientTeams>,
    ships: Query<(Entity, &Transform, &ShipClass, &Team), With<Ship>>,
    secrets_query: Query<(Entity, &ShipSecretsOwner), With<ShipSecrets>>,
    asteroid_query: Query<(&Transform, &AsteroidSize), With<Asteroid>>,
    mut clients: Query<(Entity, &mut ClientVisibility), With<ConnectedClient>>,
) {
    // Build asteroid list for LOS checks (will be empty if server has no asteroids spawned)
    let asteroids: Vec<(Vec2, f32)> = asteroid_query
        .iter()
        .map(|(t, s)| (Vec2::new(t.translation.x, t.translation.z), s.radius))
        .collect();

    // Collect all ships into a vec for cross-referencing
    let all_ships: Vec<(Entity, Vec2, f32, Team)> = ships
        .iter()
        .map(|(e, t, class, team)| {
            (e, ship_xz_position(t), class.profile().vision_range, *team)
        })
        .collect();

    // Build a map from ship entity to team for ShipSecrets lookup
    let ship_teams: HashMap<Entity, Team> = all_ships
        .iter()
        .map(|&(e, _, _, team)| (e, team))
        .collect();

    for (client_entity, mut client_visibility) in &mut clients {
        let Some(client_team) = client_teams.map.get(&client_entity) else {
            continue;
        };

        // Ship entity visibility (LOS-based for enemies, always for own team)
        for &(ship_entity, ship_pos, _vision_range, ship_team) in &all_ships {
            if ship_team == *client_team {
                // Friendly ship: always visible to this client
                client_visibility.set(ship_entity, **los_bit, true);
            } else {
                // Enemy ship: visible only if any friendly ship has LOS on it
                let seen = all_ships.iter().any(|&(_, friendly_pos, friendly_range, friendly_team)| {
                    friendly_team == *client_team
                        && is_in_los(friendly_pos, ship_pos, friendly_range, &asteroids)
                });
                client_visibility.set(ship_entity, **los_bit, seen);
            }
        }

        // ShipSecrets visibility: always visible to own team, never to enemy
        for (secrets_entity, owner) in &secrets_query {
            if let Some(ship_team) = ship_teams.get(&owner.0) {
                let visible = *ship_team == *client_team;
                client_visibility.set(secrets_entity, **los_bit, visible);
            }
        }
    }
}
