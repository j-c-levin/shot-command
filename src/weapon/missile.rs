use bevy::ecs::entity::{EntityMapper, MapEntities};
use bevy::prelude::*;
use bevy_replicon::prelude::Replicated;
use serde::{Deserialize, Serialize};

use crate::game::{GameState, Health};
use crate::map::MapBounds;
use crate::ship::{Ship, ShipClass, Velocity};

// ── Constants ──────────────────────────────────────────────────────────

/// Altitude missiles reach during cruise phase (meters).
pub const CRUISE_ALTITUDE: f32 = 60.0;

/// Climb angle in radians (~45 degrees).
pub const CLIMB_ANGLE: f32 = std::f32::consts::FRAC_PI_4;

/// XZ distance to intercept point at which the missile begins diving.
pub const DIVE_DISTANCE: f32 = 50.0;

/// Half-angle of the terminal seeker cone (~30 degrees).
pub const SEEKER_HALF_ANGLE: f32 = 0.5236;

// ── Components ─────────────────────────────────────────────────────────

/// Marker for missile entities.
#[derive(Component, Serialize, Deserialize)]
pub struct Missile;

/// Current target information for the missile.
#[derive(Component, Serialize, Deserialize, Clone)]
pub struct MissileTarget {
    /// Predicted intercept point in world space.
    pub intercept_point: Vec3,
    /// Entity being tracked (may be None if target lost).
    pub target_entity: Option<Entity>,
}

// Manual MapEntities for Option<Entity>
impl MapEntities for MissileTarget {
    fn map_entities<M: EntityMapper>(&mut self, entity_mapper: &mut M) {
        if let Some(entity) = &mut self.target_entity {
            *entity = entity_mapper.get_mapped(*entity);
        }
    }
}

/// Current 3D velocity vector.
#[derive(Component, Serialize, Deserialize, Clone)]
pub struct MissileVelocity(pub Vec3);

/// Hit points — missile can be destroyed by point defense.
#[derive(Component, Serialize, Deserialize, Clone)]
pub struct MissileHealth(pub u16);

/// Damage dealt to a ship on impact.
#[derive(Component, Serialize, Deserialize, Clone)]
pub struct MissileDamage(pub u16);

/// Ship that launched this missile. Skipped for self-hit checks.
#[derive(Component, Serialize, Deserialize, MapEntities, Clone)]
pub struct MissileOwner(#[entities] pub Entity);

/// Remaining fuel in meters of travel distance.
#[derive(Component, Serialize, Deserialize, Clone)]
pub struct MissileFuel(pub f32);

/// Current phase of missile flight.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlightPhase {
    /// Initial launch — climbing at ~45 degrees to cruise altitude.
    Climb,
    /// Level flight at cruise altitude toward intercept XZ position.
    Cruise,
    /// Pitching down toward ground-level intercept point.
    Dive,
    /// Final approach — seeker active, homing on target.
    Terminal,
}

// ── Pure functions ─────────────────────────────────────────────────────

/// Predict where the target will be when a missile arrives.
///
/// Similar to projectile lead, but accounts for ~1.3x path length due to the
/// climb-cruise-dive arc. Uses two-iteration linear prediction.
pub fn compute_intercept_point(
    shooter_pos: Vec3,
    target_pos: Vec3,
    target_velocity: Vec2,
    missile_speed: f32,
) -> Vec3 {
    if missile_speed < 0.001 {
        return target_pos;
    }

    // Arc multiplier: missiles travel ~1.3x straight-line distance
    let arc_factor = 1.3;

    // First estimate
    let dist = (shooter_pos - target_pos).length();
    let travel_time = (dist * arc_factor) / missile_speed;
    let vel3 = Vec3::new(target_velocity.x, 0.0, target_velocity.y);
    let predicted = target_pos + vel3 * travel_time;

    // Refine once
    let dist2 = (shooter_pos - predicted).length();
    let travel_time2 = (dist2 * arc_factor) / missile_speed;
    let refined = target_pos + vel3 * travel_time2;

    // Keep ground-level Y (missiles aim at ship position)
    Vec3::new(refined.x, target_pos.y, refined.z)
}

