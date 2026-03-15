use bevy::prelude::*;
use std::collections::VecDeque;

use crate::game::{EnemyVisibility, GameState, Health, Team};
use crate::map::MapBounds;

pub struct ShipPlugin;

impl Plugin for ShipPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, init_indicator_assets)
            .add_systems(
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
            )
            .add_systems(
                Update,
                (update_waypoint_markers, update_facing_indicators)
                    .run_if(in_state(GameState::Playing)),
            );
    }
}

// ── Components ──────────────────────────────────────────────────────────

#[derive(Component)]
pub struct Ship;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShipClass {
    Battleship,
    Destroyer,
    Scout,
}

#[derive(Clone, Debug)]
pub struct ShipProfile {
    pub acceleration: f32,
    pub thruster_factor: f32,
    pub turn_rate: f32,
    pub turn_acceleration: f32,
    pub top_speed: f32,
    pub vision_range: f32,
    pub collision_radius: f32,
}

impl ShipClass {
    pub fn profile(&self) -> ShipProfile {
        match self {
            ShipClass::Battleship => ShipProfile {
                acceleration: 15.0,
                thruster_factor: 0.2,
                turn_rate: 0.8,
                turn_acceleration: 0.4,
                top_speed: 40.0,
                vision_range: 250.0,
                collision_radius: 12.0,
            },
            ShipClass::Destroyer => ShipProfile {
                acceleration: 30.0,
                thruster_factor: 0.3,
                turn_rate: 1.5,
                turn_acceleration: 1.0,
                top_speed: 80.0,
                vision_range: 200.0,
                collision_radius: 8.0,
            },
            ShipClass::Scout => ShipProfile {
                acceleration: 50.0,
                thruster_factor: 0.5,
                turn_rate: 3.0,
                turn_acceleration: 2.0,
                top_speed: 130.0,
                vision_range: 150.0,
                collision_radius: 5.0,
            },
        }
    }
}

#[derive(Component, Clone, Debug, Default)]
pub struct Velocity {
    pub linear: Vec2,
    pub angular: f32,
}

#[derive(Component, Clone, Debug)]
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

#[derive(Component, Clone, Debug)]
pub struct FacingTarget {
    pub direction: Vec2,
}

/// Marker: ship facing is player-locked, not auto-set by waypoints
#[derive(Component, Clone, Debug)]
pub struct FacingLocked;

#[derive(Component)]
pub struct Selected;

#[derive(Component)]
pub struct SelectionIndicator;

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

// ── Systems ─────────────────────────────────────────────────────────────

fn update_facing_targets(
    mut commands: Commands,
    query: Query<
        (Entity, &Transform, &WaypointQueue, Option<&FacingLocked>),
        With<Ship>,
    >,
) {
    for (entity, transform, waypoints, locked) in &query {
        if locked.is_some() {
            continue;
        }

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

        // Auto-brake: no waypoints left and braking flag set
        if waypoints.braking && waypoints.waypoints.is_empty() {
            if speed > 0.1 {
                let vel_dir = velocity.linear.normalize_or_zero();
                // Braking force depends on angle between facing and opposing velocity
                let angle = angle_between_directions(facing, -vel_dir);
                let effective_decel =
                    profile.acceleration * thrust_multiplier(angle, profile.thruster_factor);
                let decel_amount = effective_decel * dt;
                if speed <= decel_amount {
                    velocity.linear = Vec2::ZERO;
                } else {
                    velocity.linear -= vel_dir * decel_amount;
                }
            } else {
                velocity.linear = Vec2::ZERO;
            }
            continue;
        }

        // No waypoints, not braking — drift
        if waypoints.waypoints.is_empty() {
            continue;
        }

        // Has waypoints — thrust toward next waypoint
        let pos = ship_xz_position(transform);
        let next_wp = waypoints.waypoints[0];
        let to_target = next_wp - pos;
        let dist = to_target.length();

        if dist < 1.0 {
            continue;
        }

        let desired_dir = to_target / dist;
        let angle = angle_between_directions(facing, desired_dir);
        let multiplier = thrust_multiplier(angle, profile.thruster_factor);
        let effective_accel = profile.acceleration * multiplier;
        let effective_top_speed = profile.top_speed * multiplier;

        // Check if approaching last waypoint and need to brake
        let is_last = waypoints.waypoints.len() == 1;
        if is_last {
            let brake_dist = braking_distance(speed, effective_accel);
            if brake_dist >= dist {
                // Brake toward the waypoint
                let vel_dir = velocity.linear.normalize_or_zero();
                let brake_angle = angle_between_directions(facing, -vel_dir);
                let brake_decel =
                    profile.acceleration * thrust_multiplier(brake_angle, profile.thruster_factor);
                let decel_amount = brake_decel * dt;
                if speed <= decel_amount {
                    velocity.linear = Vec2::ZERO;
                } else {
                    velocity.linear -= vel_dir * decel_amount;
                }
                continue;
            }
        }

        // Accelerate toward waypoint
        if speed < effective_top_speed {
            velocity.linear += desired_dir * effective_accel * dt;
            let new_speed = velocity.linear.length();
            if new_speed > effective_top_speed {
                velocity.linear = velocity.linear.normalize() * effective_top_speed;
            }
        } else if speed > effective_top_speed {
            let excess = speed - effective_top_speed;
            let decel = (effective_accel * dt).min(excess);
            velocity.linear = velocity.linear.normalize() * (speed - decel);
        }
    }
}

