use bevy::ecs::entity::{EntityMapper, MapEntities};
use bevy::prelude::*;
use bevy_replicon::prelude::Replicated;
use serde::{Deserialize, Serialize};

use crate::game::{GameState, Health, Team};
use crate::map::{Asteroid, AsteroidSize, MapBounds};
use crate::ship::{ship_facing_direction, EngineHealth, RepairCooldown, Ship, ShipClass, Velocity};
use crate::weapon::damage::apply_damage_to_ship;
use crate::weapon::Mounts;

/// Marker for explosion visual entities. Server spawns these, client materializes.
#[derive(Component, Serialize, Deserialize)]
pub struct Explosion;

/// Timer that controls how long the explosion is visible before despawning.
#[derive(Component, Serialize, Deserialize)]
pub struct ExplosionTimer(pub f32);

// ── Constants ──────────────────────────────────────────────────────────

/// Half-angle of the seeker cone (~30 degrees).
pub const SEEKER_HALF_ANGLE: f32 = 0.5236;

/// Maximum range at which the seeker can acquire targets (meters).
/// Prevents missiles from locking onto distant targets immediately after launch.
pub const SEEKER_MAX_RANGE: f32 = 200.0;

/// Maximum turn rate for missile steering (radians per second).
/// ~90°/s gives smooth arcing turns.
const MISSILE_TURN_RATE: f32 = std::f32::consts::FRAC_PI_2;

/// Y level missiles fly at (same as ships).
pub const MISSILE_Y: f32 = 5.0;

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

/// Damage dealt to a ship on impact.
#[derive(Component, Serialize, Deserialize, Clone)]
pub struct MissileDamage(pub u16);

