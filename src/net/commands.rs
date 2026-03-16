use bevy::ecs::entity::MapEntities;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::fleet::ShipSpec;
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

/// Client → server: designate a target for a ship.
#[derive(Event, Debug, Clone, Serialize, Deserialize, MapEntities)]
pub struct TargetCommand {
    #[entities]
    pub ship: Entity,
    #[entities]
    pub target: Entity,
}

/// Client → server: clear a ship's target designation.
#[derive(Event, Debug, Clone, Serialize, Deserialize, MapEntities)]
pub struct ClearTargetCommand {
    #[entities]
    pub ship: Entity,
}

/// Client → server: queue a missile launch from a ship.
#[derive(Event, Debug, Clone, Serialize, Deserialize)]
pub struct FireMissileCommand {
    pub ship: Entity,
    pub target_point: Vec2,
    pub target_entity: Option<Entity>,
}

impl MapEntities for FireMissileCommand {
    fn map_entities<M: bevy::ecs::entity::EntityMapper>(&mut self, mapper: &mut M) {
        self.ship = mapper.get_mapped(self.ship);
        if let Some(ref mut e) = self.target_entity {
            *e = mapper.get_mapped(*e);
        }
    }
}

/// Client → server: cancel all queued (not yet launched) missiles.
#[derive(Event, Debug, Clone, Serialize, Deserialize, MapEntities)]
pub struct CancelMissilesCommand {
    #[entities]
    pub ship: Entity,
}

/// Client → server: join a ship to a squad led by another ship.
#[derive(Event, Debug, Clone, Serialize, Deserialize, MapEntities)]
pub struct JoinSquadCommand {
    #[entities]
    pub ship: Entity,
    #[entities]
    pub leader: Entity,
}

/// Server → client: tells the client which team it controls.
#[derive(Event, Debug, Clone, Serialize, Deserialize)]
pub struct TeamAssignment {
    pub team: Team,
}

/// Server → client: announces the game result (which team won).
#[derive(Event, Debug, Clone, Serialize, Deserialize)]
pub struct GameResult {
    pub winning_team: Team,
}

/// Client → server: submit a fleet composition for validation.
#[derive(Event, Debug, Clone, Serialize, Deserialize)]
pub struct FleetSubmission {
    pub ships: Vec<ShipSpec>,
}

impl MapEntities for FleetSubmission {
    fn map_entities<M: bevy::ecs::entity::EntityMapper>(&mut self, _mapper: &mut M) {}
}

/// Client → server: cancel a previously submitted fleet (return to composing).
#[derive(Event, Debug, Clone, Serialize, Deserialize)]
pub struct CancelSubmission;

impl MapEntities for CancelSubmission {
    fn map_entities<M: bevy::ecs::entity::EntityMapper>(&mut self, _mapper: &mut M) {}
}

/// Describes the current state of the lobby from the server's perspective.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LobbyState {
    /// Player is still composing their fleet.
    Composing,
    /// Player has submitted; waiting for opponent to submit.
    WaitingForOpponent,
    /// Opponent has submitted; this player is still composing.
    OpponentComposing,
    /// Both submitted; countdown to game start (seconds remaining).
    Countdown(f32),
    /// Server rejected the fleet submission (with reason).
    Rejected(String),
}

/// Server → client: lobby status update.
#[derive(Event, Debug, Clone, Serialize, Deserialize)]
pub struct LobbyStatus {
    pub state: LobbyState,
}

/// Server → client: game is starting (countdown complete).
#[derive(Event, Debug, Clone, Serialize, Deserialize)]
pub struct GameStarted;
