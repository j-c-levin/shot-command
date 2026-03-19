pub mod api;
pub mod game_lobby;
pub mod main_menu;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::game::GameState;

// ── Plugin ──────────────────────────────────────────────────────────────

pub struct LobbyPlugin;

impl Plugin for LobbyPlugin {
    fn build(&self, app: &mut App) {
        // MainMenu systems
        app.add_systems(OnEnter(GameState::MainMenu), main_menu::spawn_main_menu)
            .add_systems(OnExit(GameState::MainMenu), main_menu::despawn_main_menu)
            .add_systems(
                Update,
                (
                    main_menu::poll_game_list,
                    main_menu::poll_maps,
                    main_menu::poll_pending_create,
                    main_menu::rebuild_game_list,
                    main_menu::update_error_text,
                    main_menu::handle_join_button,
                    main_menu::handle_create_button,
                    main_menu::handle_direct_connect,
                )
                    .run_if(in_state(GameState::MainMenu)),
            )
            .add_systems(
                Update,
                (
                    main_menu::handle_refresh,
                    main_menu::spawn_create_dialog,
                    main_menu::handle_map_picker_option,
                    main_menu::handle_create_confirm_close,
                    main_menu::update_button_states,
                )
                    .run_if(in_state(GameState::MainMenu)),
            );

        // GameLobby systems
        app.add_systems(
            OnEnter(GameState::GameLobby),
            game_lobby::spawn_game_lobby,
        )
        .add_systems(
            OnExit(GameState::GameLobby),
            game_lobby::despawn_game_lobby,
        )
        .add_systems(
            Update,
            (
                game_lobby::poll_game_detail,
                game_lobby::poll_pending_launch,
                game_lobby::poll_pending_delete,
                game_lobby::rebuild_player_list,
                game_lobby::handle_launch_button,
                game_lobby::handle_leave_button,
                game_lobby::sync_ready_state,
            )
                .run_if(in_state(GameState::GameLobby)),
        );
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
    #[serde(default)]
    pub ready: bool,
}
