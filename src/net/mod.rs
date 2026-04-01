pub mod client;
pub mod commands;
pub mod materializer;
pub mod replication;
pub mod server;

use bevy::prelude::*;

pub use client::ClientNetPlugin;
pub use replication::SharedReplicationPlugin;
pub use server::ServerNetPlugin;

use crate::game::Team;

/// Protocol ID for our game — used to reject connections from other applications.
/// Bump the low bytes on every breaking change to replicated components or network events.
pub const PROTOCOL_ID: u64 = 0x4E45_4255_4C41_0001; // "NEBULA" + version

/// Identifies which team this client controls.
/// `None` on server (or before assignment), `Some(team)` on client after assignment.
#[derive(Resource, Debug, Default, Clone)]
pub struct LocalTeam(pub Option<Team>);

/// Identifies which player entity this client owns.
/// Used to distinguish "my ships" from allied ships on the same team.
#[derive(Resource, Debug, Default, Clone)]
pub struct LocalPlayer(pub Option<Entity>);
