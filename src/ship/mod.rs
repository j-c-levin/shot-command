use bevy::ecs::entity::MapEntities;
use bevy::prelude::*;
use bevy_replicon::prelude::Replicated;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

use crate::game::{GameState, Health, Team};
use crate::net::LocalTeam;
use crate::radar::rwr::RwrBearings;
use crate::radar::RadarActiveSecret;
use crate::weapon::{MissileQueue, Mount, MountSize, Mounts, WeaponState, WeaponType};

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
        app.add_systems(
            Update,
            (draw_waypoint_gizmos, draw_facing_gizmos)
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
    pub rcs: f32,
}

impl ShipClass {
    /// Fixed mount layout for this ship class — sizes and positions.
    /// This defines what slots exist; weapons are assigned separately.
    pub fn mount_layout(&self) -> Vec<(MountSize, Vec2)> {
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
    pub fn default_loadout(&self) -> Vec<Option<WeaponType>> {
        match self {
            ShipClass::Battleship => vec![
                Some(WeaponType::HeavyCannon), // Large
                Some(WeaponType::HeavyVLS),    // Large
                Some(WeaponType::LightVLS),    // Medium
                Some(WeaponType::LaserPD),     // Medium
                Some(WeaponType::CWIS),        // Small
                Some(WeaponType::CWIS),        // Small
            ],
            ShipClass::Destroyer => vec![
                Some(WeaponType::Railgun),     // Large
                Some(WeaponType::Cannon),      // Medium
                Some(WeaponType::LaserPD),     // Medium
                Some(WeaponType::CWIS),        // Small
            ],
            ShipClass::Scout => vec![
                Some(WeaponType::Cannon),      // Medium
                Some(WeaponType::CWIS),        // Small
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
                        ammo: 0,
                        cooldown: 0.0,
                        pd_retarget_cooldown: 0.0,
                        tubes_loaded: profile.tubes,
                        tube_reload_timer: 0.0,
                        fire_delay: 0.0,
                    }
                }),
            })
            .collect()
    }

    /// Create the mesh and child transform for this ship class.
    /// Centralizes the mesh geometry so materializer and fog ghost code don't duplicate it.
    pub fn create_mesh(&self, meshes: &mut Assets<Mesh>) -> (Handle<Mesh>, Transform) {
        let mesh = match self {
            ShipClass::Battleship => meshes.add(Cuboid::new(12.0, 8.0, 28.0)),
            ShipClass::Destroyer => meshes.add(Cone {
                radius: 8.0,
                height: 20.0,
            }),
            ShipClass::Scout => meshes.add(Sphere::new(1.0).mesh().uv(16, 16)),
        };

        let child_transform = match self {
            ShipClass::Battleship => Transform::IDENTITY,
            ShipClass::Destroyer => {
                Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2))
            }
            ShipClass::Scout => Transform::from_scale(Vec3::new(4.0, 3.0, 7.0)),
        };

        (mesh, child_transform)
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
                vision_range: 400.0,
                collision_radius: 12.0,
                rcs: 1.0,
            },
            ShipClass::Destroyer => ShipProfile {
                hp: 100,
                acceleration: 10.0,
                thruster_factor: 0.3,
                turn_rate: 1.5,
                turn_acceleration: 1.0,
                top_speed: 28.0,
                vision_range: 400.0,
                collision_radius: 8.0,
                rcs: 0.5,
            },
            ShipClass::Scout => ShipProfile {
                hp: 50,
                acceleration: 14.0,
                thruster_factor: 0.5,
                turn_rate: 3.0,
                turn_acceleration: 2.0,
                top_speed: 35.0,
                vision_range: 400.0,
                collision_radius: 5.0,
                rcs: 0.25,
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

/// Numeric identifier for a ship within its team (1-9).
/// Used for number-key selection. Replicated via ShipSecrets.
#[derive(Component, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ShipNumber(pub u8);

/// Marks a ship as a squad follower. The `leader` entity is the squad leader,
/// and `offset` is the XZ position offset from the leader at the time of joining.
/// Uses `#[entities]` for replicon entity mapping across server/client.
#[derive(Component, Clone, Debug, Serialize, Deserialize, MapEntities)]
pub struct SquadMember {
    #[entities]
    pub leader: Entity,
    pub offset: Vec2,
}

/// Caps a ship's effective movement to the slowest member in its squad.
/// Stores the minimum values across all squad members for speed, acceleration,
/// and turn rate so that all members move, accelerate, and turn identically.
#[derive(Component, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct SquadSpeedLimit {
    pub top_speed: f32,
    pub acceleration: f32,
    pub turn_rate: f32,
    pub turn_acceleration: f32,
}

#[derive(Component)]
pub struct Selected;


