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
use crate::game::{GameState, Team};
use crate::map::{Asteroid, AsteroidSize, MapBounds};
use crate::net::commands::{
    CancelMissilesCommand, ClearTargetCommand, FacingLockCommand, FacingUnlockCommand,
    FireMissileCommand, JoinSquadCommand, MoveCommand, TargetCommand, TeamAssignment,
};
use crate::net::PROTOCOL_ID;
use crate::ship::{
    FacingLocked, FacingTarget, Ship, ShipClass, ShipSecrets, ShipSecretsOwner, SquadMember,
    TargetDesignation, WaypointQueue, ship_xz_position, spawn_server_ship,
    spawn_server_ship_default,
};
use crate::weapon::missile::{Missile, MissileOwner};
use crate::weapon::{MissileQueue, MissileQueueEntry, Mounts, WeaponCategory};
use crate::weapon::firing::{auto_fire, process_missile_queue, tick_weapon_cooldowns};

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
        // Replication registration is handled by SharedReplicationPlugin
        // (must be added before this plugin in both server and client).

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

        // Sync ship state to ShipSecrets children, then update visibility,
        // then clear targets that are no longer in LOS
        app.add_systems(
            Update,
            (sync_ship_secrets, server_update_visibility, clear_lost_targets)
                .chain()
                .run_if(in_state(GameState::Playing)),
        );

        // Weapon systems: cooldown ticking and auto-fire
        app.add_systems(
            Update,
            (tick_weapon_cooldowns, auto_fire, process_missile_queue)
                .chain()
                .run_if(in_state(GameState::Playing)),
        );

        // Observer for new client connections
        app.add_observer(on_client_connected);

        // Command handler observers
        app.add_observer(handle_move_command);
        app.add_observer(handle_facing_lock_command);
        app.add_observer(handle_facing_unlock_command);
        app.add_observer(handle_target_command);
        app.add_observer(handle_clear_target_command);
        app.add_observer(handle_fire_missile);
        app.add_observer(handle_cancel_missiles);
        app.add_observer(handle_join_squad);

        // Orphan cleanup: remove SquadMember when leader is destroyed/despawned
        app.add_systems(
            Update,
            cleanup_orphan_squad_members.run_if(in_state(GameState::Playing)),
        );

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

    // Send TeamAssignment immediately so client can transition to FleetComposition
    commands.server_trigger(ToClients {
        mode: SendMode::Direct(ClientId::Client(client_entity)),
        message: TeamAssignment { team },
    });

    // Server stays in WaitingForPlayers — lobby systems handle the
    // FleetComposition phase and transition to Playing when both fleets are submitted.
}

/// Spawn corners for each team's fleet.
const TEAM0_CORNER: Vec2 = Vec2::new(-300.0, -300.0);
const TEAM1_CORNER: Vec2 = Vec2::new(300.0, 300.0);

/// Minimum distance from spawn corners for asteroid placement.
const ASTEROID_EXCLUSION_RADIUS: f32 = 100.0;

/// Check if a candidate position is within an exclusion zone around spawn corners.
pub fn is_in_asteroid_exclusion_zone(candidate: Vec2) -> bool {
    candidate.distance(TEAM0_CORNER) < ASTEROID_EXCLUSION_RADIUS
        || candidate.distance(TEAM1_CORNER) < ASTEROID_EXCLUSION_RADIUS
}

