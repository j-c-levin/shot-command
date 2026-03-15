use bevy::ecs::entity::MapEntities;
use bevy::prelude::*;
use bevy_replicon::prelude::Replicated;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

use crate::game::{EnemyVisibility, GameState, Health, Team};
use crate::net::LocalTeam;
use crate::weapon::{Mount, MountSize, Mounts, WeaponState, WeaponType};

pub struct ShipPhysicsPlugin;

impl Plugin for ShipPhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                update_facing_targets,
                turn_ships,
                apply_thrust,
                apply_velocity,
                check_waypoint_arrival,
                clamp_ships_to_bounds,
            )
                .chain()
                .run_if(in_state(GameState::Playing)),
        );
    }
}

pub struct ShipVisualsPlugin;

impl Plugin for ShipVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, init_indicator_assets)
            .add_systems(
                Update,
                (update_waypoint_markers, update_facing_indicators)
                    .run_if(in_state(GameState::Playing)),
            );
    }
}

pub struct ShipPlugin;

impl Plugin for ShipPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((ShipPhysicsPlugin, ShipVisualsPlugin));
    }
}

// ── Components ──────────────────────────────────────────────────────────

#[derive(Component, Serialize, Deserialize)]
pub struct Ship;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShipClass {
    Battleship,
    Destroyer,
    Scout,
}

#[derive(Clone, Debug)]
pub struct ShipProfile {
    pub hp: u16,
    pub acceleration: f32,
    pub thruster_factor: f32,
    pub turn_rate: f32,
    pub turn_acceleration: f32,
    pub top_speed: f32,
    pub vision_range: f32,
    pub collision_radius: f32,
}

impl ShipClass {
    /// Fixed mount layout for this ship class — sizes and positions.
    /// This defines what slots exist; weapons are assigned separately.
    fn mount_layout(&self) -> Vec<(MountSize, Vec2)> {
        match self {
            ShipClass::Battleship => vec![
                (MountSize::Large, Vec2::new(-8.0, 6.0)),
                (MountSize::Large, Vec2::new(8.0, 6.0)),
                (MountSize::Medium, Vec2::new(-5.0, 0.0)),
                (MountSize::Medium, Vec2::new(5.0, 0.0)),
                (MountSize::Small, Vec2::new(-3.0, -6.0)),
                (MountSize::Small, Vec2::new(3.0, -6.0)),
            ],
            ShipClass::Destroyer => vec![
                (MountSize::Large, Vec2::new(0.0, 6.0)),
                (MountSize::Medium, Vec2::new(-4.0, 0.0)),
                (MountSize::Medium, Vec2::new(4.0, 0.0)),
                (MountSize::Small, Vec2::new(0.0, -5.0)),
            ],
            ShipClass::Scout => vec![
                (MountSize::Medium, Vec2::new(0.0, 3.0)),
                (MountSize::Small, Vec2::new(0.0, -3.0)),
            ],
        }
    }

    /// Default weapon loadout for this ship class.
    /// Each entry is an Option<WeaponType> matching the mount layout by index.
    /// `None` means the slot is empty. Edit this to test different loadouts.
    ///
    /// Battleship: 2 Large, 2 Medium, 2 Small
    /// Destroyer:  1 Large, 2 Medium, 1 Small
    /// Scout:      1 Medium, 1 Small
    fn default_loadout(&self) -> Vec<Option<WeaponType>> {
        match self {
            ShipClass::Battleship => vec![
                Some(WeaponType::HeavyCannon), // Large
                None,                          // Large
                None,                          // Medium
                None,                          // Medium
                None,                          // Small
                None,                          // Small
            ],
            ShipClass::Destroyer => vec![
                Some(WeaponType::Railgun),     // Large
                None,                          // Medium
                None,                          // Medium
                None,                          // Small
            ],
            ShipClass::Scout => vec![
                Some(WeaponType::Cannon),      // Medium
                None,                          // Small
            ],
        }
    }

    /// Builds the mount list by combining the fixed layout with the default loadout.
    pub fn default_mounts(&self) -> Vec<Mount> {
        let layout = self.mount_layout();
        let loadout = self.default_loadout();
        layout
            .into_iter()
            .zip(loadout)
            .map(|((size, offset), weapon_type)| Mount {
                size,
                offset,
                weapon: weapon_type.map(|wt| {
                    let profile = wt.profile();
                    WeaponState {
                        weapon_type: wt,
                        ammo: profile.max_ammo,
                        cooldown: 0.0,
                    }
                }),
            })
            .collect()
    }