/// Marker for the child entity that holds team-private ship state.
/// This entity is only visible to the owning team via replicon visibility,
/// preventing enemies from seeing WaypointQueue, FacingTarget, and FacingLocked.
#[derive(Component, Serialize, Deserialize)]
pub struct ShipSecrets;

/// Points from a ShipSecrets entity back to its parent Ship entity.
/// Uses `#[entities]` for replicon entity mapping across server/client.
#[derive(Component, Serialize, Deserialize, MapEntities)]
pub struct ShipSecretsOwner(#[entities] pub Entity);


// ── Pure Functions ──────────────────────────────────────────────────────

/// Rotate a 2D offset vector by the given angle (radians).
pub fn rotate_offset(offset: Vec2, angle: f32) -> Vec2 {
    let (sin, cos) = angle.sin_cos();
    Vec2::new(offset.x * cos - offset.y * sin, offset.x * sin + offset.y * cos)
}

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
        (&mut Transform, &mut Velocity, &ShipClass, Option<&FacingTarget>, Option<&SquadSpeedLimit>),
        With<Ship>,
    >,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }

    for (mut transform, mut velocity, class, facing_target, speed_limit) in &mut query {
        let profile = class.profile();
        // Cap turn rate/acceleration to squad minimum when in a squad
        let (effective_turn_rate, effective_turn_accel) = if let Some(limit) = speed_limit {
            (limit.turn_rate.min(profile.turn_rate), limit.turn_acceleration.min(profile.turn_acceleration))
        } else {
            (profile.turn_rate, profile.turn_acceleration)
        };

        let Some(target) = facing_target else {
            // No target — decelerate angular velocity to zero
            if velocity.angular.abs() > 0.001 {
                let decel = effective_turn_accel * dt;
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
        let stop_distance = braking_distance(velocity.angular.abs(), effective_turn_accel);
        let should_brake = stop_distance >= delta.abs();

        if should_brake {
            let decel = effective_turn_accel * dt;
            if velocity.angular.abs() <= decel {
                velocity.angular = 0.0;
            } else {
                velocity.angular -= velocity.angular.signum() * decel;
            }
        } else {
            let desired_sign = delta.signum();
            velocity.angular += desired_sign * effective_turn_accel * dt;
            velocity.angular = velocity.angular.clamp(-effective_turn_rate, effective_turn_rate);
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
        (&Transform, &mut Velocity, &ShipClass, &WaypointQueue, Option<&SquadSpeedLimit>),
        With<Ship>,
    >,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }

    for (transform, mut velocity, class, waypoints, speed_limit) in &mut query {
        let profile = class.profile();
        let facing = ship_facing_direction(transform);
        let speed = velocity.linear.length();

        // Effective top speed and acceleration: capped by squad speed limit if present.
        let (effective_top_speed, effective_acceleration) = if let Some(limit) = speed_limit {
            (limit.top_speed.min(profile.top_speed), limit.acceleration.min(profile.acceleration))
        } else {
            (profile.top_speed, profile.acceleration)
        };

        // No waypoints — always brake to a stop
        if waypoints.waypoints.is_empty() {
            if speed > 0.1 {
                let correction = compute_steering_thrust(
                    velocity.linear,
                    Vec2::ZERO, // desired: stop
                    facing,
                    effective_acceleration,
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
        let min_decel = effective_acceleration * profile.thruster_factor;

        let desired = desired_velocity_to_target(
            to_target,
            dist,
            effective_top_speed,
            min_decel,
            is_last,
        );

        let correction = compute_steering_thrust(
            velocity.linear,
            desired,
            facing,
            effective_acceleration,
            profile.thruster_factor,
            dt,
        );

        velocity.linear += correction;

        // Clamp to effective top speed
        let new_speed = velocity.linear.length();
        if new_speed > effective_top_speed {
            velocity.linear = velocity.linear.normalize() * effective_top_speed;
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

/// Spawn a ship from a `ShipSpec` (hull class + weapon loadout).
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
    spec: &crate::fleet::ShipSpec,
    ship_number: u8,
) -> Entity {
    let class = spec.class;
    let layout = class.mount_layout();
    let mounts: Vec<Mount> = layout
        .into_iter()
        .zip(spec.loadout.iter())
        .map(|((size, offset), weapon_opt)| Mount {
            size,
            offset,
            weapon: weapon_opt.map(|wt| {
                let profile = wt.profile();
                WeaponState {
                    weapon_type: wt,
                    ammo: 0,
                    cooldown: 0.0,
                    pd_retarget_cooldown: 0.0,
                    tubes_loaded: profile.tubes,
                    tube_reload_timer: 0.0,
                    fire_delay: 0.0,
                }
            }),
        })
        .collect();

    let ship_entity = commands
        .spawn((
            Ship,
            Replicated,
            team,
            class,
            Velocity::default(),
            WaypointQueue::default(),
            MissileQueue::default(),
            Transform::from_xyz(position.x, 5.0, position.y),
            Health { hp: class.profile().hp },
            Mounts(mounts),
        ))
        .id();

    // Spawn ShipSecrets child — holds replicated copies of private state.
    // Visibility is controlled per-team in server_update_visibility.
    commands.spawn((
        ShipSecrets,
        ShipSecretsOwner(ship_entity),
        Replicated,
        WaypointQueue::default(),
        MissileQueue::default(),
        ShipNumber(ship_number),
        RadarActiveSecret(false),
        RwrBearings::default(),
    ));

    ship_entity
}

/// Convenience: spawn a ship with the default loadout for its class.
/// Used for testing and fallback scenarios.
pub fn spawn_server_ship_default(
    commands: &mut Commands,
    position: Vec2,
    team: Team,
    class: ShipClass,
) -> Entity {
    let spec = crate::fleet::ShipSpec {
        class,
        loadout: class.default_loadout(),
    };
    spawn_server_ship(commands, position, team, &spec, 0)
}

// ── Visual Indicators (Gizmos) ──────────────────────────────────────────

/// Draw blue gizmo lines from selected friendly ships to their waypoints.
fn draw_waypoint_gizmos(
    mut gizmos: Gizmos,
    local_team: Res<LocalTeam>,
    secrets_query: Query<(&ShipSecretsOwner, &WaypointQueue), With<ShipSecrets>>,
    ship_query: Query<(&Transform, &Team), (With<Ship>, With<Selected>)>,
) {
    let Some(my_team) = local_team.0 else { return };

    let line_color = Color::srgba(0.3, 0.5, 1.0, 0.7);

    for (owner, waypoints) in &secrets_query {
        let Ok((transform, team)) = ship_query.get(owner.0) else {
            continue;
        };
        if *team != my_team {
            continue;
        }
        if waypoints.waypoints.is_empty() {
            continue;
        }

        let ship_pos = Vec3::new(transform.translation.x, 1.0, transform.translation.z);
        let mut prev = ship_pos;
        for wp in &waypoints.waypoints {
            let wp_pos = Vec3::new(wp.x, 1.0, wp.y);
            gizmos.line(prev, wp_pos, line_color);
            prev = wp_pos;
        }
    }
}

/// Draw yellow gizmo lines showing facing lock direction for selected friendly ships.
fn draw_facing_gizmos(
    mut gizmos: Gizmos,
    local_team: Res<LocalTeam>,
    secrets_query: Query<
        (&ShipSecretsOwner, Option<&FacingLocked>, Option<&FacingTarget>),
        With<ShipSecrets>,
    >,
    ship_query: Query<(&Transform, &Team), (With<Ship>, With<Selected>)>,
) {
    let Some(my_team) = local_team.0 else { return };

    let line_color = Color::srgba(1.0, 0.9, 0.2, 0.7);

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

        let pos = Vec3::new(transform.translation.x, 5.0, transform.translation.z);
        let end = Vec3::new(
            pos.x + target.direction.x * 30.0,
            5.0,
            pos.z + target.direction.y * 30.0,
        );
        gizmos.line(pos, end, line_color);
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
        // All mounts armed
        assert!(mounts.iter().all(|m| m.weapon.is_some()));
        // Check specific weapon types by index
        assert_eq!(mounts[0].weapon.as_ref().unwrap().weapon_type, WeaponType::HeavyCannon);
        assert_eq!(mounts[1].weapon.as_ref().unwrap().weapon_type, WeaponType::HeavyVLS);
        assert_eq!(mounts[2].weapon.as_ref().unwrap().weapon_type, WeaponType::LightVLS);
        assert_eq!(mounts[3].weapon.as_ref().unwrap().weapon_type, WeaponType::LaserPD);
        assert_eq!(mounts[4].weapon.as_ref().unwrap().weapon_type, WeaponType::CWIS);
        assert_eq!(mounts[5].weapon.as_ref().unwrap().weapon_type, WeaponType::CWIS);
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
        // All mounts armed
        assert!(mounts.iter().all(|m| m.weapon.is_some()));
        assert_eq!(mounts[0].weapon.as_ref().unwrap().weapon_type, WeaponType::Railgun);
        assert_eq!(mounts[1].weapon.as_ref().unwrap().weapon_type, WeaponType::Cannon);
        assert_eq!(mounts[2].weapon.as_ref().unwrap().weapon_type, WeaponType::LaserPD);
        assert_eq!(mounts[3].weapon.as_ref().unwrap().weapon_type, WeaponType::CWIS);
    }

    #[test]
    fn scout_has_two_mounts() {
        let mounts = ShipClass::Scout.default_mounts();
        assert_eq!(mounts.len(), 2);
        let medium = mounts.iter().filter(|m| m.size == MountSize::Medium).count();
        let small = mounts.iter().filter(|m| m.size == MountSize::Small).count();
        assert_eq!(medium, 1);
        assert_eq!(small, 1);
        // All mounts armed
        assert!(mounts.iter().all(|m| m.weapon.is_some()));
        assert_eq!(mounts[0].weapon.as_ref().unwrap().weapon_type, WeaponType::Cannon);
        assert_eq!(mounts[1].weapon.as_ref().unwrap().weapon_type, WeaponType::CWIS);
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

    #[test]
    fn ship_heading_from_default_transform() {
        let transform = Transform::default();
        let heading = ship_heading(&transform);
        assert!(heading.is_finite(), "heading should be a finite number");
        let expected = -PI / 2.0;
        assert!(
            (heading - expected).abs() < 0.01,
            "default transform faces -Z, heading should be -PI/2 (~{:.3}), got {:.3}",
            expected,
            heading
        );
    }

    // ── Squad & ShipNumber tests ─────────────────────────────────────────

    #[test]
    fn squad_offset_computation() {
        // Offset = ship_pos - leader_pos
        let ship_pos = Vec2::new(100.0, 200.0);
        let leader_pos = Vec2::new(50.0, 150.0);
        let offset = ship_pos - leader_pos;
        assert_eq!(offset, Vec2::new(50.0, 50.0));
    }

    #[test]
    fn squad_offset_negative() {
        let ship_pos = Vec2::new(-50.0, -100.0);
        let leader_pos = Vec2::new(50.0, 100.0);
        let offset = ship_pos - leader_pos;
        assert_eq!(offset, Vec2::new(-100.0, -200.0));
    }

    #[test]
    fn squad_move_destination() {
        // Follower destination = leader_destination + follower_offset
        let leader_dest = Vec2::new(300.0, 400.0);
        let offset = Vec2::new(50.0, -30.0);
        let follower_dest = leader_dest + offset;
        assert_eq!(follower_dest, Vec2::new(350.0, 370.0));
    }

    #[test]
    fn squad_move_destination_with_zero_offset() {
        let leader_dest = Vec2::new(100.0, 200.0);
        let offset = Vec2::ZERO;
        let follower_dest = leader_dest + offset;
        assert_eq!(follower_dest, leader_dest);
    }

    #[test]
    fn ship_number_assignment() {
        // Ship spec index 0 → ShipNumber(1), index 1 → ShipNumber(2), etc.
        for i in 0..9u8 {
            let number = (i + 1) as u8;
            let sn = ShipNumber(number);
            assert_eq!(sn.0, i + 1);
        }
    }

    #[test]
    fn ship_number_default_is_zero() {
        // Default ships (not from fleet list) get ShipNumber(0)
        let sn = ShipNumber(0);
        assert_eq!(sn.0, 0);
    }

    #[test]
    fn squad_speed_limit_caps_effective_speed() {
        // Scout top_speed is much higher than battleship
        let scout = ShipClass::Scout.profile();
        let bb = ShipClass::Battleship.profile();
        assert!(scout.top_speed > bb.top_speed);

        // With squad speed limit set to battleship stats, all are capped
        let limit = SquadSpeedLimit {
            top_speed: bb.top_speed,
            acceleration: bb.acceleration,
            turn_rate: bb.turn_rate,
            turn_acceleration: bb.turn_acceleration,
        };
        assert!((limit.top_speed.min(scout.top_speed) - bb.top_speed).abs() < 0.01);
        assert!((limit.acceleration.min(scout.acceleration) - bb.acceleration).abs() < 0.01);
        assert!((limit.turn_rate.min(scout.turn_rate) - bb.turn_rate).abs() < 0.01);
    }

    #[test]
    fn squad_speed_limit_no_effect_when_slower() {
        // Battleship is already slower than a very high limit
        let bb = ShipClass::Battleship.profile();
        let limit = SquadSpeedLimit {
            top_speed: 1000.0,
            acceleration: 1000.0,
            turn_rate: 1000.0,
            turn_acceleration: 1000.0,
        };
        assert!((limit.top_speed.min(bb.top_speed) - bb.top_speed).abs() < 0.01);
    }

    #[test]
    fn battleship_rcs_largest() {
        let bb = ShipClass::Battleship.profile();
        let dd = ShipClass::Destroyer.profile();
        let sc = ShipClass::Scout.profile();
        assert!(bb.rcs > dd.rcs);
        assert!(dd.rcs > sc.rcs);
    }

    #[test]
    fn rcs_values_positive() {
        for class in [ShipClass::Battleship, ShipClass::Destroyer, ShipClass::Scout] {
            assert!(class.profile().rcs > 0.0);
            assert!(class.profile().rcs <= 1.0);
        }
    }
}
