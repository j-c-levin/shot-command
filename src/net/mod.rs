pub mod commands;
pub mod server;

use bevy::prelude::*;

pub use server::ServerNetPlugin;

use crate::game::Team;

/// Identifies which team this client controls.
/// `None` on server (or before assignment), `Some(team)` on client after assignment.
#[derive(Resource, Debug, Default, Clone)]
pub struct LocalTeam(pub Option<Team>);