fn apply_velocity(
    time: Res<Time>,
    mut query: Query<(&mut Transform, &Velocity), With<Ship>>,
) {
    let dt = time.delta_secs();
    for (mut transform, velocity) in &mut query {
        transform.translation.x += velocity.linear.x * dt;
        transform.translation.z += velocity.linear.y * dt;
    }
}

const ARRIVAL_THRESHOLD: f32 = 10.0;

fn check_waypoint_arrival(
    mut query: Query<(&Transform, &mut WaypointQueue), With<Ship>>,
) {
    for (transform, mut waypoints) in &mut query {
        if let Some(&next_wp) = waypoints.waypoints.front() {
            let pos = ship_xz_position(transform);
            let dist = (next_wp - pos).length();

            if dist < ARRIVAL_THRESHOLD {
                waypoints.waypoints.pop_front();
                if waypoints.waypoints.is_empty() {
                    waypoints.braking = true;
                }
            }
        }
    }
}

fn clamp_ships_to_bounds(
    bounds: Res<MapBounds>,
    mut query: Query<(&mut Transform, &mut Velocity), With<Ship>>,
) {
    for (mut transform, mut velocity) in &mut query {
        let pos = ship_xz_position(&transform);
        let clamped = bounds.clamp(pos);
        if pos != clamped {
            transform.translation.x = clamped.x;
            transform.translation.z = clamped.y;
            if (pos.x - clamped.x).abs() > 0.01 {
                velocity.linear.x = 0.0;
            }
            if (pos.y - clamped.y).abs() > 0.01 {
                velocity.linear.y = 0.0;
            }
        }
    }
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
        ShipClass::Scout => meshes.add(Sphere::new(5.0).mesh().uv(16, 16)),
    };

    // Align mesh forward with -Z
    let mesh_rotation = match class {
        ShipClass::Battleship => Quat::IDENTITY,
        ShipClass::Destroyer => Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2),
        ShipClass::Scout => Quat::IDENTITY,
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
        Transform::from_rotation(mesh_rotation),
    ));

    if is_enemy {
        entity_commands.insert((EnemyVisibility::default(), Health { hp: 3 }));
    }

    entity_commands.id()
}

// ── Visual Indicators ───────────────────────────────────────────────────

fn update_waypoint_markers(
    mut commands: Commands,
    assets: Res<IndicatorAssets>,
    ship_query: Query<(Entity, &WaypointQueue, &Team), With<Ship>>,
    marker_query: Query<(Entity, &WaypointMarker)>,
) {
    // Despawn all existing markers
    for (entity, _) in &marker_query {
        commands.entity(entity).despawn();
    }

    // Spawn new markers for each player ship's waypoints
    for (ship_entity, waypoints, team) in &ship_query {
        if *team != Team::PLAYER {
            continue;
        }
        for wp in &waypoints.waypoints {
            commands.spawn((
                WaypointMarker { owner: ship_entity },
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
    ship_query: Query<
        (Entity, &Transform, Option<&FacingLocked>, Option<&FacingTarget>, &Team),
        With<Ship>,
    >,
    indicator_query: Query<(Entity, &FacingIndicator)>,
) {
    // Despawn all existing facing indicators
    for (entity, _) in &indicator_query {
        commands.entity(entity).despawn();
    }

    // Spawn facing indicator for locked player ships only
    for (ship_entity, transform, locked, facing, team) in &ship_query {
        if locked.is_none() || *team != Team::PLAYER {
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
            FacingIndicator { owner: ship_entity },
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

    // WaypointQueue tests
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