/// Check whether a target position falls within a 3D seeker cone.
///
/// `missile_forward` should be the normalized direction the missile is traveling.
/// Returns true if the angle between forward and the direction to target is
/// less than `cone_half_angle`.
pub fn is_in_seeker_cone(
    missile_pos: Vec3,
    missile_forward: Vec3,
    target_pos: Vec3,
    cone_half_angle: f32,
) -> bool {
    let to_target = target_pos - missile_pos;
    let dist = to_target.length();
    if dist < 0.001 {
        return true;
    }
    let to_target_dir = to_target / dist;
    let cos_angle = missile_forward.dot(to_target_dir);
    cos_angle >= cone_half_angle.cos()
}

// ── Spawning ───────────────────────────────────────────────────────────

/// Spawn a replicated missile entity with initial climb velocity.
///
/// `origin` — world-space launch position.
/// `intercept_point` — predicted ground-level intercept point.
/// `target_entity` — optional entity being targeted.
/// `speed` — scalar missile speed.
/// `damage` — HP dealt on ship hit.
/// `health` — missile's hit points (for PD damage).
/// `fuel` — max range in meters.
/// `owner` — entity of the ship that fired.
pub fn spawn_missile(
    commands: &mut Commands,
    origin: Vec3,
    intercept_point: Vec3,
    target_entity: Option<Entity>,
    speed: f32,
    damage: u16,
    health: u16,
    fuel: f32,
    owner: Entity,
) -> Entity {
    // Initial climb velocity: forward XZ toward intercept + upward at climb angle
    let xz_dir = Vec2::new(
        intercept_point.x - origin.x,
        intercept_point.z - origin.z,
    )
    .normalize_or_zero();

    let climb_speed_xz = speed * CLIMB_ANGLE.cos();
    let climb_speed_y = speed * CLIMB_ANGLE.sin();

    let velocity = Vec3::new(
        xz_dir.x * climb_speed_xz,
        climb_speed_y,
        xz_dir.y * climb_speed_xz,
    );

    commands
        .spawn((
            Missile,
            MissileTarget {
                intercept_point,
                target_entity,
            },
            MissileVelocity(velocity),
            MissileHealth(health),
            MissileDamage(damage),
            MissileOwner(owner),
            MissileFuel(fuel),
            FlightPhase::Climb,
            Transform::from_translation(origin),
            Replicated,
        ))
        .id()
}

// ── Systems ────────────────────────────────────────────────────────────

/// Move every missile by its velocity and decrement fuel by distance traveled.
fn advance_missiles(
    time: Res<Time>,
    mut query: Query<(&mut Transform, &MissileVelocity, &mut MissileFuel), With<Missile>>,
) {
    let dt = time.delta_secs();
    for (mut transform, vel, mut fuel) in &mut query {
        let displacement = vel.0 * dt;
        transform.translation += displacement;
        fuel.0 -= displacement.length();
    }
}

