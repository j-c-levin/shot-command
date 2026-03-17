use bevy::prelude::*;
use bevy_replicon::prelude::Replicated;
use rand::Rng;

use serde::{Deserialize, Serialize};

use crate::game::{GameState, Team};
use crate::radar::RadarActive;
use crate::ship::{Ship, ShipClass};
use crate::weapon::missile::{Missile, MissileOwner, MissileVelocity};
use crate::weapon::WeaponCategory;

/// Marker for a laser beam visual entity (server-spawned, client-materialized).
#[derive(Component, Serialize, Deserialize)]
pub struct LaserBeam;

/// The target end-point of the laser beam (beam goes from entity Transform to this point).
#[derive(Component, Serialize, Deserialize, Clone)]
pub struct LaserBeamTarget(pub Vec3);

/// Remaining time before the beam despawns.
#[derive(Component, Serialize, Deserialize, Clone)]
pub struct LaserBeamTimer(pub f32);

/// Server-only: the missile entity this beam is tracking (not replicated).
#[derive(Component)]
pub struct LaserBeamTracking(pub Entity);

/// Server-only: the ship entity the beam originates from (not replicated).
#[derive(Component)]
pub struct LaserBeamOrigin(pub Entity);

/// Server-only: if present, the beam will destroy its tracked missile after this delay.
#[derive(Component)]
pub struct LaserBeamKill(pub f32);
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

/// Multiplier for CWIS ranges when engaging a radar-tracked missile.
const CWIS_RADAR_RANGE_MULTIPLIER: f32 = 2.0;

/// Seconds a PD mount must wait before engaging a new target after a kill.
const PD_RETARGET_DELAY: f32 = 0.2;

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
    mut ship_query: Query<(Entity, &Transform, &ShipClass, &mut Mounts, Option<&RadarActive>), With<Ship>>,
    missile_query: Query<(Entity, &Transform, &MissileOwner), With<Missile>>,
    team_query: Query<&Team>,
) {
    let mut rng = rand::rng();

    for (ship_entity, ship_transform, ship_class, mut mounts, radar_active) in &mut ship_query {
        let ship_pos = ship_transform.translation;
        let ship_team = team_query.get(ship_entity).ok();

        // Precompute radar range for this ship (0.0 if no radar)
        let radar_range = if radar_active.is_some() {
            mounts.0.iter()
                .filter_map(|m| m.weapon.as_ref())
                .filter(|w| w.weapon_type.category() == WeaponCategory::Sensor)
                .map(|w| w.weapon_type.profile().firing_range)
                .fold(0.0_f32, f32::max)
        } else {
            0.0
        };
        let vision_range = ship_class.profile().vision_range;

        for mount_idx in 0..mounts.0.len() {
            if mounts.0[mount_idx].hp == 0 {
                continue;
            }
            let Some(ref weapon) = mounts.0[mount_idx].weapon else {
                continue;
            };

            if weapon.weapon_type != WeaponType::LaserPD {
                continue;
            }

            if weapon.cooldown > 0.0 || weapon.pd_retarget_cooldown > 0.0 {
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

                // Check if this ship can detect the missile (visual LOS or radar)
                let distance = ship_pos.distance(missile_pos);
                let in_visual_los = distance <= vision_range;
                let in_radar_range = radar_active.is_some() && distance <= radar_range;
                if !in_visual_los && !in_radar_range {
                    continue;
                }

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
                let target_pos = missile_query.get(target_entity).unwrap().1.translation;

                // Roll kill chance now, but delay the actual kill so the beam is visible first
                let killed = rng.random_range(0.0..1.0) < LASER_PD_HIT_CHANCE;

                // Spawn visible laser beam from ship to missile (tracks both endpoints)
                let beam_origin = ship_pos + Vec3::new(0.0, 5.0, 0.0);
                let mut beam = commands.spawn((
                    LaserBeam,
                    LaserBeamTarget(target_pos),
                    LaserBeamTimer(0.4),
                    LaserBeamTracking(target_entity),
                    LaserBeamOrigin(ship_entity),
                    Transform::from_translation(beam_origin),
                    Replicated,
                ));
                if killed {
                    // Kill will happen after 0.15s delay (beam visible first, then explosion)
                    beam.insert(LaserBeamKill(0.15));
                }

                let ws = mounts.0[mount_idx].weapon.as_mut().unwrap();
                ws.cooldown = profile.fire_rate_secs;
                if killed {
                    ws.pd_retarget_cooldown = PD_RETARGET_DELAY + 0.15; // account for kill delay
                }
            }
        }
    }
}