/// Ship that launched this missile. Skipped for self-hit checks.
#[derive(Component, Serialize, Deserialize, MapEntities, Clone)]
pub struct MissileOwner(#[entities] pub Entity);

/// Remaining fuel in meters of travel distance.
#[derive(Component, Serialize, Deserialize, Clone)]
pub struct MissileFuel(pub f32);

// ── Pure functions ─────────────────────────────────────────────────────

/// Predict where the target will be when a missile arrives.
///
/// Uses two-iteration linear prediction. Path is mostly flat so arc factor is 1.0.
pub fn compute_intercept_point(
    shooter_pos: Vec3,
    target_pos: Vec3,
    target_velocity: Vec2,
    missile_speed: f32,
) -> Vec3 {
    if missile_speed < 0.001 {
        return target_pos;
    }

    // Flat flight — no arc multiplier
    let arc_factor = 1.0;

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
/// Rotate `current` direction toward `desired` direction by at most `max_angle` radians.
/// Returns the new direction at the same speed.
pub fn steer_toward(current: Vec3, desired: Vec3, max_angle: f32) -> Vec3 {
    let cur_norm = current.normalize_or_zero();
    let des_norm = desired.normalize_or_zero();
    if cur_norm == Vec3::ZERO || des_norm == Vec3::ZERO {
        return current;
    }

    let dot = cur_norm.dot(des_norm).clamp(-1.0, 1.0);
    let angle = dot.acos();

    if angle <= max_angle || angle < 0.001 {
        return des_norm * current.length();
    }

    let axis = cur_norm.cross(des_norm);
    if axis.length_squared() < 1e-8 {
        // Nearly anti-parallel — pick an arbitrary perpendicular axis
        let perp = if cur_norm.x.abs() < 0.9 {
            Vec3::X
        } else {
            Vec3::Y
        };
        let axis = cur_norm.cross(perp).normalize();
        let rotation = Quat::from_axis_angle(axis, max_angle);
        return (rotation * cur_norm) * current.length();
    }

    let axis = axis.normalize();
    let rotation = Quat::from_axis_angle(axis, max_angle);
    (rotation * cur_norm) * current.length()
}

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

/// Spawn a replicated missile entity with flat initial velocity at missile level.
///
/// `origin` — world-space launch position.
/// `intercept_point` — predicted ground-level intercept point.
/// `target_entity` — optional entity being targeted.
/// `speed` — scalar missile speed.
/// `damage` — HP dealt on ship hit.
/// `fuel` — max range in meters.
/// `owner` — entity of the ship that fired.
pub fn spawn_missile(
    commands: &mut Commands,
    origin: Vec3,
    intercept_point: Vec3,
    target_entity: Option<Entity>,
    speed: f32,
    damage: u16,
    fuel: f32,
    owner: Entity,
) -> Entity {
    // Flat velocity toward intercept point at missile level
    let xz_dir = Vec2::new(
        intercept_point.x - origin.x,
        intercept_point.z - origin.z,
    )
    .normalize_or_zero();

    let velocity = Vec3::new(xz_dir.x * speed, 0.0, xz_dir.y * speed);

    // Spawn at missile level Y
    let spawn_pos = Vec3::new(origin.x, MISSILE_Y, origin.z);

    commands
        .spawn((
            Missile,
            MissileTarget {
                intercept_point,
                target_entity,
            },
            MissileVelocity(velocity),
            MissileDamage(damage),
            MissileOwner(owner),
            MissileFuel(fuel),
            Transform::from_translation(spawn_pos),
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

/// Update missile flight: seeker scanning and physics-based steering.
///
/// Each tick, for each missile:
/// 1. Track existing target entity if alive, updating intercept point
/// 2. Run seeker cone on all enemy ships — acquire closest target
/// 3. Compute desired direction toward intercept (or fly straight if past it)
/// 4. Apply steer_toward with MISSILE_TURN_RATE * dt
fn update_missile_flight(
    time: Res<Time>,
    mut missile_query: Query<
        (&Transform, &mut MissileVelocity, &mut MissileTarget, &MissileOwner),
        With<Missile>,
    >,
    ship_query: Query<(Entity, &Transform, &Velocity, &Team), With<Ship>>,
    team_query: Query<&Team>,
) {
    let dt = time.delta_secs();
    let max_turn = MISSILE_TURN_RATE * dt;

    for (transform, mut vel, mut m_target, m_owner) in &mut missile_query {
        let pos = transform.translation;
        let speed = vel.0.length();
        if speed < 0.001 {
            continue;
        }
        let missile_forward = vel.0 / speed;
        let owner_team = team_query.get(m_owner.0).ok();

        // ── Seeker cone — active entire flight ─────────────────────
        // Track existing target if alive
        if let Some(target_entity) = m_target.target_entity {
            if let Ok((_, ship_tf, ship_vel, _)) = ship_query.get(target_entity) {
                m_target.intercept_point = compute_intercept_point(
                    pos,
                    ship_tf.translation,
                    ship_vel.linear,
                    speed,
                );
            } else {
                m_target.target_entity = None;
            }
        }

        // Scan for closest enemy in seeker cone
        if m_target.target_entity.is_none() {
            let mut closest_dist = f32::MAX;
            let mut closest_entity = None;
            let mut closest_pos = Vec3::ZERO;
            let mut closest_vel = Vec2::ZERO;

            for (ship_entity, ship_tf, ship_vel, ship_team) in &ship_query {
                if ship_entity == m_owner.0 {
                    continue;
                }
                if let Some(ot) = owner_team {
                    if ot == ship_team {
                        continue;
                    }
                }
                let ship_pos = ship_tf.translation;
                let dist = (ship_pos - pos).length();
                if dist > SEEKER_MAX_RANGE {
                    continue;
                }
                if is_in_seeker_cone(pos, missile_forward, ship_pos, SEEKER_HALF_ANGLE) {
                    if dist < closest_dist {
                        closest_dist = dist;
                        closest_entity = Some(ship_entity);
                        closest_pos = ship_pos;
                        closest_vel = ship_vel.linear;
                    }
                }
            }

            if let Some(e) = closest_entity {
                m_target.target_entity = Some(e);
                m_target.intercept_point =
                    compute_intercept_point(pos, closest_pos, closest_vel, speed);
            }
        }

        // ── Compute desired direction ───────────────────────────────
        let to_target = m_target.intercept_point - pos;
        let past_intercept = to_target.dot(missile_forward) < 0.0;
        let desired_dir = if past_intercept {
            missile_forward // keep flying straight
        } else {
            to_target.normalize_or_zero()
        };

        // Apply physics-based steering
        if desired_dir != Vec3::ZERO {
            vel.0 = steer_toward(vel.0, desired_dir * speed, max_turn);
        }

        // Maintain constant speed
        let current_speed = vel.0.length();
        if current_speed > 0.001 {
            vel.0 = vel.0 * (speed / current_speed);
        }
    }
}

/// Destroy missiles that collide with asteroids (LOS blockers).
fn check_missile_asteroid_hits(
    mut commands: Commands,
    missile_query: Query<(Entity, &Transform), With<Missile>>,
    asteroid_query: Query<(&Transform, &AsteroidSize), With<Asteroid>>,
) {
    for (missile_entity, missile_tf) in &missile_query {
        let missile_xz = Vec2::new(missile_tf.translation.x, missile_tf.translation.z);

        for (asteroid_tf, asteroid_size) in &asteroid_query {
            let asteroid_xz = Vec2::new(asteroid_tf.translation.x, asteroid_tf.translation.z);
            let dist = (missile_xz - asteroid_xz).length();

            if dist < asteroid_size.radius {
                spawn_small_explosion(&mut commands, missile_tf.translation);
                if let Ok(mut e) = commands.get_entity(missile_entity) { e.despawn(); }
                break;
            }
        }
    }
}

/// Check missile-to-ship collisions. Apply damage and despawn missile on hit.
/// Skips same-team ships (missiles don't friendly-fire).
/// Public so PD systems can order themselves before this.
pub fn check_missile_hits(
    mut commands: Commands,
    missile_query: Query<
        (Entity, &Transform, &MissileVelocity, &MissileDamage, &MissileOwner),
        With<Missile>,
    >,
    mut ship_query: Query<(Entity, &Transform, &ShipClass, &mut Health, &mut EngineHealth, &mut Mounts, &mut RepairCooldown, &Team), With<Ship>>,
    team_query: Query<&Team>,
) {
    for (missile_entity, missile_transform, missile_vel, damage, owner) in &missile_query {
        let missile_pos = missile_transform.translation;
        let missile_xz = Vec2::new(missile_pos.x, missile_pos.z);
        let owner_team = team_query.get(owner.0).ok();

        for (ship_entity, ship_transform, class, mut health, mut engine_health, mut mounts, mut repair_cooldown, ship_team) in &mut ship_query {
            // Skip own ship and same-team ships
            if ship_entity == owner.0 {
                continue;
            }
            if let Some(ot) = owner_team {
                if ot == ship_team {
                    continue;
                }
            }

            let ship_xz = Vec2::new(ship_transform.translation.x, ship_transform.translation.z);
            let dist = (missile_xz - ship_xz).length();

            let y_dist = (missile_pos.y - ship_transform.translation.y).abs();

            // Use generous Y tolerance — missiles fly slightly above ships
            if dist < class.profile().collision_radius
                && y_dist < class.profile().collision_radius + MISSILE_Y
            {
                let ship_forward = ship_facing_direction(ship_transform);
                let impact_dir = Vec2::new(missile_vel.0.x, missile_vel.0.z).normalize_or_zero();
                let engines_went_offline = apply_damage_to_ship(
                    impact_dir, ship_forward, damage.0, false,
                    &mut health, &mut engine_health, &mut mounts, &mut repair_cooldown,
                );
                if engines_went_offline {
                    commands.entity(ship_entity).insert(crate::game::EngineOffline);
                }
                spawn_explosion(&mut commands, missile_pos);
                if let Ok(mut e) = commands.get_entity(missile_entity) { e.despawn(); }
                break;
            }
        }
    }
}

/// Despawn missiles that have run out of fuel (detonate at end of range).
fn despawn_spent_missiles(
    mut commands: Commands,
    query: Query<(Entity, &Transform, &MissileFuel), With<Missile>>,
) {
    for (entity, transform, fuel) in &query {
        if fuel.0 <= 0.0 {
            spawn_small_explosion(&mut commands, transform.translation);
            if let Ok(mut e) = commands.get_entity(entity) { e.despawn(); }
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
            if let Ok(mut e) = commands.get_entity(entity) { e.despawn(); }
        }
    }
}

// ── Explosions ────────────────────────────────────────────────────────

/// Spawn a short-lived explosion marker at the given position (ship impact — bright, full-size).
pub fn spawn_explosion(commands: &mut Commands, position: Vec3) {
    commands.spawn((
        Explosion,
        ExplosionTimer(0.4),
        Transform::from_translation(position),
        Replicated,
    ));
}

/// Spawn a smaller, dimmer explosion for PD kills and fuel depletion (mid-air destruction).
pub fn spawn_small_explosion(commands: &mut Commands, position: Vec3) {
    commands.spawn((
        Explosion,
        ExplosionTimer(0.25),
        Transform::from_translation(position).with_scale(Vec3::splat(0.5)),
        Replicated,
    ));
}

/// Tick explosion timers and despawn when expired.
fn tick_explosions(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut ExplosionTimer), With<Explosion>>,
) {
    let dt = time.delta_secs();
    for (entity, mut timer) in &mut query {
        timer.0 -= dt;
        if timer.0 <= 0.0 {
            if let Ok(mut e) = commands.get_entity(entity) { e.despawn(); }
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
                check_missile_asteroid_hits,
                check_missile_hits,
                despawn_spent_missiles,
                check_missile_bounds,
            )
                .chain()
                .run_if(in_state(GameState::Playing)),
        );
        app.add_systems(
            Update,
            tick_explosions.run_if(in_state(GameState::Playing)),
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
                fuel,
                owner,
            );
        }
        world.flush();

        assert!(world.get::<Missile>(missile_entity).is_some());
        assert!(world.get::<MissileTarget>(missile_entity).is_some());
        assert!(world.get::<MissileVelocity>(missile_entity).is_some());
        assert!(world.get::<MissileDamage>(missile_entity).is_some());
        assert!(world.get::<MissileOwner>(missile_entity).is_some());
        assert!(world.get::<MissileFuel>(missile_entity).is_some());
        assert!(world.get::<Transform>(missile_entity).is_some());

        let dmg = world.get::<MissileDamage>(missile_entity).unwrap();
        assert_eq!(dmg.0, 30);

        let f = world.get::<MissileFuel>(missile_entity).unwrap();
        assert_eq!(f.0, 500.0);

        let m_owner = world.get::<MissileOwner>(missile_entity).unwrap();
        assert_eq!(m_owner.0, owner);

        // Spawns at missile level Y
        let transform = world.get::<Transform>(missile_entity).unwrap();
        assert!((transform.translation.x - origin.x).abs() < 0.01);
        assert!((transform.translation.y - MISSILE_Y).abs() < 0.01);
        assert!((transform.translation.z - origin.z).abs() < 0.01);
    }

    #[test]
    fn spawn_missile_has_flat_velocity() {
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
                500.0,
                owner,
            );
        }
        world.flush();

        let vel = world.get::<MissileVelocity>(missile_entity).unwrap();
        // Should have zero Y (flat flight)
        assert!(
            vel.0.y.abs() < 0.01,
            "missile should have flat velocity (Y = 0), got {:?}",
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

    // ── steer_toward ──────────────────────────────────────────────────

    #[test]
    fn steer_toward_small_angle_reaches_target() {
        let current = Vec3::new(80.0, 0.0, 0.0);
        let desired = Vec3::new(79.0, 0.0, 10.0).normalize() * 80.0;
        let max_angle = 0.5; // ~28 degrees, more than enough

        let result = steer_toward(current, desired, max_angle);
        let result_dir = result.normalize();
        let desired_dir = desired.normalize();
        assert!((result_dir - desired_dir).length() < 0.01);
    }

    #[test]
    fn steer_toward_large_angle_is_clamped() {
        let current = Vec3::new(80.0, 0.0, 0.0); // facing +X
        let desired = Vec3::new(0.0, 0.0, 80.0); // facing +Z (90 degrees away)
        let max_angle = 0.1; // ~5.7 degrees

        let result = steer_toward(current, desired, max_angle);
        // Should have turned, but not reached desired
        let angle_to_desired = result.normalize().dot(desired.normalize()).acos();
        assert!(angle_to_desired > 0.05, "should not have reached desired direction");
        // But should have turned from original
        let angle_from_original = result.normalize().dot(current.normalize()).acos();
        assert!((angle_from_original - max_angle).abs() < 0.01, "should have turned exactly max_angle");
    }

    #[test]
    fn steer_toward_preserves_speed() {
        let current = Vec3::new(80.0, 0.0, 0.0);
        let desired = Vec3::new(0.0, 80.0, 0.0);
        let result = steer_toward(current, desired, 0.2);
        assert!((result.length() - 80.0).abs() < 0.1);
    }

    // ── asteroid collision ────────────────────────────────────────────

    #[test]
    fn missile_destroyed_by_asteroid_collision() {
        // Missile inside asteroid radius should be destroyed
        let missile_xz = Vec2::new(50.0, 0.0);
        let asteroid_xz = Vec2::new(52.0, 0.0); // 2m away
        let asteroid_radius = 20.0;
        let dist = (missile_xz - asteroid_xz).length();
        assert!(dist < asteroid_radius, "missile should be inside asteroid");
    }

    #[test]
    fn missile_survives_outside_asteroid() {
        let missile_xz = Vec2::new(50.0, 0.0);
        let asteroid_xz = Vec2::new(100.0, 0.0); // 50m away
        let asteroid_radius = 20.0;
        let dist = (missile_xz - asteroid_xz).length();
        assert!(dist >= asteroid_radius, "missile should be outside asteroid");
    }

    // ── check_missile_hits collision logic ─────────────────────────────

    #[test]
    fn missile_collision_check_xz_and_y() {
        let missile_pos = Vec3::new(100.0, MISSILE_Y, 50.0);
        let ship_pos = Vec3::new(101.0, 0.0, 50.0);
        let collision_radius = 12.0; // battleship

        let xz_dist = Vec2::new(missile_pos.x - ship_pos.x, missile_pos.z - ship_pos.z).length();
        let y_dist = (missile_pos.y - ship_pos.y).abs();

        assert!(xz_dist < collision_radius, "XZ distance {} should be within collision radius {}", xz_dist, collision_radius);
        assert!(y_dist < collision_radius + MISSILE_Y, "Y distance {} should be within tolerance {}", y_dist, collision_radius + MISSILE_Y);
    }

    #[test]
    fn missile_collision_misses_when_too_far() {
        let missile_pos = Vec3::new(100.0, MISSILE_Y, 50.0);
        let ship_pos = Vec3::new(130.0, 0.0, 50.0); // 30m away
        let collision_radius = 12.0;

        let xz_dist = Vec2::new(missile_pos.x - ship_pos.x, missile_pos.z - ship_pos.z).length();
        assert!(xz_dist >= collision_radius, "XZ distance {} should be outside collision radius {}", xz_dist, collision_radius);
    }

    #[test]
    fn missile_continues_straight_past_intercept() {
        let missile_pos = Vec3::new(110.0, MISSILE_Y, 0.0);
        let missile_forward = Vec3::new(1.0, 0.0, 0.0); // heading +X
        let intercept_point = Vec3::new(100.0, 0.0, 0.0); // behind us

        let to_target = intercept_point - missile_pos;
        let past_intercept = to_target.dot(missile_forward) < 0.0;

        assert!(past_intercept, "missile should detect it has passed the intercept point");
        // When past intercept, desired direction should be missile_forward, not to_target
    }

    #[test]
    fn missile_steers_toward_intercept_when_ahead() {
        let missile_pos = Vec3::new(50.0, MISSILE_Y, 0.0);
        let missile_forward = Vec3::new(1.0, 0.0, 0.0);
        let intercept_point = Vec3::new(200.0, 0.0, 0.0); // ahead

        let to_target = intercept_point - missile_pos;
        let past_intercept = to_target.dot(missile_forward) < 0.0;

        assert!(!past_intercept, "missile should NOT think it has passed when intercept is ahead");
    }
}
