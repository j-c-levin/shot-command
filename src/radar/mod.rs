pub mod contacts;
pub mod rwr;
pub mod visuals;

use bevy::ecs::entity::MapEntities;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::game::{GameState, Team};

/// Minimum SNR to appear as a signature (fuzzy blob on radar).
pub const SIGNATURE_THRESHOLD: f32 = 0.1;
/// Minimum SNR to achieve a track (precise position and heading).
pub const TRACK_THRESHOLD: f32 = 0.4;
/// Radius of the fuzzy position blob for signature-level contacts.
pub const SIGNATURE_FUZZ_RADIUS: f32 = 75.0;
/// Radar cross-section for missiles.
pub const MISSILE_RCS: f32 = 0.05;
/// Radar cross-section for projectiles.
pub const PROJECTILE_RCS: f32 = 0.02;

/// Compute the aspect factor for radar detection.
///
/// Broadside presentation (perpendicular to radar bearing) yields 1.0.
/// Nose-on or tail-on (parallel to radar bearing) yields 0.25.
/// Uses cross product magnitude to measure perpendicularity.
pub fn compute_aspect_factor(radar_bearing: Vec2, target_facing: Vec2) -> f32 {
    let cross = radar_bearing.x * target_facing.y - radar_bearing.y * target_facing.x;
    let sin_angle = cross.abs().clamp(0.0, 1.0);
    0.25 + 0.75 * sin_angle
}

/// Compute signal-to-noise ratio for radar detection.
///
/// Formula: (radar_range² / distance²) × target_rcs × aspect_factor
///
/// Returns a value where:
/// - >= TRACK_THRESHOLD means precise track
/// - >= SIGNATURE_THRESHOLD means fuzzy signature
/// - < SIGNATURE_THRESHOLD means undetected
pub fn compute_snr(radar_range: f32, distance: f32, target_rcs: f32, aspect_factor: f32) -> f32 {
    if distance <= 0.0 {
        return f32::MAX;
    }
    (radar_range * radar_range / (distance * distance)) * target_rcs * aspect_factor
}

/// Marker for a ship with its radar currently active. SERVER-ONLY — not replicated.
#[derive(Component, Clone, Debug)]
pub struct RadarActive;

/// Replicated component on ShipSecrets to tell the owning team if radar is on.
#[derive(Component, Serialize, Deserialize, Clone, Debug, Default)]
pub struct RadarActiveSecret(pub bool);

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContactLevel {
    Signature,
    Track,
}

#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct RadarContact;

/// Links a RadarContact back to the actual ship it represents.
/// Uses #[derive(MapEntities)] with #[entities] for replication entity mapping.
#[derive(Component, Serialize, Deserialize, Clone, Debug, MapEntities)]
pub struct ContactSourceShip(#[entities] pub Entity);

#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct ContactTeam(pub Team);

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ContactId(pub u8);

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContactKind {
    Ship,
    Missile,
    Projectile,
}

#[derive(Resource, Default, Debug)]
pub struct ContactTracker {
    pub contacts: HashMap<(u8, Entity), Entity>,
    pub next_id: HashMap<u8, u8>,
}

impl ContactTracker {
    pub fn allocate_id(&mut self, team_id: u8) -> ContactId {
        let id = self.next_id.entry(team_id).or_insert(1);
        let contact_id = ContactId(*id);
        *id = id.wrapping_add(1).max(1);
        contact_id
    }
}

pub struct RadarClientPlugin;

impl Plugin for RadarClientPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                visuals::draw_radar_status_gizmos,
                visuals::draw_radar_signature_gizmos,
                visuals::draw_radar_track_gizmos,
                visuals::draw_tracked_missile_gizmos,
                visuals::draw_rwr_gizmos,
            )
                .run_if(in_state(GameState::Playing)),
        );
    }
}

pub struct RadarPlugin;

