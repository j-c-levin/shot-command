use bevy::prelude::*;
use rand::Rng;

use crate::ship::{
    Ship, ShipClass, TargetDesignation, Velocity, angle_between_directions,
    ship_facing_direction, ship_xz_position,
};
use crate::weapon::missile::{compute_intercept_point, spawn_missile};
use crate::weapon::projectile::spawn_projectile;
use crate::weapon::{FiringArc, MissileQueue, Mounts, WeaponCategory};

/// Half-angle of the forward firing arc: 10 degrees in radians (PI / 18).
const FORWARD_ARC_HALF_ANGLE: f32 = std::f32::consts::PI / 18.0;

// ── Pure functions ─────────────────────────────────────────────────────

/// Predict where the target will be when a projectile arrives.
///
/// Two-iteration linear prediction: estimate travel time from distance,
/// predict target position, then refine once.
pub fn compute_lead_position(
    shooter_pos: Vec3,
    target_pos: Vec3,
    target_velocity: Vec2,
    projectile_speed: f32,
) -> Vec3 {
    if projectile_speed < 0.001 {
        return target_pos;
    }

    // First estimate
    let dist = (shooter_pos - target_pos).length();
    let travel_time = dist / projectile_speed;
    let vel3 = Vec3::new(target_velocity.x, 0.0, target_velocity.y);
    let predicted = target_pos + vel3 * travel_time;

    // Refine once
    let dist2 = (shooter_pos - predicted).length();
    let travel_time2 = dist2 / projectile_speed;
    let refined = target_pos + vel3 * travel_time2;

    // Keep same Y as target
    Vec3::new(refined.x, target_pos.y, refined.z)
}

/// Check whether a target direction falls within the weapon's firing arc.
///
/// `Turret` arcs have 360-degree coverage. `Forward` arcs allow +/-10 degrees
/// from the ship's facing direction.
pub fn is_in_firing_arc(ship_facing: Vec2, target_direction: Vec2, arc: &FiringArc) -> bool {
    match arc {
        FiringArc::Turret => true,
        FiringArc::Forward => {
            let angle = angle_between_directions(ship_facing, target_direction);
            angle <= FORWARD_ARC_HALF_ANGLE
        }
    }
}

// ── Systems ────────────────────────────────────────────────────────────

/// Decrement cooldown timers on all weapon mounts every frame.
pub fn tick_weapon_cooldowns(time: Res<Time>, mut query: Query<&mut Mounts, With<Ship>>) {
    let dt = time.delta_secs();
    for mut mounts in &mut query {
        for mount in mounts.0.iter_mut() {
            if let Some(ref mut weapon) = mount.weapon {
                weapon.cooldown = (weapon.cooldown - dt).max(0.0);
            }
        }
    }
}

