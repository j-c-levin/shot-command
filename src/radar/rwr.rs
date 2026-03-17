//! RWR (Radar Warning Receiver) bearing detection.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::fog::ray_blocked_by_asteroid;
use crate::game::Team;
use crate::map::{Asteroid, AsteroidSize};
use crate::radar::RadarActive;
use crate::ship::{Ship, ShipSecrets, ShipSecretsOwner, ship_xz_position};
use crate::weapon::{Mounts, WeaponCategory};

/// RWR bearing lines for a ship — directions toward enemy radar sources.
/// Lives on ShipSecrets entities (team-private).
#[derive(Component, Serialize, Deserialize, Clone, Debug, Default)]
pub struct RwrBearings(pub Vec<Vec2>);

/// Returns true if target_pos is within radar_range of radar_pos.
pub fn is_in_rwr_range(radar_pos: Vec2, radar_range: f32, target_pos: Vec2) -> bool {
    radar_pos.distance(target_pos) <= radar_range
}

/// Updates RwrBearings on the target's ShipSecrets entity.
///
/// For each active radar, checks every enemy ship that has radar hardware.
/// If the enemy is within radar range, a bearing toward the radar source
/// is added to the enemy's RwrBearings (on their ShipSecrets).
pub fn update_rwr_bearings(
    radar_ships: Query<(&Transform, &Team, &Mounts), (With<Ship>, With<RadarActive>)>,
    all_ships: Query<(Entity, &Transform, &Team, &Mounts), With<Ship>>,
    mut secrets_query: Query<(&ShipSecretsOwner, &mut RwrBearings), With<ShipSecrets>>,
    asteroid_query: Query<(&Transform, &AsteroidSize), With<Asteroid>>,
) {
    let asteroids: Vec<(Vec2, f32)> = asteroid_query
        .iter()
        .map(|(t, s)| (Vec2::new(t.translation.x, t.translation.z), s.radius))
        .collect();

    // Clear all bearings
    for (_, mut bearings) in &mut secrets_query {
        bearings.0.clear();
    }

    for (radar_transform, radar_team, radar_mounts) in &radar_ships {
        let radar_range = radar_mounts
            .0
            .iter()
            .filter_map(|m| m.weapon.as_ref())
            .filter(|w| w.weapon_type.category() == WeaponCategory::Sensor)
            .map(|w| w.weapon_type.profile().firing_range)
            .fold(0.0_f32, f32::max);

        let radar_pos = ship_xz_position(radar_transform);

        for (target_entity, target_transform, target_team, target_mounts) in &all_ships {
            if target_team.0 == radar_team.0 {
                continue;
            }

            let has_radar_hw = target_mounts.0.iter().any(|m| {
                m.weapon
                    .as_ref()
                    .is_some_and(|w| w.weapon_type.category() == WeaponCategory::Sensor)
            });
            if !has_radar_hw {
                continue;
            }

            let target_pos = ship_xz_position(target_transform);
            if !is_in_rwr_range(radar_pos, radar_range, target_pos) {
                continue;
            }
            if ray_blocked_by_asteroid(radar_pos, target_pos, &asteroids) {
                continue;
            }

            let bearing = (radar_pos - target_pos).normalize_or_zero();
            for (owner, mut bearings) in &mut secrets_query {
                if owner.0 == target_entity {
                    bearings.0.push(bearing);
                }
            }
        }
    }
}