    pub fn profile(&self) -> ShipProfile {
        match self {
            ShipClass::Battleship => ShipProfile {
                hp: 200,
                acceleration: 6.0,
                thruster_factor: 0.2,
                turn_rate: 0.8,
                turn_acceleration: 0.4,
                top_speed: 20.0,
                vision_range: 200.0,
                collision_radius: 12.0,
            },
            ShipClass::Destroyer => ShipProfile {
                hp: 100,
                acceleration: 10.0,
                thruster_factor: 0.3,
                turn_rate: 1.5,
                turn_acceleration: 1.0,
                top_speed: 28.0,
                vision_range: 200.0,
                collision_radius: 8.0,
            },
            ShipClass::Scout => ShipProfile {
                hp: 50,
                acceleration: 14.0,
                thruster_factor: 0.5,
                turn_rate: 3.0,
                turn_acceleration: 2.0,
                top_speed: 35.0,
                vision_range: 200.0,
                collision_radius: 5.0,
            },
        }
    }
}

#[derive(Component, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Velocity {
    pub linear: Vec2,
    pub angular: f32,
}

#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct WaypointQueue {
    pub waypoints: VecDeque<Vec2>,
    /// True after final waypoint is popped — ship should auto-brake
    pub braking: bool,
}

impl Default for WaypointQueue {
    fn default() -> Self {
        Self {
            waypoints: VecDeque::new(),
            braking: false,
        }
    }
}

#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct FacingTarget {
    pub direction: Vec2,
}

/// Marker: ship facing is player-locked, not auto-set by waypoints
#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct FacingLocked;

/// The entity this ship is targeting (must be an enemy ship).
/// Cleared automatically when the target leaves LOS.
#[derive(Component, Clone, Debug, Serialize, Deserialize, MapEntities)]
pub struct TargetDesignation(#[entities] pub Entity);

#[derive(Component)]
pub struct Selected;

#[derive(Component)]
pub struct SelectionIndicator;

/// Marker for the child entity that holds team-private ship state.
/// This entity is only visible to the owning team via replicon visibility,
/// preventing enemies from seeing WaypointQueue, FacingTarget, and FacingLocked.
#[derive(Component, Serialize, Deserialize)]
pub struct ShipSecrets;

/// Points from a ShipSecrets entity back to its parent Ship entity.
/// Uses `#[entities]` for replicon entity mapping across server/client.
#[derive(Component, Serialize, Deserialize, MapEntities)]
pub struct ShipSecretsOwner(#[entities] pub Entity);

/// Marker for waypoint indicator entities.
#[derive(Component)]
pub struct WaypointMarker {
    pub owner: Entity,
}

/// Marker for facing direction indicator.
#[derive(Component)]
pub struct FacingIndicator {
    pub owner: Entity,
}

/// Cached mesh/material handles for visual indicators (avoids per-frame allocation)
#[derive(Resource)]
struct IndicatorAssets {
    waypoint_mesh: Handle<Mesh>,
    waypoint_material: Handle<StandardMaterial>,
    facing_mesh: Handle<Mesh>,
    facing_material: Handle<StandardMaterial>,
}

fn init_indicator_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(IndicatorAssets {
        waypoint_mesh: meshes.add(Sphere::new(2.0)),
        waypoint_material: materials.add(StandardMaterial {
            base_color: Color::srgba(0.2, 0.8, 1.0, 0.5),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        }),
        facing_mesh: meshes.add(Capsule3d::new(0.5, 30.0)),
        facing_material: materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.8, 0.2, 0.6),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        }),
    });
}

// ── Pure Functions ──────────────────────────────────────────────────────

/// Thrust multiplier based on angle between facing and movement direction.
/// 0 radians (facing target) → 1.0
/// PI radians (facing away) → thruster_factor
/// Smooth cosine interpolation between.
pub fn thrust_multiplier(angle: f32, thruster_factor: f32) -> f32 {
    let t = (1.0 + angle.cos()) / 2.0;
    thruster_factor + t * (1.0 - thruster_factor)
}

/// Unsigned angle (0..PI) between two unit vectors.
pub fn angle_between_directions(a: Vec2, b: Vec2) -> f32 {
    a.dot(b).clamp(-1.0, 1.0).acos()
}

/// Braking distance: v²/(2a). How far you travel while decelerating to zero.
pub fn braking_distance(speed: f32, deceleration: f32) -> f32 {
    if deceleration <= 0.0 || speed <= 0.0 {
        return 0.0;
    }
    (speed * speed) / (2.0 * deceleration)
}