/// Automatically fire weapons at designated targets.
///
/// For each ship with a `TargetDesignation`, checks each weapon mount for
/// readiness (cooldown, ammo, range, arc) and spawns projectiles with lead
/// prediction and random spread.
pub fn auto_fire(
    mut commands: Commands,
    mut firing_ships: Query<
        (
            Entity,
            &Transform,
            &ShipClass,
            &mut Mounts,
            &TargetDesignation,
        ),
        With<Ship>,
    >,
    target_query: Query<(&Transform, &Velocity), With<Ship>>,
) {
    for (ship_entity, ship_transform, _ship_class, mut mounts, target) in &mut firing_ships {
        // Check target still exists as a Ship
        let Ok((target_transform, target_velocity)) = target_query.get(target.0) else {
            commands.entity(ship_entity).remove::<TargetDesignation>();
            continue;
        };

        let ship_pos = ship_xz_position(ship_transform);
        let target_pos_xz = ship_xz_position(target_transform);
        let ship_facing = ship_facing_direction(ship_transform);
        let to_target = (target_pos_xz - ship_pos).normalize_or_zero();
        let range = (target_pos_xz - ship_pos).length();

        for mount_idx in 0..mounts.0.len() {
            let mount = &mounts.0[mount_idx];
            let Some(ref weapon) = mount.weapon else {
                continue;
            };

            if weapon.cooldown > 0.0 {
                continue;
            }

            let profile = weapon.weapon_type.profile();

            // Only cannons auto-fire; missiles and PD are handled separately
            if weapon.weapon_type.category() != WeaponCategory::Cannon {
                continue;
            }

            // Range check
            if range > profile.firing_range {
                continue;
            }

            // Arc check
            if !is_in_firing_arc(ship_facing, to_target, &profile.arc) {
                continue;
            }

            // Compute lead position
            let lead = compute_lead_position(
                ship_transform.translation,
                target_transform.translation,
                target_velocity.linear,
                profile.projectile_speed,
            );

            // Compute spawn origin: ship position + rotated mount offset
            let cos = ship_facing.x;
            let sin = ship_facing.y;
            let offset = mounts.0[mount_idx].offset;
            let rotated = Vec2::new(
                offset.x * cos - offset.y * sin,
                offset.x * sin + offset.y * cos,
            );
            let origin = Vec3::new(ship_pos.x + rotated.x, 5.0, ship_pos.y + rotated.y);

            // Direction from origin to lead position (XZ plane)
            let dir_to_lead = (lead - origin).normalize_or_zero();

            let mut rng = rand::rng();

            // Perpendicular vector in XZ plane for parallel spread
            let perp = Vec3::new(-dir_to_lead.z, 0.0, dir_to_lead.x);

            for i in 0..profile.burst_count {
                // Parallel offset: evenly space projectiles perpendicular to aim
                let parallel_offset = if profile.burst_count > 1 {
                    let spacing = profile.spread_degrees; // reuse as meters of separation
                    let t = i as f32 - (profile.burst_count - 1) as f32 / 2.0;
                    perp * t * spacing
                } else {
                    Vec3::ZERO
                };

                // Small random accuracy spread (±0.5°)
                let accuracy_rad = rng.random_range(-0.5_f32..0.5_f32).to_radians();
                let cos_s = accuracy_rad.cos();
                let sin_s = accuracy_rad.sin();
                let spread_dir = Vec3::new(
                    dir_to_lead.x * cos_s - dir_to_lead.z * sin_s,
                    0.0,
                    dir_to_lead.x * sin_s + dir_to_lead.z * cos_s,
                );

                spawn_projectile(
                    &mut commands,
                    origin + parallel_offset,
                    spread_dir,
                    profile.projectile_speed,
                    profile.damage,
                    ship_entity,
                );
            }

            // Update weapon state
            let weapon_mut = mounts.0[mount_idx].weapon.as_mut().unwrap();
            weapon_mut.cooldown = profile.fire_rate_secs;
        }
    }
}