/// CWIS: fires 1 tracer round per tick (every 0.1s = continuous stream) with
/// probability-based missile destruction. Tracers fire at CWIS_VISUAL_RANGE (150m),
/// but kill probability only rolls within pd_cylinder_radius (100m).
fn cwis_fire(
    mut commands: Commands,
    mut ship_query: Query<(Entity, &Transform, &ShipClass, &mut Mounts, Option<&RadarActive>), With<Ship>>,
    missile_query: Query<(Entity, &Transform, &MissileVelocity, &MissileOwner), With<Missile>>,
    team_query: Query<&Team>,
) {
    let mut rng = rand::rng();

    for (ship_entity, ship_transform, ship_class, mut mounts, radar_active) in &mut ship_query {
        let ship_pos = ship_transform.translation;
        let ship_team = team_query.get(ship_entity).ok();

        // Precompute radar range for this ship (0.0 if no radar)
        let radar_range = if radar_active.is_some() {
            mounts.0.iter()
                .filter_map(|m| m.weapon.as_ref())
                .filter(|w| w.weapon_type.category() == WeaponCategory::Sensor)
                .map(|w| w.weapon_type.profile().firing_range)
                .fold(0.0_f32, f32::max)
        } else {
            0.0
        };
        let vision_range = ship_class.profile().vision_range;

        for mount_idx in 0..mounts.0.len() {
            if mounts.0[mount_idx].hp == 0 {
                continue;
            }
            let Some(ref weapon) = mounts.0[mount_idx].weapon else {
                continue;
            };

            if weapon.weapon_type != WeaponType::CWIS {
                continue;
            }

            if weapon.cooldown > 0.0 || weapon.pd_retarget_cooldown > 0.0 {
                continue;
            }

            let profile = weapon.weapon_type.profile();

            // Find closest ENEMY missile within VISUAL range (for tracers).
            // CWIS ranges double when engaging a radar-tracked missile.
            let mut closest_dist = f32::MAX;
            let mut closest_entity = None;
            let mut closest_radar_tracked = false;

            for (missile_entity, missile_transform, _, missile_owner) in &missile_query {
                if let (Some(my_team), Ok(owner_team)) =
                    (ship_team, team_query.get(missile_owner.0))
                {
                    if my_team == owner_team {
                        continue;
                    }
                }

                let missile_pos = missile_transform.translation;

                // Check if this ship can detect the missile (visual LOS or radar)
                let distance = ship_pos.distance(missile_pos);
                let in_visual_los = distance <= vision_range;
                let in_radar_range = radar_active.is_some() && distance <= radar_range;
                if !in_visual_los && !in_radar_range {
                    continue;
                }

                // CWIS visual range doubles for radar-tracked missiles
                let effective_visual_range = if in_radar_range {
                    CWIS_VISUAL_RANGE * CWIS_RADAR_RANGE_MULTIPLIER
                } else {
                    CWIS_VISUAL_RANGE
                };

                if is_in_pd_cylinder(ship_pos, missile_pos, effective_visual_range) {
                    let dx = ship_pos.x - missile_pos.x;
                    let dz = ship_pos.z - missile_pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist < closest_dist {
                        closest_dist = dist;
                        closest_entity = Some(missile_entity);
                        closest_radar_tracked = in_radar_range;
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

                // CWIS kill radius doubles for radar-tracked missiles
                let effective_kill_radius = if closest_radar_tracked {
                    profile.pd_cylinder_radius * CWIS_RADAR_RANGE_MULTIPLIER
                } else {
                    profile.pd_cylinder_radius
                };

                // Only roll kill chance when missile is within actual PD kill radius
                let killed = closest_dist <= effective_kill_radius
                    && rng.random_range(0.0..1.0) < CWIS_HIT_CHANCE;
                if killed {
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

                // Tracer lifetime scales with radar boost so rounds reach the target
                let tracer_lifetime = if closest_radar_tracked {
                    0.5 * CWIS_RADAR_RANGE_MULTIPLIER
                } else {
                    0.5
                };
                commands.spawn((
                    Projectile,
                    ProjectileVelocity(spread_dir * profile.projectile_speed),
                    ProjectileDamage(profile.damage),
                    ProjectileOwner(ship_entity),
                    CwisRound(tracer_lifetime),
                    Transform::from_translation(origin),
                    Replicated,
                ));

                let ws = mounts.0[mount_idx].weapon.as_mut().unwrap();
                ws.cooldown = profile.fire_rate_secs;
                if killed {
                    ws.pd_retarget_cooldown = PD_RETARGET_DELAY;
                }
            }
        }
    }
}

/// Process delayed laser kills: after the beam has been visible for a short time,
/// destroy the missile and spawn an explosion.
fn process_laser_kills(
    mut commands: Commands,
    time: Res<Time>,
    mut beam_query: Query<
        (Entity, &LaserBeamTracking, &mut LaserBeamKill),
        With<LaserBeam>,
    >,
    missile_query: Query<&Transform, With<Missile>>,
) {
    let dt = time.delta_secs();
    for (beam_entity, tracking, mut kill) in &mut beam_query {
        kill.0 -= dt;
        if kill.0 <= 0.0 {
            // Destroy the missile now (if it still exists — CWIS may have killed it)
            if let Ok(missile_tf) = missile_query.get(tracking.0) {
                crate::weapon::missile::spawn_small_explosion(
                    &mut commands,
                    missile_tf.translation,
                );
                commands.entity(tracking.0).despawn();
            }
            commands.entity(beam_entity).remove::<LaserBeamKill>();
        }
    }
}

/// Update laser beam endpoints to track the missile and ship each frame.
fn update_laser_beams(
    mut commands: Commands,
    mut beam_query: Query<
        (Entity, &mut Transform, &mut LaserBeamTarget, &LaserBeamTracking, &LaserBeamOrigin),
        With<LaserBeam>,
    >,
    transform_query: Query<&Transform, Without<LaserBeam>>,
) {
    for (beam_entity, mut beam_tf, mut beam_target, tracking, origin) in &mut beam_query {
        // Update origin to ship's current position
        if let Ok(ship_tf) = transform_query.get(origin.0) {
            beam_tf.translation = ship_tf.translation + Vec3::new(0.0, 5.0, 0.0);
        }
        // Update target to missile's current position (if still alive)
        if let Ok(missile_tf) = transform_query.get(tracking.0) {
            beam_target.0 = missile_tf.translation;
        } else {
            // Missile gone — despawn beam
            commands.entity(beam_entity).despawn();
        }
    }
}

/// Tick laser beam timers and despawn expired beams.
fn tick_laser_beams(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut LaserBeamTimer), With<LaserBeam>>,
) {
    let dt = time.delta_secs();
    for (entity, mut timer) in &mut query {
        timer.0 -= dt;
        if timer.0 <= 0.0 {
            commands.entity(entity).despawn();
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
        app.add_systems(
            Update,
            (process_laser_kills, update_laser_beams, tick_laser_beams)
                .chain()
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
