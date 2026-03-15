use bevy::ecs::entity::MapEntities;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::game::Team;

/// Client → server: order a ship to move.
#[derive(Event, Debug, Clone, Serialize, Deserialize, MapEntities)]
pub struct MoveCommand {
    #[entities]
    pub ship: Entity,
    pub destination: Vec2,
    pub append: bool,
}

/// Client → server: lock a ship's facing to a direction.
#[derive(Event, Debug, Clone, Serialize, Deserialize, MapEntities)]
pub struct FacingLockCommand {
    #[entities]
    pub ship: Entity,
    pub direction: Vec2,
}

/// Client → server: unlock a ship's facing.
#[derive(Event, Debug, Clone, Serialize, Deserialize, MapEntities)]
pub struct FacingUnlockCommand {
    #[entities]
    pub ship: Entity,
}

/// Server → client: tells the client which team it controls.
#[derive(Event, Debug, Clone, Serialize, Deserialize)]
pub struct TeamAssignment {
    pub team: Team,
}
