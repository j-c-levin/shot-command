//! Client gizmo rendering for radar contacts and radar status indicators.

use bevy::prelude::*;

use crate::game::Team;
use crate::net::LocalTeam;
use crate::radar::{ContactKind, ContactLevel, ContactTeam, RadarActiveSecret, RadarContact};
use crate::radar::rwr::RwrBearings;
use crate::ship::{Ship, ShipSecrets, ShipSecretsOwner};
use crate::weapon::{Mounts, WeaponCategory};

/// Blue/grey circle above own ships indicating radar on/off.
pub fn draw_radar_status_gizmos(
    mut gizmos: Gizmos,
    local_team: Res<LocalTeam>,
    ships: Query<(Entity, &Transform, &Team, &Mounts), With<Ship>>,
    secrets: Query<(&ShipSecretsOwner, &RadarActiveSecret), With<ShipSecrets>>,
) {
    let Some(my_team) = local_team.0 else { return };
    for (ship_entity, transform, team, mounts) in &ships {
        if *team != my_team {
            continue;
        }
        let has_radar = mounts.0.iter().any(|m| {
            m.weapon
                .as_ref()
                .is_some_and(|w| w.weapon_type.category() == WeaponCategory::Sensor)
        });
        if !has_radar {
            continue;
        }
        let is_active = secrets
            .iter()
            .find(|(owner, _)| owner.0 == ship_entity)
            .map(|(_, active)| active.0)
            .unwrap_or(false);
        let pos = transform.translation + Vec3::Y * 15.0;
        let color = if is_active {
            Color::srgb(0.2, 0.5, 1.0)
        } else {
            Color::srgb(0.4, 0.4, 0.4)
        };
        gizmos.circle(
            Isometry3d::new(pos, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            3.0,
            color,
        );
    }
}

/// Pulsing orange circles for radar signature contacts (ships only).
pub fn draw_radar_signature_gizmos(
    mut gizmos: Gizmos,
    local_team: Res<LocalTeam>,
    time: Res<Time>,
    contacts: Query<(&Transform, &ContactLevel, &ContactTeam, &ContactKind), With<RadarContact>>,
) {
    let Some(my_team) = local_team.0 else { return };
    for (transform, level, contact_team, kind) in &contacts {
        if *level != ContactLevel::Signature || contact_team.0 != my_team {
            continue;
        }
        if *kind != ContactKind::Ship {
            continue;
        }
        let pos = transform.translation;
        let pulse = 0.7 + 0.3 * (time.elapsed_secs() * 2.0).sin();
        let radius = 20.0 * pulse;
        let color = Color::srgba(1.0, 0.5, 0.0, 0.4 * pulse);
        gizmos.circle(
            Isometry3d::new(pos, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            radius,
            color,
        );
    }
}

/// Red diamond markers for radar track contacts (ships only).
pub fn draw_radar_track_gizmos(
    mut gizmos: Gizmos,
    local_team: Res<LocalTeam>,
    contacts: Query<(&Transform, &ContactLevel, &ContactTeam, &ContactKind), With<RadarContact>>,
) {
    let Some(my_team) = local_team.0 else { return };
    for (transform, level, contact_team, kind) in &contacts {
        if *level != ContactLevel::Track || contact_team.0 != my_team {
            continue;
        }
        if *kind != ContactKind::Ship {
            continue;
        }
        let pos = transform.translation;
        let color = Color::srgb(1.0, 0.2, 0.2);
        let size = 5.0;
        let top = pos + Vec3::Z * size;
        let bottom = pos - Vec3::Z * size;
        let left = pos - Vec3::X * size;
        let right = pos + Vec3::X * size;
        gizmos.line(top, right, color);
        gizmos.line(right, bottom, color);
        gizmos.line(bottom, left, color);
        gizmos.line(left, top, color);
    }
}

/// Orange X markers for radar-tracked missiles.
pub fn draw_tracked_missile_gizmos(
    mut gizmos: Gizmos,
    local_team: Res<LocalTeam>,
    contacts: Query<(&Transform, &ContactLevel, &ContactTeam, &ContactKind), With<RadarContact>>,
) {
    let Some(my_team) = local_team.0 else { return };
    for (transform, level, contact_team, kind) in &contacts {
        if contact_team.0 != my_team {
            continue;
        }
        if *kind != ContactKind::Missile {
            continue;
        }
        if *level != ContactLevel::Track {
            continue;
        }
        let pos = transform.translation;
        let color = Color::srgb(1.0, 0.4, 0.0);
        let size = 2.5;
        gizmos.line(
            pos + Vec3::new(-size, 0.0, -size),
            pos + Vec3::new(size, 0.0, size),
            color,
        );
        gizmos.line(
            pos + Vec3::new(-size, 0.0, size),
            pos + Vec3::new(size, 0.0, -size),
            color,
        );
    }
}

/// Yellow RWR bearing lines from own ships.
pub fn draw_rwr_gizmos(
    mut gizmos: Gizmos,
    local_team: Res<LocalTeam>,
    ships: Query<(Entity, &Transform, &Team), With<Ship>>,
    secrets: Query<(&ShipSecretsOwner, &RwrBearings), With<ShipSecrets>>,
) {
    let Some(my_team) = local_team.0 else { return };
    for (ship_entity, transform, team) in &ships {
        if *team != my_team {
            continue;
        }
        let Some((_, bearings)) = secrets.iter().find(|(owner, _)| owner.0 == ship_entity)
        else {
            continue;
        };
        let ship_pos = transform.translation;
        let color = Color::srgb(1.0, 1.0, 0.0);
        for bearing in &bearings.0 {
            let end = ship_pos + Vec3::new(bearing.x, 0.0, bearing.y) * 100.0;
            gizmos.line(ship_pos, end, color);
        }
    }
}
