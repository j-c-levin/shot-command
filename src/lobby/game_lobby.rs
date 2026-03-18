use std::sync::mpsc::Receiver;

use bevy::prelude::*;

use super::api;
use super::{CurrentGameId, GameDetail, LobbyConfig, PlayerName};
use crate::fleet::AutoFleet;
use crate::game::GameState;
use crate::net::client::ClientConnectAddress;
use crate::ui::fleet_builder::{
    spawn_fleet_builder_content, FleetBuilderMode, FleetBuilderState, FleetUiRoot,
};

// ── Constants ───────────────────────────────────────────────────────────

const BG_DARK: Color = Color::srgba(0.08, 0.08, 0.12, 0.95);
const BG_PANEL: Color = Color::srgb(0.12, 0.12, 0.18);
const BG_SUBMIT: Color = Color::srgb(0.15, 0.55, 0.2);
const BG_DISABLED: Color = Color::srgb(0.3, 0.3, 0.3);
const TEXT_WHITE: Color = Color::WHITE;
const TEXT_GRAY: Color = Color::srgb(0.6, 0.6, 0.6);
const TEXT_YELLOW: Color = Color::srgb(1.0, 1.0, 0.4);

const POLL_INTERVAL_SECS: f32 = 2.0;

// ── Resources ───────────────────────────────────────────────────────────

/// Display state for the game lobby (Send + Sync).
#[derive(Resource)]
pub struct GameLobbyState {
    pub detail: Option<GameDetail>,
    pub poll_timer: Timer,
    pub is_creator: bool,
    pub fleet_ready: bool,
    pub status_message: String,
    pub detail_changed: bool,
}

/// Async HTTP receivers for game lobby (not Send+Sync, uses NonSend).
pub struct GameLobbyAsync {
    pub pending_detail: Option<Receiver<Result<GameDetail, String>>>,
    pub pending_launch: Option<Receiver<Result<(), String>>>,
    pub pending_delete: Option<Receiver<Result<(), String>>>,
}

// ── Marker Components ───────────────────────────────────────────────────

#[derive(Component)]
pub struct GameLobbyRoot;

#[derive(Component)]
pub struct PlayerListPanel;

#[derive(Component)]
pub struct LobbyInfoPanel;

#[derive(Component)]
pub struct LaunchButton;

#[derive(Component)]
pub struct LeaveButton;

#[derive(Component)]
pub struct LobbyStatusText;

// ── Spawn / Despawn ─────────────────────────────────────────────────────

pub fn spawn_game_lobby(
    mut commands: Commands,
    game_id: Res<CurrentGameId>,
    _player_name: Res<PlayerName>,
    lobby_config: Res<LobbyConfig>,
) {
    let is_creator = true; // Will be confirmed when detail arrives

    // Fire initial get_game
    let pending_detail = api::get_game(&lobby_config.api_base_url, &game_id.0);

    commands.insert_resource(GameLobbyState {
        detail: None,
        poll_timer: Timer::from_seconds(POLL_INTERVAL_SECS, TimerMode::Repeating),
        is_creator,
        fleet_ready: false,
        status_message: "Loading game details...".to_string(),
        detail_changed: true,
    });

    commands.insert_resource(FleetBuilderMode::Lobby);

    // Reset fleet builder state for a fresh start
    commands.init_resource::<FleetBuilderState>();

    commands.queue(move |world: &mut World| {
        world.insert_non_send_resource(GameLobbyAsync {
            pending_detail: Some(pending_detail),
            pending_launch: None,
            pending_delete: None,
        });
    });

    commands
        .spawn((
            GameLobbyRoot,
            FleetUiRoot, // Tag so fleet builder systems can find their panels
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(BG_DARK),
            GlobalZIndex(5),
        ))
        .with_children(|root| {
            // ── Top bar: title + Leave button ──
            root.spawn(Node {
                width: Val::Percent(100.0),
                height: Val::Px(60.0),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                padding: UiRect::horizontal(Val::Px(20.0)),
                ..default()
            })
            .with_children(|bar| {
                bar.spawn((
                    Text::new("GAME LOBBY"),
                    TextFont {
                        font_size: 28.0,
                        ..default()
                    },
                    TextColor(TEXT_WHITE),
                ));

                bar.spawn((
                    LeaveButton,
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(20.0), Val::Px(8.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.6, 0.15, 0.15)),
                ))
                .with_child((
                    Text::new("Leave"),
                    TextFont {
                        font_size: 18.0,
                        ..default()
                    },
                    TextColor(TEXT_WHITE),
                ));
            });

            // ── Main area: sidebar + fleet builder ──
            root.spawn(Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                flex_direction: FlexDirection::Row,
                padding: UiRect::all(Val::Px(10.0)),
                column_gap: Val::Px(10.0),
                ..default()
            })
            .with_children(|main_area| {
                // ── Left sidebar (~25%): players, map, status, launch ──
                main_area
                    .spawn((
                        LobbyInfoPanel,
                        Node {
                            width: Val::Percent(25.0),
                            height: Val::Percent(100.0),
                            flex_direction: FlexDirection::Column,
                            padding: UiRect::all(Val::Px(10.0)),
                            row_gap: Val::Px(10.0),
                            ..default()
                        },
                        BackgroundColor(BG_PANEL),
                    ))
                    .with_children(|sidebar| {
                        sidebar.spawn((
                            Text::new("PLAYERS"),
                            TextFont {
                                font_size: 20.0,
                                ..default()
                            },
                            TextColor(TEXT_YELLOW),
                        ));

                        // Player list
                        sidebar.spawn((
                            PlayerListPanel,
                            Node {
                                width: Val::Percent(100.0),
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(4.0),
                                ..default()
                            },
                        ));

                        // Status text
                        sidebar.spawn((
                            LobbyStatusText,
                            Text::new("Loading..."),
                            TextFont {
                                font_size: 16.0,
                                ..default()
                            },
                            TextColor(TEXT_GRAY),
                        ));

                        // Launch button (creator only)
                        sidebar
                            .spawn((
                                LaunchButton,
                                Button,
                                Node {
                                    width: Val::Percent(100.0),
                                    padding: UiRect::all(Val::Px(10.0)),
                                    margin: UiRect::top(Val::Auto),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    ..default()
                                },
                                BackgroundColor(BG_DISABLED),
                            ))
                            .with_child((
                                Text::new("LAUNCH"),
                                TextFont {
                                    font_size: 20.0,
                                    ..default()
                                },
                                TextColor(TEXT_WHITE),
                            ));
                    });

                // ── Right area (~75%): fleet builder ──
                main_area
                    .spawn(Node {
                        width: Val::Percent(75.0),
                        height: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        ..default()
                    })
                    .with_children(|fleet_area| {
                        spawn_fleet_builder_content(fleet_area);
                    });
            });
        });
}