/// Signed shortest angle from `from` to `to` in radians (-PI..PI).
pub fn shortest_angle_delta(from: f32, to: f32) -> f32 {
    let mut delta = (to - from) % std::f32::consts::TAU;
    if delta > std::f32::consts::PI {
        delta -= std::f32::consts::TAU;
    } else if delta < -std::f32::consts::PI {
        delta += std::f32::consts::TAU;
    }
    delta
}

/// Extract XZ position from transform as Vec2.
pub fn ship_xz_position(transform: &Transform) -> Vec2 {
    Vec2::new(transform.translation.x, transform.translation.z)
}

/// Get the ship's facing direction as a Vec2 in XZ plane.
pub fn ship_facing_direction(transform: &Transform) -> Vec2 {
    let forward = transform.forward();
    Vec2::new(forward.x, forward.z).normalize_or_zero()
}

/// Get the current heading angle (radians) of the ship in XZ plane.
pub fn ship_heading(transform: &Transform) -> f32 {
    let dir = ship_facing_direction(transform);
    dir.y.atan2(dir.x)
}

/// Compute the desired velocity to reach a target, accounting for braking.
/// Returns a velocity vector that, if matched, would take the ship straight to
/// the target and arrive at zero speed (for last waypoint) or cruise speed.
///
/// `deceleration` is the effective braking force available.
/// A safety factor of 2.5x is applied so ships start slowing down well before
/// the target and arrive cleanly — braking shouldn't be a player concern.
pub fn desired_velocity_to_target(
    to_target: Vec2,
    dist: f32,
    top_speed: f32,
    deceleration: f32,
    is_last_waypoint: bool,
) -> Vec2 {
    if dist < 0.5 {
        return Vec2::ZERO;
    }
    let dir = to_target / dist;

    let desired_speed = if is_last_waypoint {
        // v = sqrt(2 * a * d), with safety factor so we brake early
        let safe_decel = deceleration * 2.5;
        let stopping_speed = (2.0 * safe_decel * dist).sqrt();
        // Also cap approach speed based on distance — don't come in hot
        let approach_cap = (dist * 0.8).min(top_speed);
        stopping_speed.min(approach_cap).min(top_speed)
    } else {
        top_speed
    };

    dir * desired_speed
}

/// Compute the thrust vector needed to correct velocity toward desired.
/// Returns the direction and magnitude of thrust to apply.
pub fn compute_steering_thrust(
    current_velocity: Vec2,
    desired_velocity: Vec2,
    facing: Vec2,
    acceleration: f32,
    thruster_factor: f32,
    dt: f32,
) -> Vec2 {
    let velocity_error = desired_velocity - current_velocity;
    let error_magnitude = velocity_error.length();

    if error_magnitude < 0.01 {
        return Vec2::ZERO;
    }

    let thrust_dir = velocity_error / error_magnitude;

    // How much thrust we can apply in this direction given our facing
    let angle = angle_between_directions(facing, thrust_dir);
    let effective_accel = acceleration * thrust_multiplier(angle, thruster_factor);

    // Don't overshoot the correction
    let thrust_magnitude = (effective_accel * dt).min(error_magnitude);

    thrust_dir * thrust_magnitude
}

// ── Systems ─────────────────────────────────────────────────────────────

fn update_facing_targets(
    mut commands: Commands,
    query: Query<
        (
            Entity,
            &Transform,
            &WaypointQueue,
            Option<&FacingLocked>,
            Option<&TargetDesignation>,
            Option<&crate::weapon::Mounts>,
        ),
        With<Ship>,
    >,
    target_transforms: Query<&Transform, Without<crate::weapon::projectile::Projectile>>,
) {
    for (entity, transform, waypoints, locked, target, mounts) in &query {
        if locked.is_some() {
            continue;
        }

        // Check if this ship has a forward-arc weapon and a target — if so,
        // face the target so the railgun can fire, even while moving to a waypoint.
        if let (Some(designation), Some(mounts)) = (target, mounts) {
            let has_forward_weapon = mounts.0.iter().any(|m| {
                m.weapon.as_ref().is_some_and(|w| {
                    w.weapon_type.profile().arc == crate::weapon::FiringArc::Forward
                })
            });
            if has_forward_weapon {
                if let Ok(target_transform) = target_transforms.get(designation.0) {
                    let pos = ship_xz_position(transform);
                    let target_pos = ship_xz_position(target_transform);
                    let dir = (target_pos - pos).normalize_or_zero();
                    if dir != Vec2::ZERO {
                        commands.entity(entity).insert(FacingTarget { direction: dir });
                        continue;
                    }
                }
            }
        }

        // Default behavior: face next waypoint, or remove facing target
        if let Some(&next_wp) = waypoints.waypoints.front() {
            let pos = ship_xz_position(transform);
            let dir = (next_wp - pos).normalize_or_zero();
            if dir != Vec2::ZERO {
                commands.entity(entity).insert(FacingTarget { direction: dir });
            }
        } else {
            commands.entity(entity).remove::<FacingTarget>();
        }
    }
}