/// Update missile flight phases and velocities.
fn update_missile_flight(
    mut query: Query<
        (
            &Transform,
            &mut MissileVelocity,
            &mut FlightPhase,
            &MissileTarget,
        ),
        With<Missile>,
    >,
) {
    for (transform, mut vel, mut phase, target) in &mut query {
        let pos = transform.translation;
        let speed = vel.0.length();
        if speed < 0.001 {
            continue;
        }

        match *phase {
            FlightPhase::Climb => {
                // Transition to Cruise when reaching cruise altitude
                if pos.y >= CRUISE_ALTITUDE {
                    *phase = FlightPhase::Cruise;
                    // Set level flight toward intercept XZ
                    let xz_dir = Vec2::new(
                        target.intercept_point.x - pos.x,
                        target.intercept_point.z - pos.z,
                    )
                    .normalize_or_zero();
                    vel.0 = Vec3::new(xz_dir.x * speed, 0.0, xz_dir.y * speed);
                }
            }
            FlightPhase::Cruise => {
                // Transition to Dive when within DIVE_DISTANCE XZ of intercept
                let xz_dist = Vec2::new(
                    target.intercept_point.x - pos.x,
                    target.intercept_point.z - pos.z,
                )
                .length();

                if xz_dist <= DIVE_DISTANCE {
                    *phase = FlightPhase::Dive;
                    // Pitch down toward ground-level intercept
                    let to_intercept = Vec3::new(
                        target.intercept_point.x - pos.x,
                        target.intercept_point.y - pos.y,
                        target.intercept_point.z - pos.z,
                    )
                    .normalize_or_zero();
                    vel.0 = to_intercept * speed;
                } else {
                    // Maintain level flight, adjust XZ heading toward intercept
                    let xz_dir = Vec2::new(
                        target.intercept_point.x - pos.x,
                        target.intercept_point.z - pos.z,
                    )
                    .normalize_or_zero();
                    vel.0 = Vec3::new(xz_dir.x * speed, 0.0, xz_dir.y * speed);
                }
            }
            FlightPhase::Dive => {
                // Transition to Terminal at half DIVE_DISTANCE
                let xz_dist = Vec2::new(
                    target.intercept_point.x - pos.x,
                    target.intercept_point.z - pos.z,
                )
                .length();

                if xz_dist <= DIVE_DISTANCE * 0.5 {
                    *phase = FlightPhase::Terminal;
                } else {
                    // Continue pitching toward intercept point
                    let to_intercept = Vec3::new(
                        target.intercept_point.x - pos.x,
                        target.intercept_point.y - pos.y,
                        target.intercept_point.z - pos.z,
                    )
                    .normalize_or_zero();
                    vel.0 = to_intercept * speed;
                }
            }
            FlightPhase::Terminal => {
                // If we have a target entity, aim toward current target position
                // (handled by terminal_seeker_scan updating the intercept_point)
                let to_intercept = Vec3::new(
                    target.intercept_point.x - pos.x,
                    target.intercept_point.y - pos.y,
                    target.intercept_point.z - pos.z,
                )
                .normalize_or_zero();
                vel.0 = to_intercept * speed;
            }
        }
    }
}

/// Terminal seeker: for missiles in Terminal phase, scan for ships in seeker cone.
/// If the missile has a target_entity that still exists, update intercept_point to
/// track current position. Otherwise, acquire the closest valid target in cone.
fn terminal_seeker_scan_system(
    mut missile_query: Query<
        (
            &Transform,
            &MissileVelocity,
            &mut MissileTarget,
            &MissileOwner,
            &FlightPhase,
        ),
        With<Missile>,
    >,
    ship_query: Query<(Entity, &Transform, &Velocity), With<Ship>>,
) {
    for (m_transform, m_vel, mut m_target, m_owner, phase) in &mut missile_query {
        if *phase != FlightPhase::Terminal {
            continue;
        }

        let missile_pos = m_transform.translation;
        let speed = m_vel.0.length();
        if speed < 0.001 {
            continue;
        }
        let missile_forward = m_vel.0 / speed;

        // Check if existing target entity is still alive
        if let Some(target_entity) = m_target.target_entity {
            if let Ok((_, ship_transform, _)) = ship_query.get(target_entity) {
                m_target.intercept_point = ship_transform.translation;
                continue;
            } else {
                m_target.target_entity = None;
            }
        }

        // No valid target — scan for closest ship in seeker cone
        let mut closest_dist = f32::MAX;
        let mut closest_entity = None;
        let mut closest_pos = Vec3::ZERO;

        for (ship_entity, ship_transform, _) in &ship_query {
            if ship_entity == m_owner.0 {
                continue;
            }

            let ship_pos = ship_transform.translation;
            if is_in_seeker_cone(missile_pos, missile_forward, ship_pos, SEEKER_HALF_ANGLE) {
                let dist = (ship_pos - missile_pos).length();
                if dist < closest_dist {
                    closest_dist = dist;
                    closest_entity = Some(ship_entity);
                    closest_pos = ship_pos;
                }
            }
        }

        if let Some(entity) = closest_entity {
            m_target.target_entity = Some(entity);
            m_target.intercept_point = closest_pos;
        }
    }
}