pub fn despawn_game_lobby(
    mut commands: Commands,
    roots: Query<Entity, With<GameLobbyRoot>>,
) {
    for entity in &roots {
        commands.entity(entity).despawn();
    }
    commands.remove_resource::<GameLobbyState>();
    commands.remove_resource::<FleetBuilderMode>();
    commands.queue(|world: &mut World| {
        world.remove_non_send_resource::<GameLobbyAsync>();
    });
}

// ── Polling Systems ─────────────────────────────────────────────────────

pub fn poll_game_detail(
    time: Res<Time>,
    lobby_config: Res<LobbyConfig>,
    game_id: Res<CurrentGameId>,
    mut state: ResMut<GameLobbyState>,
    mut async_state: NonSendMut<GameLobbyAsync>,
    player_name: Res<PlayerName>,
    mut commands: Commands,
    mut next_state: ResMut<NextState<GameState>>,
    fleet_state: Res<FleetBuilderState>,
) {
    // Check pending detail receiver
    if let Some(ref rx) = async_state.pending_detail {
        match rx.try_recv() {
            Ok(Ok(detail)) => {
                // Determine if we are the creator
                state.is_creator = detail.creator == player_name.0;

                // Check if server is ready
                if detail.status == "ready" {
                    if let Some(ref addr) = detail.server_address {
                        info!("Server ready at {}, transitioning to Connecting", addr);
                        commands.insert_resource(ClientConnectAddress(addr.clone()));

                        // Store current fleet as AutoFleet for auto-submit
                        if !fleet_state.ships.is_empty() {
                            commands.insert_resource(AutoFleet(fleet_state.ships.clone()));
                        }

                        next_state.set(GameState::Connecting);
                        async_state.pending_detail = None;
                        return;
                    }
                }

                // Update status message
                let player_count = detail.players.len();
                state.status_message = match detail.status.as_str() {
                    "waiting" if player_count < 2 => {
                        "Waiting for opponent...".to_string()
                    }
                    "waiting" => "Ready to launch!".to_string(),
                    "launching" => "Launching server...".to_string(),
                    other => format!("Status: {}", other),
                };

                state.detail = Some(detail);
                state.detail_changed = true;
                async_state.pending_detail = None;
            }
            Ok(Err(e)) => {
                warn!("Failed to get game detail: {}", e);
                state.status_message = format!("Error: {e}");
                async_state.pending_detail = None;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                async_state.pending_detail = None;
            }
        }
    }

    // Poll timer for periodic refresh
    state.poll_timer.tick(time.delta());
    if state.poll_timer.just_finished() && async_state.pending_detail.is_none() {
        async_state.pending_detail =
            Some(api::get_game(&lobby_config.api_base_url, &game_id.0));
    }
}

pub fn poll_pending_launch(
    mut state: ResMut<GameLobbyState>,
    mut async_state: NonSendMut<GameLobbyAsync>,
) {
    if let Some(ref rx) = async_state.pending_launch {
        match rx.try_recv() {
            Ok(Ok(())) => {
                info!("Launch request sent successfully");
                state.status_message = "Launching server...".to_string();
                async_state.pending_launch = None;
            }
            Ok(Err(e)) => {
                warn!("Failed to launch game: {}", e);
                state.status_message = format!("Launch failed: {e}");
                async_state.pending_launch = None;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                async_state.pending_launch = None;
            }
        }
    }
}

