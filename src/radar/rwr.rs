//! RWR (Radar Warning Receiver) bearing detection.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// RWR bearing lines for a ship — directions toward enemy radar sources.
/// Lives on ShipSecrets entities (team-private).
#[derive(Component, Serialize, Deserialize, Clone, Debug, Default)]
pub struct RwrBearings(pub Vec<Vec2>);

/// Returns true if target_pos is within radar_range of radar_pos.
pub fn is_in_rwr_range(radar_pos: Vec2, radar_range: f32, target_pos: Vec2) -> bool {
    radar_pos.distance(target_pos) <= radar_range
}