/// Process missile launch queues: pop entries and spawn missiles from ready VLS tubes.
pub fn process_missile_queue(
    mut commands: Commands,
    mut ships: Query<(Entity, &Transform, &mut Mounts, &mut MissileQueue), With<Ship>>,
    target_query: Query<(&Transform, &Velocity), With<Ship>>,
) {
    for (ship_entity, ship_transform, mut mounts, mut queue) in &mut ships {
        if queue.0.is_empty() {
            continue;
        }

        for mount_idx in 0..mounts.0.len() {
            if queue.0.is_empty() {
                break;
            }

            let mount = &mounts.0[mount_idx];
            let Some(ref weapon) = mount.weapon else {
                continue;
            };

            if weapon.weapon_type.category() != WeaponCategory::Missile {
                continue;
            }

            if weapon.cooldown > 0.0 {
                continue;
            }

            let profile = weapon.weapon_type.profile();

            // Fire 1 missile per mount per tick (rapid fire until queue drained)
            {
                let entry = queue.0.remove(0);

                // Compute intercept point
                let intercept = if let Some(target_entity) = entry.target_entity {
                    if let Ok((target_transform, target_velocity)) =
                        target_query.get(target_entity)
                    {
                        compute_intercept_point(
                            ship_transform.translation,
                            target_transform.translation,
                            target_velocity.linear,
                            profile.projectile_speed,
                        )
                    } else {
                        Vec3::new(entry.target_point.x, 0.0, entry.target_point.y)
                    }
                } else {
                    Vec3::new(entry.target_point.x, 0.0, entry.target_point.y)
                };

                let origin = ship_transform.translation + Vec3::new(0.0, 5.0, 0.0);

                spawn_missile(
                    &mut commands,
                    origin,
                    intercept,
                    entry.target_entity,
                    profile.projectile_speed,
                    profile.damage,
                    profile.missile_hp,
                    profile.missile_fuel,
                    ship_entity,
                );
            }

            // Always use short inter-tube delay — VLS is a rapid-fire tube system
            // with no full reload between salvos.
            mounts.0[mount_idx].weapon.as_mut().unwrap().cooldown = 0.15;
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lead_stationary_target() {
        let shooter = Vec3::new(0.0, 5.0, 0.0);
        let target = Vec3::new(100.0, 5.0, 0.0);
        let velocity = Vec2::ZERO;

        let lead = compute_lead_position(shooter, target, velocity, 150.0);
        assert!(
            (lead - target).length() < 0.01,
            "stationary target: lead should equal target_pos, got {:?}",
            lead
        );
    }

    #[test]
    fn lead_moving_target() {
        let shooter = Vec3::new(0.0, 5.0, 0.0);
        let target = Vec3::new(100.0, 5.0, 0.0);
        let velocity = Vec2::new(50.0, 0.0); // moving +X

        let lead = compute_lead_position(shooter, target, velocity, 150.0);
        assert!(
            lead.x > target.x,
            "moving target: lead should be ahead in X, got {:?}",
            lead
        );
        // Y should match target
        assert!(
            (lead.y - target.y).abs() < 0.01,
            "Y should match target Y"
        );
    }

    #[test]
    fn lead_zero_projectile_speed() {
        let shooter = Vec3::new(0.0, 5.0, 0.0);
        let target = Vec3::new(100.0, 5.0, 50.0);
        let velocity = Vec2::new(50.0, 30.0);

        let lead = compute_lead_position(shooter, target, velocity, 0.0);
        assert!(
            (lead - target).length() < 0.01,
            "zero speed: should return target_pos directly"
        );
    }

    #[test]
    fn turret_arc_always_passes() {
        let facing = Vec2::new(1.0, 0.0);
        let behind = Vec2::new(-1.0, 0.0);
        let sideways = Vec2::new(0.0, 1.0);

        assert!(is_in_firing_arc(facing, behind, &FiringArc::Turret));
        assert!(is_in_firing_arc(facing, sideways, &FiringArc::Turret));
        assert!(is_in_firing_arc(facing, facing, &FiringArc::Turret));
    }

    #[test]
    fn forward_arc_within_cone() {
        let facing = Vec2::new(1.0, 0.0);
        // 5 degrees off — well within 10-degree cone
        let angle = 5.0_f32.to_radians();
        let direction = Vec2::new(angle.cos(), angle.sin());

        assert!(
            is_in_firing_arc(facing, direction, &FiringArc::Forward),
            "5 degrees should be within forward arc"
        );
    }

    #[test]
    fn forward_arc_outside_cone() {
        let facing = Vec2::new(1.0, 0.0);
        let perpendicular = Vec2::new(0.0, 1.0); // 90 degrees off

        assert!(
            !is_in_firing_arc(facing, perpendicular, &FiringArc::Forward),
            "90 degrees should be outside forward arc"
        );
    }

    #[test]
    fn cannon_auto_fire_skips_missile_types() {
        use crate::weapon::WeaponType;

        let profile = WeaponType::HeavyVLS.profile();
        assert_eq!(WeaponType::HeavyVLS.category(), WeaponCategory::Missile);
        // auto_fire only processes Cannon category — VLS weapons are handled
        // by process_missile_queue instead
        assert!(profile.missile_hp > 0);
        assert!(profile.tubes > 0);
    }
}
