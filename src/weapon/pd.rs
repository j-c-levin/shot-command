use bevy::prelude::*;
use bevy_replicon::prelude::Replicated;
use rand::Rng;

use crate::game::{GameState, Team};
use crate::ship::Ship;
use crate::weapon::missile::{Missile, MissileHealth, MissileOwner, MissileVelocity};
use crate::weapon::projectile::{
    CwisRound, Projectile, ProjectileDamage, ProjectileOwner, ProjectileVelocity,
};
use crate::weapon::{Mounts, WeaponType};

/// Random spread per round in degrees.
const CWIS_SPREAD_DEGREES: f32 = 3.0;

/// Number of visual tracer rounds to spawn per CWIS tick.
const CWIS_TRACER_COUNT: u32 = 3;

/// Hit probability per CWIS tick (50%).
const CWIS_HIT_CHANCE: f32 = 0.5;

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
/// Skips friendly missiles (same team as PD ship).
fn laser_pd_fire(
    mut ship_query: Query<(Entity, &Transform, &mut Mounts), With<Ship>>,
    mut missile_query: Query<(Entity, &Transform, &mut MissileHealth, &MissileOwner), With<Missile>>,
    team_query: Query<&Team>,
) {
    for (ship_entity, ship_transform, mut mounts) in &mut ship_query {
        let ship_pos = ship_transform.translation;
        let ship_team = team_query.get(ship_entity).ok();

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

            // Find closest ENEMY missile within PD cylinder
            let mut closest_dist = f32::MAX;
            let mut closest_entity = None;

            for (missile_entity, missile_transform, _, missile_owner) in &missile_query {
                // IFF: skip missiles fired by ships on the same team
                if let (Some(my_team), Ok(owner_team)) =
                    (ship_team, team_query.get(missile_owner.0))
                {
                    if my_team == owner_team {
                        continue;
                    }
                }

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
                if let Ok((_, _, mut health, _)) = missile_query.get_mut(target_entity) {
                    health.0 = health.0.saturating_sub(profile.damage);
                }

                // Reset cooldown
                mounts.0[mount_idx].weapon.as_mut().unwrap().cooldown = profile.fire_rate_secs;
            }
        }
    }
}

/// CWIS: probability-based point defense that damages missiles directly with a
/// 50% hit chance per tick. Spawns visual-only tracer rounds (CwisRound marker)
/// for aesthetics — these have no collision detection.
fn cwis_fire(
    mut commands: Commands,
    mut ship_query: Query<(Entity, &Transform, &mut Mounts), With<Ship>>,
    mut missile_query: Query<(Entity, &Transform, &MissileVelocity, &mut MissileHealth, &MissileOwner), With<Missile>>,
    team_query: Query<&Team>,
) {
    let mut rng = rand::rng();

    for (ship_entity, ship_transform, mut mounts) in &mut ship_query {
        let ship_pos = ship_transform.translation;
        let ship_team = team_query.get(ship_entity).ok();

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

            // Find closest ENEMY missile within PD cylinder
            let mut closest_dist = f32::MAX;
            let mut closest_entity = None;

            for (missile_entity, missile_transform, _, _, missile_owner) in &missile_query {
                // IFF: skip missiles fired by ships on the same team
                if let (Some(my_team), Ok(owner_team)) =
                    (ship_team, team_query.get(missile_owner.0))
                {
                    if my_team == owner_team {
                        continue;
                    }
                }

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
                let (_, target_tf, target_vel, _, _) = missile_query.get(target_entity).unwrap();
                let target_pos = target_tf.translation;
                let target_v = target_vel.0;

                let origin = ship_pos + Vec3::new(0.0, 5.0, 0.0);

                // Lead prediction for visual tracers
                let dist = (target_pos - origin).length();
                let travel_time = if profile.projectile_speed > 0.001 {
                    dist / profile.projectile_speed
                } else {
                    0.0
                };
                let lead_pos = target_pos + target_v * travel_time;
                let dir_to_lead = (lead_pos - origin).normalize_or_zero();

                // Roll hit chance — deal damage directly on hit
                if rng.random_range(0.0..1.0) < CWIS_HIT_CHANCE {
                    if let Ok((_, _, _, mut health, _)) = missile_query.get_mut(target_entity) {
                        health.0 = health.0.saturating_sub(profile.damage);
                    }
                }

                // Spawn visual-only tracer rounds (CwisRound marker — no collision detection)
                for _ in 0..CWIS_TRACER_COUNT {
                    let spread_rad =
                        rng.random_range(-CWIS_SPREAD_DEGREES..CWIS_SPREAD_DEGREES).to_radians();
                    let cos_s = spread_rad.cos();
                    let sin_s = spread_rad.sin();
                    let xz_spread = Vec3::new(
                        dir_to_lead.x * cos_s - dir_to_lead.z * sin_s,
                        dir_to_lead.y,
                        dir_to_lead.x * sin_s + dir_to_lead.z * cos_s,
                    );
                    let vert_spread = rng.random_range(-1.0_f32..1.0_f32).to_radians();
                    let spread_dir = Vec3::new(
                        xz_spread.x,
                        xz_spread.y + vert_spread.sin() * 0.03,
                        xz_spread.z,
                    )
                    .normalize_or_zero();

                    commands.spawn((
                        Projectile,
                        ProjectileVelocity(spread_dir * profile.projectile_speed),
                        ProjectileDamage(profile.damage),
                        ProjectileOwner(ship_entity),
                        CwisRound,
                        Transform::from_translation(origin),
                        Replicated,
                    ));
                }

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
