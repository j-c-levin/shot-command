use bevy::prelude::*;
use rand::Rng;

use crate::ship::{
    Ship, ShipClass, TargetDesignation, Velocity, angle_between_directions,
    ship_facing_direction, ship_xz_position,
};
use crate::weapon::projectile::spawn_projectile;
use crate::weapon::{FiringArc, Mounts};

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
            angle <= 0.1745 // ~10 degrees in radians
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

            if weapon.cooldown > 0.0 || weapon.ammo == 0 {
                continue;
            }

            let profile = weapon.weapon_type.profile();

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

            for _ in 0..profile.burst_count {
                // Apply random spread
                let spread_rad = rng
                    .random_range(-profile.spread_degrees..profile.spread_degrees)
                    .to_radians();
                let cos_s = spread_rad.cos();
                let sin_s = spread_rad.sin();
                let spread_dir = Vec3::new(
                    dir_to_lead.x * cos_s - dir_to_lead.z * sin_s,
                    0.0,
                    dir_to_lead.x * sin_s + dir_to_lead.z * cos_s,
                );

                spawn_projectile(
                    &mut commands,
                    origin,
                    spread_dir,
                    profile.projectile_speed,
                    profile.damage,
                    ship_entity,
                );
            }

            // Update weapon state
            let weapon_mut = mounts.0[mount_idx].weapon.as_mut().unwrap();
            weapon_mut.ammo = weapon_mut.ammo.saturating_sub(profile.burst_count as u16);
            weapon_mut.cooldown = profile.fire_rate_secs;
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
}