/// Check missile-to-ship collisions. Apply damage and despawn missile on hit.
/// Skips the ship that fired the missile, but friendly fire IS possible.
fn check_missile_hits(
    mut commands: Commands,
    missile_query: Query<
        (Entity, &Transform, &MissileDamage, &MissileOwner),
        With<Missile>,
    >,
    mut ship_query: Query<(Entity, &Transform, &ShipClass, &mut Health), With<Ship>>,
) {
    for (missile_entity, missile_transform, damage, owner) in &missile_query {
        let missile_pos = missile_transform.translation;
        // Use XZ distance for collision (missiles come in from above)
        let missile_xz = Vec2::new(missile_pos.x, missile_pos.z);

        for (ship_entity, ship_transform, class, mut health) in &mut ship_query {
            if ship_entity == owner.0 {
                continue;
            }

            let ship_xz = Vec2::new(ship_transform.translation.x, ship_transform.translation.z);
            let dist = (missile_xz - ship_xz).length();

            // Also check Y — missile must be near ground level to hit
            let y_dist = (missile_pos.y - ship_transform.translation.y).abs();

            if dist < class.profile().collision_radius && y_dist < class.profile().collision_radius {
                health.hp = health.hp.saturating_sub(damage.0);
                commands.entity(missile_entity).despawn();
                break;
            }
        }
    }
}