impl Plugin for RadarPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ContactTracker>();
        app.add_systems(
            Update,
            (
                contacts::update_radar_contacts,
                contacts::cleanup_stale_contacts,
                rwr::update_rwr_bearings,
            )
                .chain()
                .run_if(in_state(GameState::Playing)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aspect_broadside_is_max() {
        let factor = compute_aspect_factor(Vec2::X, Vec2::Y);
        assert!((factor - 1.0).abs() < 0.01);
    }

    #[test]
    fn aspect_nose_on_is_min() {
        let factor = compute_aspect_factor(Vec2::X, Vec2::NEG_X);
        assert!((factor - 0.25).abs() < 0.01);
    }

    #[test]
    fn aspect_tail_on_is_min() {
        let factor = compute_aspect_factor(Vec2::X, Vec2::X);
        assert!((factor - 0.25).abs() < 0.01);
    }

    #[test]
    fn aspect_factor_range() {
        for angle_deg in (0..360).step_by(10) {
            let angle = (angle_deg as f32).to_radians();
            let target_facing = Vec2::new(angle.cos(), angle.sin());
            let factor = compute_aspect_factor(Vec2::X, target_facing);
            assert!(factor >= 0.24, "factor {factor} below min at {angle_deg}°");
            assert!(factor <= 1.01, "factor {factor} above max at {angle_deg}°");
        }
    }

    #[test]
    fn snr_at_zero_distance_is_high() {
        let snr = compute_snr(800.0, 1.0, 1.0, 1.0);
        assert!(snr > TRACK_THRESHOLD);
    }

    #[test]
    fn snr_decreases_with_distance() {
        let near = compute_snr(800.0, 200.0, 1.0, 1.0);
        let far = compute_snr(800.0, 600.0, 1.0, 1.0);
        assert!(near > far);
    }

    #[test]
    fn snr_increases_with_rcs() {
        let small = compute_snr(800.0, 400.0, 0.25, 1.0);
        let large = compute_snr(800.0, 400.0, 1.0, 1.0);
        assert!(large > small);
    }

    #[test]
    fn snr_increases_with_aspect() {
        let nose = compute_snr(800.0, 400.0, 1.0, 0.25);
        let broadside = compute_snr(800.0, 400.0, 1.0, 1.0);
        assert!(broadside > nose);
    }

    #[test]
    fn battleship_broadside_tracked_at_800() {
        let snr = compute_snr(800.0, 800.0, 1.0, 1.0);
        assert!(
            snr >= TRACK_THRESHOLD,
            "BB broadside at max range should be tracked, got {snr}"
        );
    }

    #[test]
    fn scout_nose_on_not_tracked_at_800() {
        let snr = compute_snr(800.0, 800.0, 0.25, 0.25);
        assert!(
            snr < TRACK_THRESHOLD,
            "Scout nose-on at max range should not be tracked, got {snr}"
        );
    }

    #[test]
    fn scout_nose_on_signature_at_500() {
        let snr = compute_snr(800.0, 500.0, 0.25, 0.25);
        assert!(
            snr >= SIGNATURE_THRESHOLD,
            "Scout nose-on at 500m should be signature, got {snr}"
        );
    }

    #[test]
    fn nav_radar_shorter_range() {
        let search = compute_snr(800.0, 400.0, 0.5, 1.0);
        let nav = compute_snr(500.0, 400.0, 0.5, 1.0);
        assert!(search > nav);
    }

    #[test]
    fn battleship_detected_at_600m_broadside() {
        let aspect = compute_aspect_factor(Vec2::X, Vec2::Y);
        let snr = compute_snr(800.0, 600.0, 1.0, aspect);
        assert!(
            snr >= TRACK_THRESHOLD,
            "BB broadside at 600m should be tracked, got {snr}"
        );
    }

    #[test]
    fn scout_not_detected_at_700m_nose_on() {
        let aspect = compute_aspect_factor(Vec2::X, Vec2::NEG_X);
        let snr = compute_snr(800.0, 700.0, 0.25, aspect);
        assert!(
            snr < TRACK_THRESHOLD,
            "Scout nose-on at 700m should not be tracked, got {snr}"
        );
    }

    #[test]
    fn nav_radar_tracks_destroyer_at_300m() {
        let aspect = compute_aspect_factor(Vec2::X, Vec2::Y);
        let snr = compute_snr(500.0, 300.0, 0.5, aspect);
        assert!(
            snr >= TRACK_THRESHOLD,
            "DD broadside at 300m with nav radar should be tracked, got {snr}"
        );
    }

    #[test]
    fn missile_detected_by_radar_at_close_range() {
        let snr = compute_snr(800.0, 200.0, MISSILE_RCS, 1.0);
        assert!(
            snr >= SIGNATURE_THRESHOLD,
            "Missile at 200m should be at least signature, got {snr}"
        );
    }

    #[test]
    fn rwr_detects_radar_within_range() {
        assert!(rwr::is_in_rwr_range(
            Vec2::ZERO,
            800.0,
            Vec2::new(600.0, 0.0)
        ));
    }

    #[test]
    fn rwr_no_detection_outside_range() {
        assert!(!rwr::is_in_rwr_range(
            Vec2::ZERO,
            800.0,
            Vec2::new(900.0, 0.0)
        ));
    }

    #[test]
    fn rwr_exact_boundary() {
        assert!(rwr::is_in_rwr_range(
            Vec2::ZERO,
            800.0,
            Vec2::new(800.0, 0.0)
        ));
    }
}