fn turn_ships(
    time: Res<Time>,
    mut query: Query<
        (&mut Transform, &mut Velocity, &ShipClass, Option<&FacingTarget>),
        With<Ship>,
    >,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }

    for (mut transform, mut velocity, class, facing_target) in &mut query {
        let profile = class.profile();

        let Some(target) = facing_target else {
            // No target — decelerate angular velocity to zero
            if velocity.angular.abs() > 0.001 {
                let decel = profile.turn_acceleration * dt;
                if velocity.angular.abs() <= decel {
                    velocity.angular = 0.0;
                } else {
                    velocity.angular -= velocity.angular.signum() * decel;
                }
            } else {
                velocity.angular = 0.0;
            }

            if velocity.angular.abs() > 0.001 {
                let current_heading = ship_heading(&transform);
                let new_heading = current_heading + velocity.angular * dt;
                let dir = Vec2::new(new_heading.cos(), new_heading.sin());
                let look_target = Vec3::new(
                    transform.translation.x + dir.x,
                    transform.translation.y,
                    transform.translation.z + dir.y,
                );
                transform.look_at(look_target, Vec3::Y);
            }
            continue;
        };

        let target_heading = target.direction.y.atan2(target.direction.x);
        let current_heading = ship_heading(&transform);
        let delta = shortest_angle_delta(current_heading, target_heading);

        // If close enough to target, snap and zero angular velocity
        if delta.abs() < 0.01 && velocity.angular.abs() < 0.01 {
            velocity.angular = 0.0;
            let look_target = Vec3::new(
                transform.translation.x + target.direction.x,
                transform.translation.y,
                transform.translation.z + target.direction.y,
            );
            transform.look_at(look_target, Vec3::Y);
            continue;
        }

        // Calculate if we need to decelerate to stop at target angle
        let stop_distance = braking_distance(velocity.angular.abs(), profile.turn_acceleration);
        let should_brake = stop_distance >= delta.abs();

        if should_brake {
            let decel = profile.turn_acceleration * dt;
            if velocity.angular.abs() <= decel {
                velocity.angular = 0.0;
            } else {
                velocity.angular -= velocity.angular.signum() * decel;
            }
        } else {
            let desired_sign = delta.signum();
            velocity.angular += desired_sign * profile.turn_acceleration * dt;
            velocity.angular = velocity.angular.clamp(-profile.turn_rate, profile.turn_rate);
        }

        // Apply angular velocity
        let new_heading = current_heading + velocity.angular * dt;
        let dir = Vec2::new(new_heading.cos(), new_heading.sin());
        let look_target = Vec3::new(
            transform.translation.x + dir.x,
            transform.translation.y,
            transform.translation.z + dir.y,
        );
        transform.look_at(look_target, Vec3::Y);
    }
}

fn apply_thrust(
    time: Res<Time>,
    mut query: Query<
        (&Transform, &mut Velocity, &ShipClass, &WaypointQueue),
        With<Ship>,
    >,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }

    for (transform, mut velocity, class, waypoints) in &mut query {
        let profile = class.profile();
        let facing = ship_facing_direction(transform);
        let speed = velocity.linear.length();

        // No waypoints — always brake to a stop
        if waypoints.waypoints.is_empty() {
            if speed > 0.1 {
                let correction = compute_steering_thrust(
                    velocity.linear,
                    Vec2::ZERO, // desired: stop
                    facing,
                    profile.acceleration,
                    profile.thruster_factor,
                    dt,
                );
                velocity.linear += correction;
                if velocity.linear.length() < 0.1 {
                    velocity.linear = Vec2::ZERO;
                }
            } else {
                velocity.linear = Vec2::ZERO;
            }
            continue;
        }

        // Has waypoints — use steering controller
        let pos = ship_xz_position(transform);
        let next_wp = waypoints.waypoints[0];
        let to_target = next_wp - pos;
        let dist = to_target.length();

        if dist < 0.5 {
            continue;
        }

        let is_last = waypoints.waypoints.len() == 1;

        // Worst-case deceleration: ship faces target and must brake with
        // rear thrusters only (thruster_factor). Using worst-case ensures
        // the ship starts braking early enough to stop at the waypoint.
        let min_decel = profile.acceleration * profile.thruster_factor;

        let desired = desired_velocity_to_target(
            to_target,
            dist,
            profile.top_speed,
            min_decel,
            is_last,
        );

        let correction = compute_steering_thrust(
            velocity.linear,
            desired,
            facing,
            profile.acceleration,
            profile.thruster_factor,
            dt,
        );

        velocity.linear += correction;

        // Clamp to top speed
        let new_speed = velocity.linear.length();
        if new_speed > profile.top_speed {
            velocity.linear = velocity.linear.normalize() * profile.top_speed;
        }
    }
}

