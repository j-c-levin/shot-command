//! Server system: creates/updates/despawns RadarContact entities based on SNR.

use bevy::ecs::entity::Entities;
use bevy::prelude::*;
use bevy_replicon::prelude::Replicated;
use std::collections::{HashMap, HashSet};

use crate::fog::ray_blocked_by_asteroid;
use crate::game::Team;
use crate::map::{Asteroid, AsteroidSize};
use crate::radar::{
    ContactKind, ContactLevel, ContactSourceShip, ContactTeam, ContactTracker,
    RadarActive, RadarContact, MISSILE_RCS, PROJECTILE_RCS, SIGNATURE_FUZZ_RADIUS,
    SIGNATURE_THRESHOLD, TRACK_THRESHOLD, compute_aspect_factor, compute_snr,
};
use crate::ship::{Ship, ShipClass, ship_facing_direction, ship_xz_position};
use crate::weapon::{Mounts, WeaponCategory};
use crate::weapon::missile::{Missile, MissileOwner};
use crate::weapon::projectile::{Projectile, ProjectileOwner};

/// Deterministic fuzz offset for signature-level contacts.
/// Uses entity index bits to produce a stable offset that varies by how close
/// the SNR is to the track threshold.
fn fuzz_offset(entity: Entity, snr: f32) -> Vec2 {
    let bits = entity.to_bits();
    // Use entity bits for a stable pseudo-random angle
    let angle = ((bits & 0xFFFF) as f32) * std::f32::consts::TAU / 65536.0;
    // Scale: full fuzz at SIGNATURE_THRESHOLD, zero fuzz at TRACK_THRESHOLD
    let t = ((snr - SIGNATURE_THRESHOLD) / (TRACK_THRESHOLD - SIGNATURE_THRESHOLD)).clamp(0.0, 1.0);
    let radius = SIGNATURE_FUZZ_RADIUS * (1.0 - t);
    Vec2::new(angle.cos() * radius, angle.sin() * radius)
}

/// Find the best radar range from a ship's mounts (sensors only).
fn best_radar_range(mounts: &Mounts) -> f32 {
    let mut best = 0.0_f32;
    for mount in &mounts.0 {
        if let Some(ref ws) = mount.weapon {
            if ws.weapon_type.category() == WeaponCategory::Sensor {
                let range = ws.weapon_type.profile().firing_range;
                best = best.max(range);
            }
        }
    }
    best
}