/// Spawn fleets from lobby submissions when entering Playing state.
/// Also inserts MapBounds and spawns asteroids with exclusion zones.
fn server_setup_game(
    mut commands: Commands,
    lobby: Res<crate::fleet::lobby::LobbyTracker>,
    client_teams: Res<ClientTeams>,
) {
    // Insert MapBounds resource (server doesn't use MapPlugin which spawns visual elements)
    commands.insert_resource(MapBounds {
        half_extents: Vec2::splat(500.0),
    });

    // Spawn fleets from lobby submissions (or default fleets as fallback)
    let team_corners = [
        (Team(0), TEAM0_CORNER),
        (Team(1), TEAM1_CORNER),
    ];

    // Build a mapping: team_id -> Vec<ShipSpec>
    let mut team_specs: HashMap<u8, Vec<crate::fleet::ShipSpec>> = HashMap::new();
    for (&client_entity, specs) in &lobby.submissions {
        if let Some(team) = client_teams.map.get(&client_entity) {
            team_specs.insert(team.0, specs.clone());
        }
    }

    // Perpendicular direction for line formation offset:
    // spawn diagonal is (-1,-1) to (1,1), perpendicular is (-1,1) normalized
    let perp = Vec2::new(-1.0, 1.0).normalize();
    let ship_spacing = 30.0;

    for (team, corner) in &team_corners {
        if let Some(specs) = team_specs.get(&team.0) {
            // Spawn from lobby submissions
            for (i, spec) in specs.iter().enumerate() {
                let offset = perp * (i as f32 - (specs.len() as f32 - 1.0) / 2.0) * ship_spacing;
                let pos = *corner + offset;
                let entity = spawn_server_ship(&mut commands, pos, *team, spec, (i + 1) as u8);
                info!(
                    "Spawned {:?} for Team {} at ({:.0}, {:.0}): {:?}",
                    spec.class, team.0, pos.x, pos.y, entity
                );
            }
        } else {
            // Fallback: default fleet (1 battleship, 1 destroyer, 1 scout)
            let default_classes = [ShipClass::Battleship, ShipClass::Destroyer, ShipClass::Scout];
            for (i, class) in default_classes.iter().enumerate() {
                let offset =
                    perp * (i as f32 - (default_classes.len() as f32 - 1.0) / 2.0) * ship_spacing;
                let pos = *corner + offset;
                let entity = spawn_server_ship_default(&mut commands, pos, *team, *class);
                // Note: default ships get ShipNumber(0) via spawn_server_ship_default
                info!(
                    "Spawned default {:?} for Team {} at ({:.0}, {:.0}): {:?}",
                    class, team.0, pos.x, pos.y, entity
                );
            }
        }
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
            if candidate.length() > min_distance_from_center
                && !is_in_asteroid_exclusion_zone(candidate)
            {
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
        "Server: spawned fleets for 2 teams and {} asteroids",
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
/// If the target ship is a squad leader, propagate the move to all followers
/// with their offset applied. If the target ship is a follower receiving a
/// direct order, break formation (remove SquadMember).
fn handle_move_command(
    trigger: On<FromClient<MoveCommand>>,
    mut commands: Commands,
    client_teams: Res<ClientTeams>,
    team_query: Query<&Team, With<Ship>>,
    mut waypoint_query: Query<(&mut WaypointQueue, Option<&SquadMember>), With<Ship>>,
    follower_query: Query<(Entity, &SquadMember), With<Ship>>,
) {
    let from = trigger.event();
    let cmd = &from.message;

    if validate_ownership(from.client_id, cmd.ship, &client_teams, &team_query, "MoveCommand")
        .is_none()
    {
        return;
    }

    // If this ship is a follower, break formation (direct move order)
    {
        let Ok((_, squad)) = waypoint_query.get(cmd.ship) else {
            return;
        };
        if squad.is_some() {
            commands.entity(cmd.ship).remove::<SquadMember>();
            info!("Ship {:?} received direct move — leaving squad", cmd.ship);
        }
    }

    // Apply the move to the target ship
    {
        let Ok((mut waypoints, _)) = waypoint_query.get_mut(cmd.ship) else {
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
    }

    // Propagate to squad followers (if this ship is a leader)
    for (follower_entity, squad_member) in &follower_query {
        if squad_member.leader != cmd.ship {
            continue;
        }
        // Skip the ship itself — its SquadMember removal is deferred and hasn't flushed yet
        if follower_entity == cmd.ship {
            continue;
        }
        let offset = squad_member.offset;
        let follower_dest = cmd.destination + offset;

        let Ok((mut follower_waypoints, _)) = waypoint_query.get_mut(follower_entity) else {
            continue;
        };
        if cmd.append {
            follower_waypoints.waypoints.push_back(follower_dest);
            follower_waypoints.braking = false;
        } else {
            follower_waypoints.waypoints.clear();
            follower_waypoints.waypoints.push_back(follower_dest);
            follower_waypoints.braking = false;
        }

        info!(
            "Squad propagated: follower {:?} -> ({:.0}, {:.0})",
            follower_entity, follower_dest.x, follower_dest.y
        );
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

/// Observer: handle `TargetCommand` from clients.
fn handle_target_command(
    trigger: On<FromClient<TargetCommand>>,
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
        "TargetCommand",
    )
    .is_none()
    {
        return;
    }

    // Validate target is a ship
    let Ok(target_team) = team_query.get(cmd.target) else {
        warn!(
            "TargetCommand rejected: target {:?} is not a ship",
            cmd.target
        );
        return;
    };

    // Validate target is on a different team
    let Ok(ship_team) = team_query.get(cmd.ship) else {
        return;
    };
    if *target_team == *ship_team {
        warn!(
            "TargetCommand rejected: target {:?} is on the same team as ship {:?}",
            cmd.target, cmd.ship
        );
        return;
    }

    commands
        .entity(cmd.ship)
        .insert(TargetDesignation(cmd.target));

    info!(
        "TargetCommand applied: ship {:?} targeting {:?}",
        cmd.ship, cmd.target
    );
}

/// Observer: handle `ClearTargetCommand` from clients.
fn handle_clear_target_command(
    trigger: On<FromClient<ClearTargetCommand>>,
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
        "ClearTargetCommand",
    )
    .is_none()
    {
        return;
    }

    commands.entity(cmd.ship).remove::<TargetDesignation>();

    info!("ClearTargetCommand applied: ship {:?}", cmd.ship);
}

/// Observer: handle `FireMissileCommand` from clients.
fn handle_fire_missile(
    trigger: On<FromClient<FireMissileCommand>>,
    client_teams: Res<ClientTeams>,
    team_query: Query<&Team, With<Ship>>,
    mut ship_query: Query<(&mut MissileQueue, &Mounts), With<Ship>>,
) {
    let from = trigger.event();
    let cmd = &from.message;

    if validate_ownership(
        from.client_id,
        cmd.ship,
        &client_teams,
        &team_query,
        "FireMissileCommand",
    )
    .is_none()
    {
        return;
    }

    let Ok((mut queue, mounts)) = ship_query.get_mut(cmd.ship) else {
        return;
    };

    // Count total loaded tubes across all VLS mounts
    let total_loaded: usize = mounts
        .0
        .iter()
        .filter_map(|m| {
            let w = m.weapon.as_ref()?;
            if w.weapon_type.category() == WeaponCategory::Missile {
                Some(w.tubes_loaded as usize)
            } else {
                None
            }
        })
        .sum();

    // Reject if queue already has as many entries as loaded tubes
    if queue.0.len() >= total_loaded {
        return;
    }

    queue.0.push(MissileQueueEntry {
        target_point: cmd.target_point,
        target_entity: cmd.target_entity,
    });

    info!(
        "FireMissileCommand applied: ship {:?} queued missile at ({}, {})",
        cmd.ship, cmd.target_point.x, cmd.target_point.y
    );
}

/// Observer: handle `CancelMissilesCommand` from clients.
fn handle_cancel_missiles(
    trigger: On<FromClient<CancelMissilesCommand>>,
    client_teams: Res<ClientTeams>,
    team_query: Query<&Team, With<Ship>>,
    mut queue_query: Query<&mut MissileQueue, With<Ship>>,
) {
    let from = trigger.event();
    let cmd = &from.message;

    if validate_ownership(
        from.client_id,
        cmd.ship,
        &client_teams,
        &team_query,
        "CancelMissilesCommand",
    )
    .is_none()
    {
        return;
    }

    let Ok(mut queue) = queue_query.get_mut(cmd.ship) else {
        return;
    };

    queue.0.clear();

    info!("CancelMissilesCommand applied: ship {:?}", cmd.ship);
}

/// Observer: handle `JoinSquadCommand` from clients.
/// Validates both ships are on the same team, then adds SquadMember to the follower.
/// Prevents cycles by walking the leader chain up to 10 hops.
/// Reassigns existing followers of cmd.ship to follow cmd.leader instead.
fn handle_join_squad(
    trigger: On<FromClient<JoinSquadCommand>>,
    mut commands: Commands,
    client_teams: Res<ClientTeams>,
    team_query: Query<&Team, With<Ship>>,
    transform_query: Query<&Transform, With<Ship>>,
    squad_query: Query<(Entity, &SquadMember), With<Ship>>,
) {
    let from = trigger.event();
    let cmd = &from.message;

    if validate_ownership(
        from.client_id,
        cmd.ship,
        &client_teams,
        &team_query,
        "JoinSquadCommand",
    )
    .is_none()
    {
        return;
    }

    // Validate leader exists and is on the same team
    let Ok(ship_team) = team_query.get(cmd.ship) else {
        return;
    };
    let Ok(leader_team) = team_query.get(cmd.leader) else {
        warn!("JoinSquadCommand: leader {:?} not found", cmd.leader);
        return;
    };
    if *ship_team != *leader_team {
        warn!("JoinSquadCommand: ship and leader are on different teams");
        return;
    }

    // Don't join self
    if cmd.ship == cmd.leader {
        return;
    }

    // Cycle detection: walk the leader chain from cmd.leader up to 10 hops.
    // If we encounter cmd.ship, adding it would create a cycle.
    {
        let mut current = cmd.leader;
        for _ in 0..10 {
            let found = squad_query.iter().find(|(e, _)| *e == current);
            if let Some((_, member)) = found {
                if member.leader == cmd.ship {
                    warn!(
                        "JoinSquadCommand rejected: would create cycle ({:?} -> {:?})",
                        cmd.ship, cmd.leader
                    );
                    return;
                }
                current = member.leader;
            } else {
                break;
            }
        }
    }

    // Reassign existing followers of cmd.ship to follow cmd.leader instead
    let Ok(leader_tf) = transform_query.get(cmd.leader) else {
        return;
    };
    let leader_pos = ship_xz_position(leader_tf);

    for (follower_entity, squad_member) in &squad_query {
        if squad_member.leader == cmd.ship {
            let Ok(follower_tf) = transform_query.get(follower_entity) else {
                continue;
            };
            let follower_pos = ship_xz_position(follower_tf);
            let new_offset = follower_pos - leader_pos;
            commands.entity(follower_entity).insert(SquadMember {
                leader: cmd.leader,
                offset: new_offset,
            });
            info!(
                "Reassigned follower {:?} from {:?} to {:?}",
                follower_entity, cmd.ship, cmd.leader
            );
        }
    }

    // Compute offset from positions
    let Ok(ship_tf) = transform_query.get(cmd.ship) else {
        return;
    };

    let ship_pos = ship_xz_position(ship_tf);
    let offset = ship_pos - leader_pos;

    commands.entity(cmd.ship).insert(SquadMember {
        leader: cmd.leader,
        offset,
    });

    info!(
        "JoinSquadCommand applied: ship {:?} joined squad of {:?} with offset ({:.0}, {:.0})",
        cmd.ship, cmd.leader, offset.x, offset.y
    );
}

/// Remove SquadMember from ships whose leader no longer exists or is destroyed.
fn cleanup_orphan_squad_members(
    mut commands: Commands,
    squad_query: Query<(Entity, &SquadMember), With<Ship>>,
    ship_query: Query<Entity, With<Ship>>,
    destroyed_query: Query<Entity, With<crate::game::Destroyed>>,
) {
    for (entity, squad) in &squad_query {
        let leader_gone = ship_query.get(squad.leader).is_err();
        let leader_destroyed = destroyed_query.get(squad.leader).is_ok();
        if leader_gone || leader_destroyed {
            commands.entity(entity).remove::<SquadMember>();
            info!(
                "Orphan cleanup: ship {:?} lost leader {:?}",
                entity, squad.leader
            );
        }
    }
}

/// Each frame, check if targeted enemies are still visible to the targeting ship's team.
/// If no friendly ship has LOS on the target, clear the TargetDesignation.
fn clear_lost_targets(
    mut commands: Commands,
    targeting_ships: Query<(Entity, &TargetDesignation, &Team), With<Ship>>,
    all_ships: Query<(Entity, &Transform, &ShipClass, &Team), With<Ship>>,
    target_transforms: Query<&Transform, With<Ship>>,
    asteroid_query: Query<(&Transform, &AsteroidSize), With<Asteroid>>,
) {
    let asteroids: Vec<(Vec2, f32)> = asteroid_query
        .iter()
        .map(|(t, s)| (Vec2::new(t.translation.x, t.translation.z), s.radius))
        .collect();

    for (ship_entity, target_designation, ship_team) in &targeting_ships {
        let Ok(target_transform) = target_transforms.get(target_designation.0) else {
            // Target entity no longer exists — clear designation
            commands.entity(ship_entity).remove::<TargetDesignation>();
            continue;
        };

        let target_pos = ship_xz_position(target_transform);

        // Check if any friendly ship has LOS on the target
        let any_friendly_sees_target =
            all_ships
                .iter()
                .any(|(_, friendly_t, friendly_class, friendly_team)| {
                    *friendly_team == *ship_team
                        && is_in_los(
                            ship_xz_position(friendly_t),
                            target_pos,
                            friendly_class.profile().vision_range,
                            &asteroids,
                        )
                });

        if !any_friendly_sees_target {
            commands.entity(ship_entity).remove::<TargetDesignation>();
            info!(
                "Target {:?} lost LOS — clearing designation on ship {:?}",
                target_designation.0, ship_entity
            );
        }
    }
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
        (
            &WaypointQueue,
            Option<&FacingTarget>,
            Option<&FacingLocked>,
            Option<&TargetDesignation>,
            &MissileQueue,
            Option<&SquadMember>,
        ),
        With<Ship>,
    >,
    mut secrets_query: Query<
        (Entity, &ShipSecretsOwner, &mut WaypointQueue, &mut MissileQueue),
        (With<ShipSecrets>, Without<Ship>),
    >,
) {
    for (secrets_entity, owner, mut secrets_waypoints, mut secrets_missiles) in &mut secrets_query {
        let Ok((ship_waypoints, ship_facing, ship_locked, ship_target, ship_missiles, ship_squad)) =
            ship_query.get(owner.0)
        else {
            continue;
        };

        // Sync WaypointQueue
        *secrets_waypoints = ship_waypoints.clone();

        // Sync MissileQueue
        *secrets_missiles = ship_missiles.clone();

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

        // Sync TargetDesignation: insert or remove on the ShipSecrets entity
        if let Some(target) = ship_target {
            commands.entity(secrets_entity).insert(target.clone());
        } else {
            commands
                .entity(secrets_entity)
                .remove::<TargetDesignation>();
        }

        // Sync SquadMember: insert or remove on the ShipSecrets entity
        if let Some(squad) = ship_squad {
            commands.entity(secrets_entity).insert(squad.clone());
        } else {
            commands.entity(secrets_entity).remove::<SquadMember>();
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
    missile_query: Query<(Entity, &Transform, &MissileOwner), With<Missile>>,
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

        // Missile entity visibility:
        // Own team's missiles: always visible (you can track your own missiles)
        // Enemy missiles: LOS-based (only visible if a friendly ship can see them)
        for (missile_entity, missile_transform, missile_owner) in &missile_query {
            let is_friendly = ship_teams
                .get(&missile_owner.0)
                .is_some_and(|owner_team| *owner_team == *client_team);

            let visible = if is_friendly {
                true
            } else {
                let missile_pos = Vec2::new(
                    missile_transform.translation.x,
                    missile_transform.translation.z,
                );
                all_ships.iter().any(
                    |&(_, friendly_pos, friendly_range, ship_team)| {
                        ship_team == *client_team
                            && is_in_los(friendly_pos, missile_pos, friendly_range, &asteroids)
                    },
                )
            };
            client_visibility.set(missile_entity, **los_bit, visible);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asteroid_exclusion_near_team0_corner() {
        // Right at team 0 corner (-300, -300)
        assert!(is_in_asteroid_exclusion_zone(Vec2::new(-300.0, -300.0)));
        // Just inside the 100m radius
        assert!(is_in_asteroid_exclusion_zone(Vec2::new(-300.0, -210.0)));
    }

    #[test]
    fn asteroid_exclusion_near_team1_corner() {
        // Right at team 1 corner (300, 300)
        assert!(is_in_asteroid_exclusion_zone(Vec2::new(300.0, 300.0)));
        // Just inside the 100m radius
        assert!(is_in_asteroid_exclusion_zone(Vec2::new(300.0, 210.0)));
    }

    #[test]
    fn asteroid_exclusion_outside_both_zones() {
        // Center of the map — far from both corners
        assert!(!is_in_asteroid_exclusion_zone(Vec2::ZERO));
        // Midway between corners
        assert!(!is_in_asteroid_exclusion_zone(Vec2::new(0.0, 200.0)));
    }

    #[test]
    fn asteroid_exclusion_boundary() {
        // Exactly at 100m from team 0 corner should be excluded (< 100)
        // Distance from (-300,-300) to (-200,-300) = 100, which is NOT < 100
        assert!(!is_in_asteroid_exclusion_zone(Vec2::new(-200.0, -300.0)));
        // Just barely inside
        assert!(is_in_asteroid_exclusion_zone(Vec2::new(-201.0, -300.0)));
    }
}
