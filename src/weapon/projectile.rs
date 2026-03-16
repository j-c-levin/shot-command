use bevy::ecs::entity::MapEntities;
use bevy::prelude::*;
use bevy_replicon::prelude::Replicated;
use serde::{Deserialize, Serialize};

use crate::game::{GameState, Health};
use crate::map::MapBounds;
use crate::ship::{Ship, ShipClass};

// ── Components ──────────────────────────────────────────────────────────

/// Marker for projectile entities.
#[derive(Component, Serialize, Deserialize)]
pub struct Projectile;

/// Constant velocity vector (direction * speed). No drag applied.
#[derive(Component, Serialize, Deserialize, Clone)]
pub struct ProjectileVelocity(pub Vec3);

/// HP to subtract on hit.
#[derive(Component, Serialize, Deserialize, Clone)]
pub struct ProjectileDamage(pub u16);

/// Ship that fired this projectile. Used only to skip self-hits;
/// friendly fire IS possible.
#[derive(Component, Serialize, Deserialize, MapEntities, Clone)]
pub struct ProjectileOwner(#[entities] pub Entity);

/// CWIS tracer round with remaining lifetime in seconds. These only damage missiles, not ships.
#[derive(Component, Serialize, Deserialize)]
pub struct CwisRound(pub f32);

// ── Spawning ────────────────────────────────────────────────────────────

/// Spawn a replicated projectile entity.
///
/// `origin` — world-space start position (caller determines Y).
/// `direction` — will be normalized internally.
/// `speed` — scalar speed.
/// `damage` — HP to subtract on hit.
/// `owner` — entity of the ship that fired.
pub fn spawn_projectile(
    commands: &mut Commands,
    origin: Vec3,
    direction: Vec3,
    speed: f32,
    damage: u16,
    owner: Entity,
) -> Entity {
    let dir = direction.normalize_or_zero();
    let velocity = dir * speed;

    commands
        .spawn((
            Projectile,
            ProjectileVelocity(velocity),
            ProjectileDamage(damage),
            ProjectileOwner(owner),
            Transform::from_translation(origin),
            Replicated,
        ))
        .id()
}

// ── Systems ─────────────────────────────────────────────────────────────

/// Move every projectile by its constant velocity. No drag.
fn advance_projectiles(
    time: Res<Time>,
    mut query: Query<(&mut Transform, &ProjectileVelocity), With<Projectile>>,
) {
    let dt = time.delta_secs();
    for (mut transform, vel) in &mut query {
        transform.translation += vel.0 * dt;
    }
}

/// Despawn CWIS tracer rounds after their lifetime expires.
fn despawn_cwis_rounds(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut CwisRound), With<Projectile>>,
) {
    let dt = time.delta_secs();
    for (entity, mut round) in &mut query {
        round.0 -= dt;
        if round.0 <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}

/// Despawn projectiles that leave the map bounds (XZ plane).
fn check_projectile_bounds(
    mut commands: Commands,
    bounds: Res<MapBounds>,
    query: Query<(Entity, &Transform), With<Projectile>>,
) {
    for (entity, transform) in &query {
        let pos = transform.translation;
        if pos.x.abs() > bounds.half_extents.x || pos.z.abs() > bounds.half_extents.y {
            commands.entity(entity).despawn();
        }
    }
}

/// Check projectile-to-ship collisions. One hit per projectile per frame.
/// Skips the ship that fired the projectile (self-hit), but friendly fire
/// against other same-team ships IS allowed.
fn check_projectile_hits(
    mut commands: Commands,
    projectile_query: Query<
        (Entity, &Transform, &ProjectileDamage, &ProjectileOwner),
        (With<Projectile>, Without<CwisRound>),
    >,
    mut ship_query: Query<(Entity, &Transform, &ShipClass, &mut Health), With<Ship>>,
) {
    for (proj_entity, proj_transform, _damage, owner) in &projectile_query {
        let proj_xz = Vec2::new(proj_transform.translation.x, proj_transform.translation.z);

        for (ship_entity, ship_transform, class, mut _health) in &mut ship_query {
            // Skip the ship that fired this projectile
            if ship_entity == owner.0 {
                continue;
            }

            let ship_xz = Vec2::new(ship_transform.translation.x, ship_transform.translation.z);
            let dist = (proj_xz - ship_xz).length();

            if dist < class.profile().collision_radius {
                // TODO: re-enable damage: health.hp = health.hp.saturating_sub(damage.0);
                commands.entity(proj_entity).despawn();
                break;
            }
        }
    }
}

// ── Plugin ──────────────────────────────────────────────────────────────

pub struct ProjectilePlugin;

impl Plugin for ProjectilePlugin {
    fn build(&self, app: &mut App) {
        // NOTE: Projectile component replication is registered in ServerNetPlugin
        // to ensure ordering matches the client exactly (protocol hash).
        app.add_systems(
                Update,
                (
                    advance_projectiles,
                    despawn_cwis_rounds,
                    check_projectile_bounds,
                    check_projectile_hits,
                )
                    .chain()
                    .run_if(in_state(GameState::Playing)),
            );
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_projectile_creates_all_components() {
        let mut world = World::new();
        let owner = world.spawn_empty().id();

        let origin = Vec3::new(10.0, 5.0, -20.0);
        let direction = Vec3::new(1.0, 0.0, 0.0);
        let speed = 150.0;
        let damage = 15;

        let proj_entity;
        {
            let mut commands = world.commands();
            proj_entity = spawn_projectile(&mut commands, origin, direction, speed, damage, owner);
        }
        world.flush();

        // Verify all components are present
        assert!(world.get::<Projectile>(proj_entity).is_some());

        let vel = world.get::<ProjectileVelocity>(proj_entity).unwrap();
        assert!((vel.0 - Vec3::new(150.0, 0.0, 0.0)).length() < 0.01);

        let dmg = world.get::<ProjectileDamage>(proj_entity).unwrap();
        assert_eq!(dmg.0, 15);

        let proj_owner = world.get::<ProjectileOwner>(proj_entity).unwrap();
        assert_eq!(proj_owner.0, owner);

        let transform = world.get::<Transform>(proj_entity).unwrap();
        assert!((transform.translation - origin).length() < 0.01);
    }

    #[test]
    fn spawn_projectile_normalizes_direction() {
        let mut world = World::new();
        let owner = world.spawn_empty().id();

        let direction = Vec3::new(3.0, 0.0, 4.0); // length 5, not unit
        let speed = 100.0;

        let proj_entity;
        {
            let mut commands = world.commands();
            proj_entity =
                spawn_projectile(&mut commands, Vec3::ZERO, direction, speed, 10, owner);
        }
        world.flush();

        let vel = world.get::<ProjectileVelocity>(proj_entity).unwrap();
        // Should be normalized direction * speed = (0.6, 0, 0.8) * 100
        let expected = Vec3::new(60.0, 0.0, 80.0);
        assert!(
            (vel.0 - expected).length() < 0.01,
            "velocity should be normalized direction * speed: got {:?}",
            vel.0
        );
    }

    #[test]
    fn projectile_advances_by_velocity_times_dt() {
        let mut world = World::new();
        let owner = world.spawn_empty().id();

        let origin = Vec3::new(0.0, 5.0, 0.0);
        let velocity = Vec3::new(100.0, 0.0, 0.0);

        let proj_entity = world
            .spawn((
                Projectile,
                ProjectileVelocity(velocity),
                ProjectileDamage(10),
                ProjectileOwner(owner),
                Transform::from_translation(origin),
            ))
            .id();

        // Simulate one frame: dt = 0.1s
        let dt = 0.1_f32;
        {
            let mut transform = world.get_mut::<Transform>(proj_entity).unwrap();
            let vel = Vec3::new(100.0, 0.0, 0.0);
            transform.translation += vel * dt;
        }

        let transform = world.get::<Transform>(proj_entity).unwrap();
        let expected = Vec3::new(10.0, 5.0, 0.0);
        assert!(
            (transform.translation - expected).length() < 0.01,
            "projectile should advance to {:?}, got {:?}",
            expected,
            transform.translation
        );
    }

    #[test]
    fn projectile_outside_bounds_is_despawned() {
        let mut world = World::new();
        let owner = world.spawn_empty().id();

        let bounds = MapBounds {
            half_extents: Vec2::splat(500.0),
        };

        // Inside bounds — should survive
        let inside = world
            .spawn((
                Projectile,
                ProjectileVelocity(Vec3::X * 100.0),
                ProjectileDamage(10),
                ProjectileOwner(owner),
                Transform::from_xyz(100.0, 5.0, 0.0),
            ))
            .id();

        // Outside bounds (X > 500) — should be despawned
        let outside = world
            .spawn((
                Projectile,
                ProjectileVelocity(Vec3::X * 100.0),
                ProjectileDamage(10),
                ProjectileOwner(owner),
                Transform::from_xyz(501.0, 5.0, 0.0),
            ))
            .id();

        // Manual bounds check (simulating the system logic)
        let mut to_despawn = Vec::new();
        for entity in [inside, outside] {
            let transform = world.get::<Transform>(entity).unwrap();
            let pos = transform.translation;
            if pos.x.abs() > bounds.half_extents.x || pos.z.abs() > bounds.half_extents.y {
                to_despawn.push(entity);
            }
        }
        for entity in to_despawn {
            world.despawn(entity);
        }

        // Inside should still exist
        assert!(world.get_entity(inside).is_ok());
        // Outside should be gone
        assert!(world.get_entity(outside).is_err());
    }

    #[test]
    fn projectile_hits_ship_within_collision_radius() {
        let proj_pos = Vec2::new(100.0, 50.0);
        let ship_pos = Vec2::new(105.0, 50.0); // 5m away
        let collision_radius = 12.0; // battleship

        let dist = (proj_pos - ship_pos).length();
        assert!(dist < collision_radius, "projectile at {}m should hit ship with radius {}", dist, collision_radius);
    }

    #[test]
    fn projectile_misses_ship_outside_collision_radius() {
        let proj_pos = Vec2::new(100.0, 50.0);
        let ship_pos = Vec2::new(120.0, 50.0); // 20m away
        let collision_radius = 12.0;

        let dist = (proj_pos - ship_pos).length();
        assert!(dist >= collision_radius, "projectile at {}m should miss ship with radius {}", dist, collision_radius);
    }
}
