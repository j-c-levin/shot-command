pub mod api;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

// ── Plugin ──────────────────────────────────────────────────────────────

pub struct LobbyPlugin;

impl Plugin for LobbyPlugin {
    fn build(&self, _app: &mut App) {
        // Systems will be registered as MainMenu and GameLobby UIs are added.
    }
}

// ── Resources ───────────────────────────────────────────────────────────

/// Base URL for the lobby API (Firebase Cloud Functions).
#[derive(Resource, Debug, Clone)]
pub struct LobbyConfig {
    pub api_base_url: String,
}

/// Player display name (from CLI --name flag).
#[derive(Resource, Debug, Clone)]
pub struct PlayerName(pub String);

/// Resource set when entering a game lobby (from join or create).
#[derive(Resource, Debug, Clone)]
pub struct CurrentGameId(pub String);

// ── Data Types ──────────────────────────────────────────────────────────

/// Summary of a game in the lobby list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameInfo {
    pub game_id: String,
    pub creator: String,
    pub player_count: usize,
    pub map: Option<String>,
    pub status: String,
}

/// Full detail of a specific game.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameDetail {
    pub game_id: String,
    pub creator: String,
    pub status: String,
    pub players: Vec<PlayerInfo>,
    pub server_address: Option<String>,
    pub map: Option<String>,
}

/// A player in a game lobby.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerInfo {
    pub name: String,
    pub team: u8,
}