/// Core radar detection system. Runs every frame on the server.
///
/// For each team's active radars, computes SNR against all enemy ships, missiles,
/// and projectiles. Creates/updates/despawns RadarContact entities accordingly.
pub fn update_radar_contacts(
    mut commands: Commands,
    mut tracker: ResMut<ContactTracker>,
    radar_ships: Query<(&Transform, &Team, &Mounts), (With<Ship>, With<RadarActive>)>,
    all_ships: Query<(Entity, &Transform, &ShipClass, &Team), With<Ship>>,
    missile_query: Query<(Entity, &Transform, &MissileOwner), With<Missile>>,
    projectile_query: Query<(Entity, &Transform, &ProjectileOwner), With<Projectile>>,
    asteroid_query: Query<(&Transform, &AsteroidSize), With<Asteroid>>,
    ship_team_query: Query<&Team, With<Ship>>,
) {
    // Collect asteroid positions for LOS blocking
    let asteroids: Vec<(Vec2, f32)> = asteroid_query
        .iter()
        .map(|(t, s)| (Vec2::new(t.translation.x, t.translation.z), s.radius))
        .collect();

    // Best SNR per (team_id, target_entity) across all team radars
    let mut best_snr: HashMap<(u8, Entity), (f32, ContactKind)> = HashMap::new();

    // Process each active radar ship
    for (radar_transform, radar_team, mounts) in radar_ships.iter() {
        let radar_range = best_radar_range(mounts);
        if radar_range <= 0.0 {
            continue;
        }
        let radar_pos = ship_xz_position(radar_transform);
        let team_id = radar_team.0;
        let max_detect_range = radar_range * 2.0;

        // Check enemy ships
        for (target_entity, target_transform, ship_class, target_team) in all_ships.iter() {
            if target_team.0 == team_id {
                continue; // skip friendlies
            }
            let target_pos = ship_xz_position(target_transform);
            let distance = radar_pos.distance(target_pos);
            if distance > max_detect_range {
                continue; // quick range cull
            }
            if ray_blocked_by_asteroid(radar_pos, target_pos, &asteroids) {
                continue;
            }

            let radar_bearing = (target_pos - radar_pos).normalize_or_zero();
            let target_facing = ship_facing_direction(target_transform);
            let aspect = compute_aspect_factor(radar_bearing, target_facing);
            let rcs = ship_class.profile().rcs;
            let snr = compute_snr(radar_range, distance, rcs, aspect);

            if snr >= SIGNATURE_THRESHOLD {
                let key = (team_id, target_entity);
                let entry = best_snr.entry(key).or_insert((0.0, ContactKind::Ship));
                if snr > entry.0 {
                    entry.0 = snr;
                    entry.1 = ContactKind::Ship;
                }
            }
        }

        // Check enemy missiles
        for (missile_entity, missile_transform, missile_owner) in missile_query.iter() {
            // Determine missile team from owner ship
            let owner_team = match ship_team_query.get(missile_owner.0) {
                Ok(t) => t.0,
                Err(_) => continue, // owner ship gone
            };
            if owner_team == team_id {
                continue; // skip friendly missiles
            }

            let target_pos = Vec2::new(
                missile_transform.translation.x,
                missile_transform.translation.z,
            );
            let distance = radar_pos.distance(target_pos);
            if distance > max_detect_range {
                continue;
            }
            if ray_blocked_by_asteroid(radar_pos, target_pos, &asteroids) {
                continue;
            }

            let snr = compute_snr(radar_range, distance, MISSILE_RCS, 1.0);

            if snr >= SIGNATURE_THRESHOLD {
                let key = (team_id, missile_entity);
                let entry = best_snr.entry(key).or_insert((0.0, ContactKind::Missile));
                if snr > entry.0 {
                    entry.0 = snr;
                    entry.1 = ContactKind::Missile;
                }
            }
        }

        // Check enemy projectiles
        for (proj_entity, proj_transform, proj_owner) in projectile_query.iter() {
            let owner_team = match ship_team_query.get(proj_owner.0) {
                Ok(t) => t.0,
                Err(_) => continue,
            };
            if owner_team == team_id {
                continue;
            }

            let target_pos = Vec2::new(
                proj_transform.translation.x,
                proj_transform.translation.z,
            );
            let distance = radar_pos.distance(target_pos);
            if distance > max_detect_range {
                continue;
            }
            if ray_blocked_by_asteroid(radar_pos, target_pos, &asteroids) {
                continue;
            }

            let snr = compute_snr(radar_range, distance, PROJECTILE_RCS, 1.0);

            if snr >= SIGNATURE_THRESHOLD {
                let key = (team_id, proj_entity);
                let entry = best_snr
                    .entry(key)
                    .or_insert((0.0, ContactKind::Projectile));
                if snr > entry.0 {
                    entry.0 = snr;
                    entry.1 = ContactKind::Projectile;
                }
            }
        }
    }

    // Track which (team, target) pairs are currently detected
    let mut active_keys: HashSet<(u8, Entity)> = HashSet::new();

    // Create or update contacts
    for (&(team_id, target_entity), &(snr, kind)) in &best_snr {
        active_keys.insert((team_id, target_entity));

        let level = if snr >= TRACK_THRESHOLD {
            ContactLevel::Track
        } else {
            ContactLevel::Signature
        };

        // Determine display position
        let display_pos = match kind {
            ContactKind::Ship => {
                if let Ok((_, t, _, _)) = all_ships.get(target_entity) {
                    let pos = ship_xz_position(t);
                    match level {
                        ContactLevel::Track => pos,
                        ContactLevel::Signature => pos + fuzz_offset(target_entity, snr),
                    }
                } else {
                    continue;
                }
            }
            ContactKind::Missile => {
                if let Ok((_, t, _)) = missile_query.get(target_entity) {
                    let pos = Vec2::new(t.translation.x, t.translation.z);
                    match level {
                        ContactLevel::Track => pos,
                        ContactLevel::Signature => pos + fuzz_offset(target_entity, snr),
                    }
                } else {
                    continue;
                }
            }
            ContactKind::Projectile => {
                if let Ok((_, t, _)) = projectile_query.get(target_entity) {
                    let pos = Vec2::new(t.translation.x, t.translation.z);
                    match level {
                        ContactLevel::Track => pos,
                        ContactLevel::Signature => pos + fuzz_offset(target_entity, snr),
                    }
                } else {
                    continue;
                }
            }
        };

        let key = (team_id, target_entity);

        if let Some(&contact_entity) = tracker.contacts.get(&key) {
            // Update existing contact
            if let Ok(mut entity_commands) = commands.get_entity(contact_entity) {
                entity_commands.insert((
                    level,
                    Transform::from_xyz(display_pos.x, 0.0, display_pos.y),
                ));
            }
        } else {
            // Create new contact
            let contact_id = tracker.allocate_id(team_id);
            let contact_entity = commands
                .spawn((
                    RadarContact,
                    level,
                    ContactTeam(Team(team_id)),
                    ContactSourceShip(target_entity),
                    contact_id,
                    kind,
                    Transform::from_xyz(display_pos.x, 0.0, display_pos.y),
                    Replicated,
                ))
                .id();
            tracker.contacts.insert(key, contact_entity);
        }
    }

    // Despawn contacts that are no longer detected
    let stale_keys: Vec<(u8, Entity)> = tracker
        .contacts
        .keys()
        .filter(|k| !active_keys.contains(k))
        .cloned()
        .collect();

    for key in stale_keys {
        if let Some(contact_entity) = tracker.contacts.remove(&key) {
            if let Ok(mut entity_commands) = commands.get_entity(contact_entity) {
                entity_commands.despawn();
            }
        }
    }
}

/// Remove contacts whose source entity no longer exists (e.g., ship destroyed).
pub fn cleanup_stale_contacts(
    mut commands: Commands,
    mut tracker: ResMut<ContactTracker>,
    entities: &Entities,
) {
    let stale: Vec<(u8, Entity)> = tracker
        .contacts
        .keys()
        .filter(|(_, source)| !entities.contains(*source))
        .cloned()
        .collect();

    for key in stale {
        if let Some(contact_entity) = tracker.contacts.remove(&key) {
            if let Ok(mut entity_commands) = commands.get_entity(contact_entity) {
                entity_commands.despawn();
            }
        }
    }
}
