use bevy::prelude::*;
use bevy_replicon::prelude::Replicated;
use rand::Rng;

use crate::game::{GameState, Team};
use crate::ship::Ship;
use crate::weapon::missile::{Missile, MissileOwner, MissileVelocity};
use crate::weapon::projectile::{
    CwisRound, Projectile, ProjectileDamage, ProjectileOwner, ProjectileVelocity,
};
use crate::weapon::{Mounts, WeaponType};

/// Random spread per round in degrees.
const CWIS_SPREAD_DEGREES: f32 = 3.0;

/// Hit probability per CWIS tick (destroys missile outright).
const CWIS_HIT_CHANCE: f32 = 0.15;

/// Hit probability per Laser PD shot (destroys missile outright).
const LASER_PD_HIT_CHANCE: f32 = 0.6;

/// Range at which CWIS starts firing visual tracers (wider than kill range).
const CWIS_VISUAL_RANGE: f32 = 150.0;

// ── Pure functions ─────────────────────────────────────────────────────

/// Check if a missile is within a PD's vertical cylinder (XZ distance only).
/// The cylinder has infinite Y extent — missiles at any altitude are valid targets.
pub fn is_in_pd_cylinder(pd_pos: Vec3, missile_pos: Vec3, radius: f32) -> bool {
    let dx = pd_pos.x - missile_pos.x;
    let dz = pd_pos.z - missile_pos.z;
    (dx * dx + dz * dz).sqrt() <= radius
}

// ── Systems ────────────────────────────────────────────────────────────

/// Laser PD: fires once per second, 60% chance to destroy the missile outright.
/// Skips friendly missiles (same team as PD ship).
fn laser_pd_fire(
    mut commands: Commands,
    mut ship_query: Query<(Entity, &Transform, &mut Mounts), With<Ship>>,
    missile_query: Query<(Entity, &Transform, &MissileOwner), With<Missile>>,
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

            for (missile_entity, missile_transform, missile_owner) in &missile_query {
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
                // 60% chance to destroy outright
                if rng.random_range(0.0..1.0) < LASER_PD_HIT_CHANCE {
                    if let Ok((_, missile_tf, _)) = missile_query.get(target_entity) {
                        crate::weapon::missile::spawn_small_explosion(
                            &mut commands,
                            missile_tf.translation,
                        );
                    }
                    commands.entity(target_entity).despawn();
                }

                mounts.0[mount_idx].weapon.as_mut().unwrap().cooldown = profile.fire_rate_secs;
            }
        }
    }
}

/// CWIS: fires 1 tracer round per tick (every 0.1s = continuous stream) with
/// probability-based missile destruction. Tracers fire at CWIS_VISUAL_RANGE (150m),
/// but kill probability only rolls within pd_cylinder_radius (100m).
fn cwis_fire(
    mut commands: Commands,
    mut ship_query: Query<(Entity, &Transform, &mut Mounts), With<Ship>>,
    missile_query: Query<(Entity, &Transform, &MissileVelocity, &MissileOwner), With<Missile>>,
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

            // Find closest ENEMY missile within VISUAL range (for tracers)
            let mut closest_dist = f32::MAX;
            let mut closest_entity = None;

            for (missile_entity, missile_transform, _, missile_owner) in &missile_query {
                if let (Some(my_team), Ok(owner_team)) =
                    (ship_team, team_query.get(missile_owner.0))
                {
                    if my_team == owner_team {
                        continue;
                    }
                }

                let missile_pos = missile_transform.translation;
                if is_in_pd_cylinder(ship_pos, missile_pos, CWIS_VISUAL_RANGE) {
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
                let (_, target_tf, target_vel, _) = missile_query.get(target_entity).unwrap();
                let target_pos = target_tf.translation;
                let target_v = target_vel.0;

                let origin = ship_pos + Vec3::new(0.0, 5.0, 0.0);

                // Lead prediction for visual tracer
                let dist = (target_pos - origin).length();
                let travel_time = if profile.projectile_speed > 0.001 {
                    dist / profile.projectile_speed
                } else {
                    0.0
                };
                let lead_pos = target_pos + target_v * travel_time;
                let dir_to_lead = (lead_pos - origin).normalize_or_zero();

                // Only roll kill chance when missile is within actual PD kill radius
                if closest_dist <= profile.pd_cylinder_radius
                    && rng.random_range(0.0..1.0) < CWIS_HIT_CHANCE
                {
                    crate::weapon::missile::spawn_small_explosion(
                        &mut commands,
                        target_pos,
                    );
                    commands.entity(target_entity).despawn();
                }

                // Spawn 1 visual tracer round (cosmetic only, CwisRound = no ship collision)
                let spread_rad =
                    rng.random_range(-CWIS_SPREAD_DEGREES..CWIS_SPREAD_DEGREES).to_radians();
                let cos_s = spread_rad.cos();
                let sin_s = spread_rad.sin();
                let spread_dir = Vec3::new(
                    dir_to_lead.x * cos_s - dir_to_lead.z * sin_s,
                    dir_to_lead.y + rng.random_range(-0.02..0.02),
                    dir_to_lead.x * sin_s + dir_to_lead.z * cos_s,
                )
                .normalize_or_zero();

                commands.spawn((
                    Projectile,
                    ProjectileVelocity(spread_dir * profile.projectile_speed),
                    ProjectileDamage(profile.damage),
                    ProjectileOwner(ship_entity),
                    CwisRound(0.5),
                    Transform::from_translation(origin),
                    Replicated,
                ));

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
            (laser_pd_fire, cwis_fire)
                .before(crate::weapon::missile::check_missile_hits)
                .run_if(in_state(GameState::Playing)),
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