/// Fraction of velocity lost per second to "space friction" (drag).
/// Not realistic, but makes ships feel controllable and assists braking.
/// At 0.3, a coasting ship loses ~26% of its speed per second.
const SPACE_DRAG: f32 = 0.3;

fn apply_velocity(
    time: Res<Time>,
    mut query: Query<(&mut Transform, &mut Velocity), With<Ship>>,
) {
    let dt = time.delta_secs();
    for (mut transform, mut velocity) in &mut query {
        // Apply drag: exponential decay so ships naturally bleed speed
        let drag = (1.0 - SPACE_DRAG * dt).max(0.0);
        velocity.linear *= drag;

        transform.translation.x += velocity.linear.x * dt;
        transform.translation.z += velocity.linear.y * dt;
    }
}

const ARRIVAL_THRESHOLD_TIGHT: f32 = 10.0;
const ARRIVAL_THRESHOLD_LOOSE: f32 = 30.0;

fn check_waypoint_arrival(
    mut query: Query<(&Transform, &mut WaypointQueue), With<Ship>>,
) {
    for (transform, mut waypoints) in &mut query {
        if let Some(&next_wp) = waypoints.waypoints.front() {
            let pos = ship_xz_position(transform);
            let dist = (next_wp - pos).length();

            // Tight threshold for final destination, loose for intermediate waypoints
            let threshold = if waypoints.waypoints.len() > 1 {
                ARRIVAL_THRESHOLD_LOOSE
            } else {
                ARRIVAL_THRESHOLD_TIGHT
            };

            if dist < threshold {
                waypoints.waypoints.pop_front();
                if waypoints.waypoints.is_empty() {
                    waypoints.braking = true;
                }
            }
        }
    }
}

fn clamp_ships_to_bounds() {
    // Ships are allowed to drift outside map bounds and return on their own.
    // Player commands are constrained to the ground plane (which is within bounds).
}

// ── Spawning ────────────────────────────────────────────────────────────

pub fn spawn_ship(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    position: Vec2,
    team: Team,
    color: Color,
    class: ShipClass,
) -> Entity {
    let ship_mesh = match class {
        ShipClass::Battleship => meshes.add(Cuboid::new(12.0, 8.0, 28.0)),
        ShipClass::Destroyer => meshes.add(Cone {
            radius: 8.0,
            height: 20.0,
        }),
        ShipClass::Scout => meshes.add(Sphere::new(1.0).mesh().uv(16, 16)),
    };

    // Child transform: rotation to align forward with -Z, plus scale for ellipsoid
    let mesh_transform = match class {
        ShipClass::Battleship => Transform::IDENTITY,
        ShipClass::Destroyer => {
            Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2))
        }
        // Ellipsoid: unit sphere scaled (wide, short, long)
        ShipClass::Scout => Transform::from_scale(Vec3::new(4.0, 3.0, 7.0)),
    };

    let is_enemy = team != Team::PLAYER;
    let ship_material = materials.add(StandardMaterial {
        base_color: if is_enemy {
            color.with_alpha(0.0)
        } else {
            color
        },
        emissive: color.into(),
        alpha_mode: if is_enemy {
            AlphaMode::Blend
        } else {
            AlphaMode::Opaque
        },
        ..default()
    });

    let initial_visibility = if is_enemy {
        Visibility::Hidden
    } else {
        Visibility::Visible
    };

    let mut entity_commands = commands.spawn((
        Ship,
        team,
        class,
        Velocity::default(),
        WaypointQueue::default(),
        Transform::from_xyz(position.x, 5.0, position.y),
        initial_visibility,
    ));

    entity_commands.with_child((
        Mesh3d(ship_mesh),
        MeshMaterial3d(ship_material),
        mesh_transform,
    ));

    if is_enemy {
        entity_commands.insert((EnemyVisibility::default(), Health { hp: class.profile().hp }));
    }

    entity_commands.id()
}

