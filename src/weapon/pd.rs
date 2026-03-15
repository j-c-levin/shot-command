use bevy::prelude::*;
use bevy_replicon::prelude::Replicated;
use rand::Rng;

use crate::game::GameState;
use crate::ship::Ship;
use crate::weapon::missile::{Missile, MissileHealth};
use crate::weapon::projectile::{
    CwisRound, Projectile, ProjectileDamage, ProjectileOwner, ProjectileVelocity,
};
use crate::weapon::{Mounts, WeaponType};

// ── Pure functions ─────────────────────────────────────────────────────

/// Check if a missile is within a PD's vertical cylinder (XZ distance only).
/// The cylinder has infinite Y extent — missiles at any altitude are valid targets.
pub fn is_in_pd_cylinder(pd_pos: Vec3, missile_pos: Vec3, radius: f32) -> bool {
    let dx = pd_pos.x - missile_pos.x;
    let dz = pd_pos.z - missile_pos.z;
    (dx * dx + dz * dz).sqrt() <= radius
}

// ── Systems ────────────────────────────────────────────────────────────

/// Laser PD: instant-hit point defense that damages missiles directly.
fn laser_pd_fire(
    mut ship_query: Query<(&Transform, &mut Mounts), With<Ship>>,
    mut missile_query: Query<(Entity, &Transform, &mut MissileHealth), With<Missile>>,
) {
    for (ship_transform, mut mounts) in &mut ship_query {
        let ship_pos = ship_transform.translation;

        for mount_idx in 0..mounts.0.len() {
            let Some(ref weapon) = mounts.0[mount_idx].weapon else {
                continue;
            };

            if weapon.weapon_type != WeaponType::LaserPD {
                continue;
            }

            if weapon.cooldown > 0.0 {
                continue;
            }

            let profile = weapon.weapon_type.profile();

            // Find closest missile within PD cylinder
            let mut closest_dist = f32::MAX;
            let mut closest_entity = None;

            for (missile_entity, missile_transform, _) in &missile_query {
                let missile_pos = missile_transform.translation;
                if is_in_pd_cylinder(ship_pos, missile_pos, profile.pd_cylinder_radius) {
                    let dx = ship_pos.x - missile_pos.x;
                    let dz = ship_pos.z - missile_pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist < closest_dist {
                        closest_dist = dist;
                        closest_entity = Some(missile_entity);
                    }
                }
            }

            if let Some(target_entity) = closest_entity {
                // Deal damage to missile
                if let Ok((_, _, mut health)) = missile_query.get_mut(target_entity) {
                    health.0 = health.0.saturating_sub(profile.damage);
                }

                // Reset cooldown
                mounts.0[mount_idx].weapon.as_mut().unwrap().cooldown = profile.fire_rate_secs;
            }
        }
    }
}

/// CIWS: fires small projectiles at missiles with slight spread.
fn cwis_fire(
    mut commands: Commands,
    mut ship_query: Query<(&Transform, &mut Mounts), With<Ship>>,
    missile_query: Query<(Entity, &Transform), With<Missile>>,
) {
    for (ship_transform, mut mounts) in &mut ship_query {
        let ship_pos = ship_transform.translation;

        for mount_idx in 0..mounts.0.len() {
            let Some(ref weapon) = mounts.0[mount_idx].weapon else {
                continue;
            };

            if weapon.weapon_type != WeaponType::CWIS {
                continue;
            }

            if weapon.cooldown > 0.0 {
                continue;
            }

            let profile = weapon.weapon_type.profile();

            // Find closest missile within PD cylinder
            let mut closest_dist = f32::MAX;
            let mut closest_entity = None;

            for (missile_entity, missile_transform) in &missile_query {
                let missile_pos = missile_transform.translation;
                if is_in_pd_cylinder(ship_pos, missile_pos, profile.pd_cylinder_radius) {
                    let dx = ship_pos.x - missile_pos.x;
                    let dz = ship_pos.z - missile_pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist < closest_dist {
                        closest_dist = dist;
                        closest_entity = Some(missile_entity);
                    }
                }
            }

            if let Some(target_entity) = closest_entity {
                let target_transform = missile_query.get(target_entity).unwrap().1;
                let target_pos = target_transform.translation;

                let origin = ship_pos + Vec3::new(0.0, 5.0, 0.0);
                let dir_to_target = (target_pos - origin).normalize_or_zero();

                // Apply random spread (±2° around Y axis)
                let mut rng = rand::rng();
                let spread_rad = rng.random_range(-2.0_f32..2.0_f32).to_radians();
                let cos_s = spread_rad.cos();
                let sin_s = spread_rad.sin();
                let spread_dir = Vec3::new(
                    dir_to_target.x * cos_s - dir_to_target.z * sin_s,
                    dir_to_target.y,
                    dir_to_target.x * sin_s + dir_to_target.z * cos_s,
                );

                commands.spawn((
                    Projectile,
                    ProjectileVelocity(spread_dir * profile.projectile_speed),
                    ProjectileDamage(profile.damage),
                    ProjectileOwner(Entity::PLACEHOLDER),
                    CwisRound,
                    Transform::from_translation(origin),
                    Replicated,
                ));

                // Reset cooldown
                mounts.0[mount_idx].weapon.as_mut().unwrap().cooldown = profile.fire_rate_secs;
            }
        }
    }
}

// ── Plugin ─────────────────────────────────────────────────────────────

pub struct PdPlugin;

impl Plugin for PdPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (laser_pd_fire, cwis_fire).run_if(in_state(GameState::Playing)),
        );
    }
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missile_inside_cylinder_detected() {
        let pd_pos = Vec3::new(100.0, 0.0, 100.0);
        let missile_pos = Vec3::new(110.0, 60.0, 105.0); // 11.2m XZ distance, high altitude
        assert!(is_in_pd_cylinder(pd_pos, missile_pos, 150.0));
    }

    #[test]
    fn missile_outside_cylinder_not_detected() {
        let pd_pos = Vec3::new(100.0, 0.0, 100.0);
        let missile_pos = Vec3::new(300.0, 60.0, 100.0); // 200m XZ distance
        assert!(!is_in_pd_cylinder(pd_pos, missile_pos, 150.0));
    }

    #[test]
    fn cylinder_ignores_altitude() {
        let pd_pos = Vec3::new(0.0, 0.0, 0.0);
        let missile_low = Vec3::new(50.0, 5.0, 0.0);
        let missile_high = Vec3::new(50.0, 500.0, 0.0);
        // Same XZ distance, different Y — both should be detected
        assert!(is_in_pd_cylinder(pd_pos, missile_low, 100.0));
        assert!(is_in_pd_cylinder(pd_pos, missile_high, 100.0));
    }
}
