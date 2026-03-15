pub mod client;
pub mod commands;
pub mod server;

use bevy::prelude::*;

pub use client::ClientNetPlugin;
pub use server::ServerNetPlugin;

use crate::game::Team;

/// Protocol ID for our game -- used to reject connections from other applications.
pub const PROTOCOL_ID: u64 = 0x4E45_4255_4C41_0001; // "NEBULA" + version

/// Identifies which team this client controls.
/// `None` on server (or before assignment), `Some(team)` on client after assignment.
#[derive(Resource, Debug, Default, Clone)]
pub struct LocalTeam(pub Option<Team>);