pub fn poll_pending_delete(
    mut async_state: NonSendMut<GameLobbyAsync>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if let Some(ref rx) = async_state.pending_delete {
        match rx.try_recv() {
            Ok(Ok(())) => {
                info!("Left game successfully");
                async_state.pending_delete = None;
                next_state.set(GameState::MainMenu);
            }
            Ok(Err(e)) => {
                warn!("Failed to delete game: {}", e);
                async_state.pending_delete = None;
                // Go back to menu anyway
                next_state.set(GameState::MainMenu);
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                async_state.pending_delete = None;
                next_state.set(GameState::MainMenu);
            }
        }
    }
}

// ── Rebuild Systems ─────────────────────────────────────────────────────

/// Despawn all children of a parent entity.
fn clear_children(commands: &mut Commands, parent: Entity, children_query: &Query<&Children>) {
    if let Ok(children) = children_query.get(parent) {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }
}

pub fn rebuild_player_list(
    mut commands: Commands,
    mut state: ResMut<GameLobbyState>,
    panel_query: Query<Entity, With<PlayerListPanel>>,
    children_query: Query<&Children>,
    mut status_query: Query<(&mut Text, &mut TextColor), With<LobbyStatusText>>,
    mut launch_bg_query: Query<&mut BackgroundColor, With<LaunchButton>>,
) {
    if !state.detail_changed {
        return;
    }
    state.detail_changed = false;

    let Ok(panel_entity) = panel_query.single() else {
        return;
    };

    clear_children(&mut commands, panel_entity, &children_query);

    commands.entity(panel_entity).with_children(|panel| {
        if let Some(ref detail) = state.detail {
            for player in &detail.players {
                panel.spawn((
                    Text::new(format!("{}  (Team {})", player.name, player.team)),
                    TextFont {
                        font_size: 16.0,
                        ..default()
                    },
                    TextColor(TEXT_WHITE),
                ));
            }

            // Empty slot
            if detail.players.len() < 2 {
                panel.spawn((
                    Text::new("???  (waiting)"),
                    TextFont {
                        font_size: 16.0,
                        ..default()
                    },
                    TextColor(TEXT_GRAY),
                ));
            }

            // Map info
            let map_text = detail
                .map
                .as_deref()
                .unwrap_or("random");
            panel.spawn((
                Text::new(format!("Map: {}", map_text)),
                TextFont {
                    font_size: 16.0,
                    ..default()
                },
                TextColor(TEXT_GRAY),
                Node {
                    margin: UiRect::top(Val::Px(10.0)),
                    ..default()
                },
            ));
        } else {
            panel.spawn((
                Text::new("Loading..."),
                TextFont {
                    font_size: 16.0,
                    ..default()
                },
                TextColor(TEXT_GRAY),
            ));
        }
    });

    // Update status text
    for (mut text, mut color) in &mut status_query {
        *text = Text::new(&state.status_message);
        color.0 = TEXT_YELLOW;
    }

    // Update launch button color
    let can_launch = state.is_creator
        && state
            .detail
            .as_ref()
            .map(|d| d.players.len() >= 2 && d.status == "waiting")
            .unwrap_or(false);

    for mut bg in &mut launch_bg_query {
        bg.0 = if can_launch { BG_SUBMIT } else { BG_DISABLED };
    }
}

// ── Click Handlers ──────────────────────────────────────────────────────

pub fn handle_launch_button(
    query: Query<&Interaction, (Changed<Interaction>, With<LaunchButton>)>,
    state: Res<GameLobbyState>,
    game_id: Res<CurrentGameId>,
    lobby_config: Res<LobbyConfig>,
    player_name: Res<PlayerName>,
    mut async_state: NonSendMut<GameLobbyAsync>,
) {
    for interaction in &query {
        if *interaction == Interaction::Pressed {
            // Only creator can launch, need 2 players
            let can_launch = state.is_creator
                && state
                    .detail
                    .as_ref()
                    .map(|d| d.players.len() >= 2 && d.status == "waiting")
                    .unwrap_or(false);

            if can_launch && async_state.pending_launch.is_none() {
                info!("Launching game: {}", game_id.0);
                async_state.pending_launch = Some(api::launch_game(
                    &lobby_config.api_base_url,
                    &game_id.0,
                    &player_name.0,
                ));
            }
        }
    }
}

pub fn handle_leave_button(
    query: Query<&Interaction, (Changed<Interaction>, With<LeaveButton>)>,
    game_id: Res<CurrentGameId>,
    lobby_config: Res<LobbyConfig>,
    player_name: Res<PlayerName>,
    mut async_state: NonSendMut<GameLobbyAsync>,
) {
    for interaction in &query {
        if *interaction == Interaction::Pressed {
            if async_state.pending_delete.is_none() {
                info!("Leaving game: {}", game_id.0);
                async_state.pending_delete = Some(api::delete_game(
                    &lobby_config.api_base_url,
                    &game_id.0,
                    &player_name.0,
                ));
            }
        }
    }
}