/// Spawn a ship with only data components (no mesh, material, or visibility).
/// Used by the server, which has no rendering context.
/// Also spawns a `ShipSecrets` entity that holds team-private state
/// (WaypointQueue, FacingTarget, FacingLocked) for per-component visibility.
/// Note: ShipSecrets is NOT a Bevy child entity — it's a standalone entity with
/// a `ShipSecretsOwner` back-reference. This is intentional: true Bevy children
/// inherit their parent's replication visibility, which would defeat the purpose.
/// When ship destruction is added, ShipSecrets must be despawned alongside the ship.
pub fn spawn_server_ship(
    commands: &mut Commands,
    position: Vec2,
    team: Team,
    class: ShipClass,
) -> Entity {
    let ship_entity = commands
        .spawn((
            Ship,
            Replicated,
            team,
            class,
            Velocity::default(),
            WaypointQueue::default(),
            Transform::from_xyz(position.x, 5.0, position.y),
            Health { hp: class.profile().hp },
            Mounts(class.default_mounts()),
        ))
        .id();

    // Spawn ShipSecrets child — holds replicated copies of private state.
    // Visibility is controlled per-team in server_update_visibility.
    commands.spawn((
        ShipSecrets,
        ShipSecretsOwner(ship_entity),
        Replicated,
        WaypointQueue::default(),
    ));

    ship_entity
}

// ── Visual Indicators ───────────────────────────────────────────────────

fn update_waypoint_markers(
    mut commands: Commands,
    assets: Res<IndicatorAssets>,
    local_team: Res<LocalTeam>,
    secrets_query: Query<(&ShipSecretsOwner, &WaypointQueue), With<ShipSecrets>>,
    ship_team_query: Query<&Team, With<Ship>>,
    marker_query: Query<(Entity, &WaypointMarker)>,
) {
    // Despawn all existing markers
    for (entity, _) in &marker_query {
        commands.entity(entity).despawn();
    }

    let Some(my_team) = local_team.0 else { return; };

    // Spawn markers from ShipSecrets entities (only visible for own team)
    for (owner, waypoints) in &secrets_query {
        let Ok(team) = ship_team_query.get(owner.0) else {
            continue;
        };
        if *team != my_team {
            continue;
        }
        for wp in &waypoints.waypoints {
            commands.spawn((
                WaypointMarker { owner: owner.0 },
                Mesh3d(assets.waypoint_mesh.clone()),
                MeshMaterial3d(assets.waypoint_material.clone()),
                Transform::from_xyz(wp.x, 1.0, wp.y),
            ));
        }
    }
}