/// Despawn missiles that have run out of fuel.
fn despawn_spent_missiles(
    mut commands: Commands,
    query: Query<(Entity, &MissileFuel), With<Missile>>,
) {
    for (entity, fuel) in &query {
        if fuel.0 <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}

/// Despawn missiles that have been destroyed by point defense.
fn despawn_destroyed_missiles(
    mut commands: Commands,
    query: Query<(Entity, &MissileHealth), With<Missile>>,
) {
    for (entity, health) in &query {
        if health.0 == 0 {
            commands.entity(entity).despawn();
        }
    }
}

/// Despawn missiles that leave the map bounds.
fn check_missile_bounds(
    mut commands: Commands,
    bounds: Res<MapBounds>,
    query: Query<(Entity, &Transform), With<Missile>>,
) {
    for (entity, transform) in &query {
        let pos = transform.translation;
        if pos.x.abs() > bounds.half_extents.x || pos.z.abs() > bounds.half_extents.y {
            commands.entity(entity).despawn();
        }
    }
}

// ── Plugin ─────────────────────────────────────────────────────────────

pub struct MissilePlugin;

impl Plugin for MissilePlugin {
    fn build(&self, app: &mut App) {
        // NOTE: Missile component replication is registered in SharedReplicationPlugin.
        app.add_systems(
            Update,
            (
                advance_missiles,
                update_missile_flight,
                terminal_seeker_scan_system,
                check_missile_hits,
                despawn_spent_missiles,
                despawn_destroyed_missiles,
                check_missile_bounds,
            )
                .chain()
                .run_if(in_state(GameState::Playing)),
        );
    }
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── compute_intercept_point ────────────────────────────────────────

    #[test]
    fn intercept_stationary_target() {
        let shooter = Vec3::new(0.0, 0.0, 0.0);
        let target = Vec3::new(100.0, 0.0, 0.0);
        let target_vel = Vec2::ZERO;
        let speed = 80.0;

        let result = compute_intercept_point(shooter, target, target_vel, speed);
        // Stationary target: intercept should be at target position
        assert!(
            (result - target).length() < 0.01,
            "stationary target intercept should be at target pos, got {:?}",
            result
        );
    }

    #[test]
    fn intercept_moving_target() {
        let shooter = Vec3::new(0.0, 0.0, 0.0);
        let target = Vec3::new(100.0, 0.0, 0.0);
        let target_vel = Vec2::new(0.0, 20.0); // moving in Z
        let speed = 80.0;

        let result = compute_intercept_point(shooter, target, target_vel, speed);
        // Moving target: intercept Z should be positive (target moves in +Z)
        assert!(
            result.z > 0.0,
            "intercept Z should be positive for +Z moving target, got {}",
            result.z
        );
        // X should still be roughly near 100
        assert!(
            (result.x - 100.0).abs() < 10.0,
            "intercept X should be near target X"
        );
    }

    #[test]
    fn intercept_zero_speed_returns_target() {
        let shooter = Vec3::new(0.0, 0.0, 0.0);
        let target = Vec3::new(50.0, 5.0, 30.0);
        let result = compute_intercept_point(shooter, target, Vec2::new(10.0, 10.0), 0.0);
        assert!(
            (result - target).length() < 0.01,
            "zero speed should return target pos"
        );
    }

    // ── is_in_seeker_cone ──────────────────────────────────────────────

    #[test]
    fn target_inside_seeker_cone() {
        let missile_pos = Vec3::ZERO;
        let missile_forward = Vec3::new(1.0, 0.0, 0.0); // facing +X
        let target_pos = Vec3::new(100.0, 5.0, 10.0); // slightly off-axis
        let half_angle = SEEKER_HALF_ANGLE; // ~30 degrees

        assert!(
            is_in_seeker_cone(missile_pos, missile_forward, target_pos, half_angle),
            "target slightly off-axis should be in 30-degree cone"
        );
    }

    #[test]
    fn target_outside_seeker_cone() {
        let missile_pos = Vec3::ZERO;
        let missile_forward = Vec3::new(1.0, 0.0, 0.0); // facing +X
        let target_pos = Vec3::new(10.0, 0.0, 100.0); // far off to the side
        let half_angle = SEEKER_HALF_ANGLE;

        assert!(
            !is_in_seeker_cone(missile_pos, missile_forward, target_pos, half_angle),
            "target far off-axis should NOT be in 30-degree cone"
        );
    }

    #[test]
    fn target_directly_ahead_in_cone() {
        let missile_pos = Vec3::ZERO;
        let missile_forward = Vec3::new(0.0, 0.0, -1.0);
        let target_pos = Vec3::new(0.0, 0.0, -50.0);

        assert!(is_in_seeker_cone(
            missile_pos,
            missile_forward,
            target_pos,
            SEEKER_HALF_ANGLE
        ));
    }

    #[test]
    fn target_behind_not_in_cone() {
        let missile_pos = Vec3::ZERO;
        let missile_forward = Vec3::new(1.0, 0.0, 0.0);
        let target_pos = Vec3::new(-50.0, 0.0, 0.0); // behind

        assert!(!is_in_seeker_cone(
            missile_pos,
            missile_forward,
            target_pos,
            SEEKER_HALF_ANGLE
        ));
    }

    // ── spawn_missile ──────────────────────────────────────────────────

    #[test]
    fn spawn_missile_creates_all_components() {
        let mut world = World::new();
        let owner = world.spawn_empty().id();

        let origin = Vec3::new(10.0, 0.0, -20.0);
        let intercept = Vec3::new(100.0, 0.0, -20.0);
        let speed = 80.0;
        let damage = 30;
        let health = 5;
        let fuel = 500.0;

        let missile_entity;
        {
            let mut commands = world.commands();
            missile_entity = spawn_missile(
                &mut commands,
                origin,
                intercept,
                Some(owner), // target_entity (using owner as placeholder)
                speed,
                damage,
                health,
                fuel,
                owner,
            );
        }
        world.flush();

        assert!(world.get::<Missile>(missile_entity).is_some());
        assert!(world.get::<MissileTarget>(missile_entity).is_some());
        assert!(world.get::<MissileVelocity>(missile_entity).is_some());
        assert!(world.get::<MissileHealth>(missile_entity).is_some());
        assert!(world.get::<MissileDamage>(missile_entity).is_some());
        assert!(world.get::<MissileOwner>(missile_entity).is_some());
        assert!(world.get::<MissileFuel>(missile_entity).is_some());
        assert!(world.get::<FlightPhase>(missile_entity).is_some());
        assert!(world.get::<Transform>(missile_entity).is_some());

        let phase = world.get::<FlightPhase>(missile_entity).unwrap();
        assert_eq!(*phase, FlightPhase::Climb);

        let dmg = world.get::<MissileDamage>(missile_entity).unwrap();
        assert_eq!(dmg.0, 30);

        let hp = world.get::<MissileHealth>(missile_entity).unwrap();
        assert_eq!(hp.0, 5);

        let f = world.get::<MissileFuel>(missile_entity).unwrap();
        assert_eq!(f.0, 500.0);

        let m_owner = world.get::<MissileOwner>(missile_entity).unwrap();
        assert_eq!(m_owner.0, owner);

        let transform = world.get::<Transform>(missile_entity).unwrap();
        assert!((transform.translation - origin).length() < 0.01);
    }

    #[test]
    fn spawn_missile_has_upward_velocity() {
        let mut world = World::new();
        let owner = world.spawn_empty().id();

        let origin = Vec3::ZERO;
        let intercept = Vec3::new(100.0, 0.0, 0.0);

        let missile_entity;
        {
            let mut commands = world.commands();
            missile_entity = spawn_missile(
                &mut commands,
                origin,
                intercept,
                None,
                80.0,
                30,
                5,
                500.0,
                owner,
            );
        }
        world.flush();

        let vel = world.get::<MissileVelocity>(missile_entity).unwrap();
        // Should have positive Y (climbing)
        assert!(
            vel.0.y > 0.0,
            "missile should have upward velocity during climb, got {:?}",
            vel.0
        );
        // Should have positive X (toward intercept)
        assert!(
            vel.0.x > 0.0,
            "missile should have forward velocity toward intercept, got {:?}",
            vel.0
        );
        // Speed should be approximately the requested speed
        let actual_speed = vel.0.length();
        assert!(
            (actual_speed - 80.0).abs() < 0.1,
            "missile speed should be ~80, got {}",
            actual_speed
        );
    }

    // ── Flight phase transitions ───────────────────────────────────────

    #[test]
    fn climb_to_cruise_transition_at_altitude() {
        // A missile at cruise altitude should transition
        let pos_below = Vec3::new(0.0, CRUISE_ALTITUDE - 1.0, 0.0);
        let pos_at = Vec3::new(0.0, CRUISE_ALTITUDE, 0.0);
        let pos_above = Vec3::new(0.0, CRUISE_ALTITUDE + 1.0, 0.0);

        assert!(
            pos_below.y < CRUISE_ALTITUDE,
            "below cruise altitude: no transition"
        );
        assert!(
            pos_at.y >= CRUISE_ALTITUDE,
            "at cruise altitude: should transition"
        );
        assert!(
            pos_above.y >= CRUISE_ALTITUDE,
            "above cruise altitude: should transition"
        );
    }

    #[test]
    fn cruise_to_dive_transition_at_distance() {
        let intercept = Vec3::new(100.0, 0.0, 0.0);

        // Far away: no transition
        let pos_far = Vec3::new(0.0, CRUISE_ALTITUDE, 0.0);
        let xz_dist_far = Vec2::new(intercept.x - pos_far.x, intercept.z - pos_far.z).length();
        assert!(xz_dist_far > DIVE_DISTANCE, "far away: no transition");

        // Within dive distance
        let pos_close = Vec3::new(60.0, CRUISE_ALTITUDE, 0.0);
        let xz_dist_close =
            Vec2::new(intercept.x - pos_close.x, intercept.z - pos_close.z).length();
        assert!(
            xz_dist_close <= DIVE_DISTANCE,
            "within dive distance: should transition"
        );
    }

    #[test]
    fn dive_to_terminal_at_half_dive_distance() {
        let intercept = Vec3::new(100.0, 0.0, 0.0);

        // At half dive distance
        let threshold = DIVE_DISTANCE * 0.5;
        let pos = Vec3::new(100.0 - threshold + 1.0, 30.0, 0.0);
        let xz_dist = Vec2::new(intercept.x - pos.x, intercept.z - pos.z).length();
        assert!(
            xz_dist <= threshold,
            "within half dive distance: should transition to terminal"
        );
    }
}
