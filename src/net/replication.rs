//! Shared replication registration plugin.
//!
//! Both server and client must register replicated components and events in
//! **exactly the same order** — bevy_replicon computes a protocol hash from
//! registration order and rejects connections on mismatch. This plugin is the
//! single source of truth for that ordering.

use bevy::prelude::*;
use bevy_replicon::prelude::*;

use crate::game::{EngineOffline, Health, Player, Team};
use crate::map::{Asteroid, AsteroidSize};
use crate::net::commands::{
    CancelMissilesCommand, CancelSubmission, ClearTargetCommand, FacingLockCommand,
    FacingUnlockCommand, FireMissileCommand, FleetSubmission, GameResult, GameStarted,
    JoinSquadCommand, LaunchCommand, LobbyStatus, MoveCommand, RadarToggleCommand,
    TargetByContactCommand, TargetCommand, TeamAssignment,
};
use crate::radar::{
    ContactId, ContactKind, ContactLevel, ContactSourceShip, ContactTeam, RadarActiveSecret,
    RadarContact,
};
use crate::radar::rwr::RwrBearings;
use crate::ship::{
    EngineHealth, FacingLocked, FacingTarget, Ship, ShipClass, ShipNumber, ShipSecrets,
    ShipSecretsOwner, SquadMember, SquadSpeedLimit, TargetDesignation, WaypointQueue,
};
use crate::weapon::{MissileQueue, Mounts};
use crate::weapon::missile::{
    Explosion, ExplosionTimer, Missile, MissileDamage, MissileFuel,
    MissileOwner, MissileTarget, MissileVelocity,
};
use crate::weapon::pd::{LaserBeam, LaserBeamTarget, LaserBeamTimer};
use crate::control_point::{ControlPoint, ControlPointRadius, ControlPointState, TeamScores};
use crate::weapon::projectile::{CwisRound, Projectile, ProjectileDamage, ProjectileOwner, ProjectileVelocity};

pub struct SharedReplicationPlugin;

impl Plugin for SharedReplicationPlugin {
    fn build(&self, app: &mut App) {
        // ── Replicated components ──────────────────────────────────────
        // Velocity is server-only, not replicated.
        // WaypointQueue, FacingTarget, FacingLocked are NOT replicated on Ship entities —
        // they arrive via ShipSecrets entities with per-team visibility.
        app.replicate::<Ship>()
            .replicate::<ShipClass>()
            .replicate::<Team>()
            .replicate::<Player>()
            .replicate::<EngineOffline>()
            .replicate::<Transform>()
            .replicate::<Health>()
            .replicate::<EngineHealth>()
            .replicate::<Mounts>()
            .replicate::<Asteroid>()
            .replicate::<AsteroidSize>()
            .replicate::<Projectile>()
            .replicate::<ProjectileVelocity>()
            .replicate::<ProjectileDamage>()
            .replicate::<ProjectileOwner>()
            .replicate::<CwisRound>();

        // Missile components
        app.replicate::<Missile>()
            .replicate::<MissileTarget>()
            .replicate::<MissileVelocity>()
            .replicate::<MissileDamage>()
            .replicate::<MissileOwner>()
            .replicate::<MissileFuel>()
            .replicate::<Explosion>()
            .replicate::<ExplosionTimer>()
            .replicate::<LaserBeam>()
            .replicate::<LaserBeamTarget>()
            .replicate::<LaserBeamTimer>();

        // ShipSecrets entity components (team-private state)
        app.replicate::<ShipSecrets>()
            .replicate::<ShipSecretsOwner>()
            .replicate::<WaypointQueue>()
            .replicate::<FacingTarget>()
            .replicate::<FacingLocked>()
            .replicate::<TargetDesignation>()
            .replicate::<MissileQueue>()
            .replicate::<ShipNumber>()
            .replicate::<SquadMember>()
            .replicate::<SquadSpeedLimit>();

        // Radar components (on ShipSecrets)
        app.replicate::<RadarActiveSecret>()
            .replicate::<RwrBearings>();

        // Radar contact entity components
        app.replicate::<RadarContact>()
            .replicate::<ContactLevel>()
            .replicate::<ContactTeam>()
            .replicate::<ContactId>()
            .replicate::<ContactSourceShip>()
            .replicate::<ContactKind>();

        // Control point components
        app.replicate::<ControlPoint>()
            .replicate::<ControlPointState>()
            .replicate::<ControlPointRadius>()
            .replicate::<TeamScores>();

        // ── Client→server triggers ─────────────────────────────────────
        app.add_mapped_client_event::<MoveCommand>(Channel::Ordered)
            .add_mapped_client_event::<FacingLockCommand>(Channel::Ordered)
            .add_mapped_client_event::<FacingUnlockCommand>(Channel::Ordered)
            .add_mapped_client_event::<TargetCommand>(Channel::Ordered)
            .add_mapped_client_event::<ClearTargetCommand>(Channel::Ordered)
            .add_mapped_client_event::<TargetByContactCommand>(Channel::Ordered)
            .add_mapped_client_event::<FireMissileCommand>(Channel::Ordered)
            .add_mapped_client_event::<CancelMissilesCommand>(Channel::Ordered)
            .add_mapped_client_event::<JoinSquadCommand>(Channel::Ordered)
            .add_mapped_client_event::<RadarToggleCommand>(Channel::Ordered)
            .add_mapped_client_event::<FleetSubmission>(Channel::Ordered)
            .add_mapped_client_event::<CancelSubmission>(Channel::Ordered)
            .add_mapped_client_event::<LaunchCommand>(Channel::Ordered);

        // ── Server→client triggers ─────────────────────────────────────
        app.add_mapped_server_event::<TeamAssignment>(Channel::Ordered);
        app.add_server_event::<GameResult>(Channel::Ordered);
        app.add_server_event::<LobbyStatus>(Channel::Ordered);
        app.add_server_event::<GameStarted>(Channel::Ordered);
    }
}