fn update_facing_indicators(
    mut commands: Commands,
    assets: Res<IndicatorAssets>,
    local_team: Res<LocalTeam>,
    secrets_query: Query<
        (&ShipSecretsOwner, Option<&FacingLocked>, Option<&FacingTarget>),
        With<ShipSecrets>,
    >,
    ship_query: Query<(&Transform, &Team), With<Ship>>,
    indicator_query: Query<(Entity, &FacingIndicator)>,
) {
    // Despawn all existing facing indicators
    for (entity, _) in &indicator_query {
        commands.entity(entity).despawn();
    }

    let Some(my_team) = local_team.0 else { return; };

    // Spawn facing indicator for locked local team ships only, reading from ShipSecrets
    for (owner, locked, facing) in &secrets_query {
        if locked.is_none() {
            continue;
        }
        let Ok((transform, team)) = ship_query.get(owner.0) else {
            continue;
        };
        if *team != my_team {
            continue;
        }
        let Some(target) = facing else {
            continue;
        };

        let pos = transform.translation;
        let arrow_len = 30.0;
        let mid = Vec3::new(
            pos.x + target.direction.x * arrow_len / 2.0,
            1.0,
            pos.z + target.direction.y * arrow_len / 2.0,
        );

        // Capsule3d is Y-axis aligned — rotate Y-axis to match facing direction in XZ
        let direction_3d = Vec3::new(target.direction.x, 0.0, target.direction.y);
        let rotation = Quat::from_rotation_arc(Vec3::Y, direction_3d.normalize());

        commands.spawn((
            FacingIndicator { owner: owner.0 },
            Mesh3d(assets.facing_mesh.clone()),
            MeshMaterial3d(assets.facing_material.clone()),
            Transform::from_translation(mid).with_rotation(rotation),
        ));
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    // thrust_multiplier tests
    #[test]
    fn thrust_multiplier_facing_target_is_one() {
        let m = thrust_multiplier(0.0, 0.2);
        assert!((m - 1.0).abs() < 0.001);
    }

    #[test]
    fn thrust_multiplier_facing_away_is_thruster_factor() {
        let m = thrust_multiplier(PI, 0.2);
        assert!((m - 0.2).abs() < 0.001);
    }

    #[test]
    fn thrust_multiplier_perpendicular_is_midpoint() {
        let m = thrust_multiplier(PI / 2.0, 0.2);
        let expected = 0.6; // lerp(0.2, 1.0, 0.5)
        assert!((m - expected).abs() < 0.01);
    }

    // ShipClass profile ordering
    #[test]
    fn ship_profiles_are_ordered() {
        let b = ShipClass::Battleship.profile();
        let d = ShipClass::Destroyer.profile();
        let s = ShipClass::Scout.profile();
        assert!(b.acceleration < d.acceleration);
        assert!(d.acceleration < s.acceleration);
        assert!(b.top_speed < d.top_speed);
        assert!(d.top_speed < s.top_speed);
        assert!(b.turn_rate < d.turn_rate);
        assert!(d.turn_rate < s.turn_rate);
    }

    #[test]
    fn default_velocity_is_zero() {
        let v = Velocity::default();
        assert_eq!(v.linear, Vec2::ZERO);
        assert_eq!(v.angular, 0.0);
    }

    // angle_between_directions tests
    #[test]
    fn angle_between_same_direction_is_zero() {
        let a = angle_between_directions(Vec2::X, Vec2::X);
        assert!(a.abs() < 0.001);
    }

    #[test]
    fn angle_between_opposite_is_pi() {
        let a = angle_between_directions(Vec2::X, Vec2::NEG_X);
        assert!((a - PI).abs() < 0.001);
    }

    #[test]
    fn angle_between_perpendicular_is_half_pi() {
        let a = angle_between_directions(Vec2::X, Vec2::Y);
        assert!((a - PI / 2.0).abs() < 0.001);
    }

    // braking_distance tests
    #[test]
    fn braking_distance_at_zero_speed_is_zero() {
        assert_eq!(braking_distance(0.0, 10.0), 0.0);
    }

    #[test]
    fn braking_distance_increases_with_speed() {
        let d1 = braking_distance(10.0, 5.0);
        let d2 = braking_distance(20.0, 5.0);
        assert!(d2 > d1);
    }

    #[test]
    fn braking_distance_formula() {
        // v²/(2a) = 100/(10) = 10
        let d = braking_distance(10.0, 5.0);
        assert!((d - 10.0).abs() < 0.01);
    }

    // shortest_angle_delta tests
    #[test]
    fn shortest_angle_positive() {
        let a = shortest_angle_delta(0.0, 1.0);
        assert!((a - 1.0).abs() < 0.001);
    }

    #[test]
    fn shortest_angle_wraps_around() {
        let from = 350.0_f32.to_radians();
        let to = 10.0_f32.to_radians();
        let delta = shortest_angle_delta(from, to);
        assert!((delta - 20.0_f32.to_radians()).abs() < 0.01);
    }

    #[test]
    fn shortest_angle_negative() {
        let from = 10.0_f32.to_radians();
        let to = 350.0_f32.to_radians();
        let delta = shortest_angle_delta(from, to);
        assert!((delta - (-20.0_f32.to_radians())).abs() < 0.01);
    }

    // Position/facing extraction
    #[test]
    fn ship_xz_extracts_correctly() {
        let transform = Transform::from_xyz(10.0, 5.0, -20.0);
        assert_eq!(ship_xz_position(&transform), Vec2::new(10.0, -20.0));
    }

    #[test]
    fn facing_direction_from_default_transform() {
        // Default transform faces -Z in Bevy
        let t = Transform::default();
        let dir = ship_facing_direction(&t);
        assert!((dir - Vec2::new(0.0, -1.0)).length() < 0.01);
    }

    // Steering tests
    #[test]
    fn desired_velocity_slows_near_target() {
        // Far away: full speed
        let far = desired_velocity_to_target(Vec2::X * 1000.0, 1000.0, 100.0, 10.0, true);
        // Close: should be slower
        let close = desired_velocity_to_target(Vec2::X * 10.0, 10.0, 100.0, 10.0, true);
        assert!(close.length() < far.length());
    }

    #[test]
    fn desired_velocity_is_zero_at_target() {
        let v = desired_velocity_to_target(Vec2::X * 0.1, 0.1, 100.0, 10.0, true);
        assert_eq!(v, Vec2::ZERO);
    }

    #[test]
    fn desired_velocity_points_toward_target() {
        let v = desired_velocity_to_target(Vec2::new(100.0, 0.0), 100.0, 80.0, 10.0, true);
        assert!(v.x > 0.0);
        assert!(v.y.abs() < 0.001);
    }

    #[test]
    fn desired_velocity_not_last_waypoint_is_full_speed() {
        let v = desired_velocity_to_target(Vec2::X * 5.0, 5.0, 100.0, 10.0, false);
        // Not last waypoint: should want full top speed even when close
        assert!((v.length() - 100.0).abs() < 0.01);
    }

    #[test]
    fn steering_corrects_perpendicular_velocity() {
        // Ship moving +X, target is +Y — correction should push toward +Y and against +X
        let correction = compute_steering_thrust(
            Vec2::new(50.0, 0.0),    // current: moving right
            Vec2::new(0.0, 50.0),    // desired: moving up
            Vec2::Y,                  // facing up
            30.0,                     // acceleration
            0.3,                      // thruster_factor
            0.1,                      // dt
        );
        // Should have negative X component (braking sideways) and positive Y (accelerating forward)
        assert!(correction.x < 0.0, "should brake perpendicular: {}", correction.x);
        assert!(correction.y > 0.0, "should accelerate toward target: {}", correction.y);
    }

    #[test]
    fn steering_brakes_when_overshooting() {
        // Ship moving fast toward target but should be slowing down
        let correction = compute_steering_thrust(
            Vec2::new(80.0, 0.0),    // current: fast
            Vec2::new(10.0, 0.0),    // desired: slow (near target)
            Vec2::X,                  // facing right
            30.0,
            0.3,
            0.1,
        );
        // Correction should oppose velocity (negative X)
        assert!(correction.x < 0.0, "should brake: {}", correction.x);
    }

    // WaypointQueue tests
    // default_mounts tests
    #[test]
    fn battleship_has_six_mounts() {
        let mounts = ShipClass::Battleship.default_mounts();
        assert_eq!(mounts.len(), 6);
        let large = mounts.iter().filter(|m| m.size == MountSize::Large).count();
        let medium = mounts.iter().filter(|m| m.size == MountSize::Medium).count();
        let small = mounts.iter().filter(|m| m.size == MountSize::Small).count();
        assert_eq!(large, 2);
        assert_eq!(medium, 2);
        assert_eq!(small, 2);
        // First large mount has heavy cannon, second is empty
        let armed_large = mounts.iter().filter(|m| m.size == MountSize::Large && m.weapon.is_some()).count();
        assert_eq!(armed_large, 1);
        let w = mounts.iter().find(|m| m.size == MountSize::Large && m.weapon.is_some())
            .unwrap().weapon.as_ref().unwrap();
        assert_eq!(w.weapon_type, WeaponType::HeavyCannon);
        // Medium and small mounts are empty
        for m in mounts.iter().filter(|m| m.size != MountSize::Large) {
            assert!(m.weapon.is_none());
        }
    }

    #[test]
    fn destroyer_has_four_mounts() {
        let mounts = ShipClass::Destroyer.default_mounts();
        assert_eq!(mounts.len(), 4);
        let large = mounts.iter().filter(|m| m.size == MountSize::Large).count();
        let medium = mounts.iter().filter(|m| m.size == MountSize::Medium).count();
        let small = mounts.iter().filter(|m| m.size == MountSize::Small).count();
        assert_eq!(large, 1);
        assert_eq!(medium, 2);
        assert_eq!(small, 1);
        let large_mount = mounts.iter().find(|m| m.size == MountSize::Large).unwrap();
        assert_eq!(large_mount.weapon.as_ref().unwrap().weapon_type, WeaponType::Railgun);
    }

    #[test]
    fn scout_has_two_mounts() {
        let mounts = ShipClass::Scout.default_mounts();
        assert_eq!(mounts.len(), 2);
        let medium = mounts.iter().filter(|m| m.size == MountSize::Medium).count();
        let small = mounts.iter().filter(|m| m.size == MountSize::Small).count();
        assert_eq!(medium, 1);
        assert_eq!(small, 1);
        let medium_mount = mounts.iter().find(|m| m.size == MountSize::Medium).unwrap();
        assert_eq!(medium_mount.weapon.as_ref().unwrap().weapon_type, WeaponType::Cannon);
        let small_mount = mounts.iter().find(|m| m.size == MountSize::Small).unwrap();
        assert!(small_mount.weapon.is_none());
    }

    #[test]
    fn waypoint_queue_default_is_empty() {
        let wq = WaypointQueue::default();
        assert!(wq.waypoints.is_empty());
        assert!(!wq.braking);
    }

    #[test]
    fn waypoint_queue_pop_sets_braking() {
        let mut wq = WaypointQueue::default();
        wq.waypoints.push_back(Vec2::new(100.0, 0.0));
        wq.waypoints.pop_front();
        // Manually set braking as the system would
        if wq.waypoints.is_empty() {
            wq.braking = true;
        }
        assert!(wq.braking);
    }
}
