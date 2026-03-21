use std::sync::mpsc::Receiver;

use bevy::prelude::*;

use super::api;
use super::{CurrentGameId, GameDetail, LobbyConfig, PlayerName};
use crate::fleet::AutoFleet;
use crate::game::{GameConfig, GameState, Team};
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
const TEXT_GREEN: Color = Color::srgb(0.3, 1.0, 0.3);
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
    pub last_submitted: bool,
}

/// Async HTTP receivers for game lobby (not Send+Sync, uses NonSend).
pub struct GameLobbyAsync {
    pub pending_detail: Option<Receiver<Result<GameDetail, String>>>,
    pub pending_launch: Option<Receiver<Result<(), String>>>,
    pub pending_delete: Option<Receiver<Result<(), String>>>,
    pub pending_switch: Option<Receiver<Result<(), String>>>,
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

/// Button to switch the local player to a specific team.
#[derive(Component)]
pub struct TeamSwitchButton(pub u8);

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
        last_submitted: false,
    });

    commands.insert_resource(FleetBuilderMode::Lobby);

    // Reset fleet builder state for a fresh start (insert, not init, to trigger is_changed)
    commands.insert_resource(FleetBuilderState::default());

    commands.queue(move |world: &mut World| {
        world.insert_non_send_resource(GameLobbyAsync {
            pending_detail: Some(pending_detail),
            pending_launch: None,
            pending_delete: None,
            pending_switch: None,
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

                        // Pre-insert GameConfig from lobby detail so UI has it during connection
                        let team_count = detail.team_count.unwrap_or(2);
                        let players_per_team = detail.players_per_team.unwrap_or(1);
                        commands.insert_resource(GameConfig {
                            team_count,
                            players_per_team,
                        });

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
                let tc = detail.team_count.unwrap_or(2) as usize;
                let all_teams_have_ready = (0..tc).all(|t| {
                    detail
                        .players
                        .iter()
                        .any(|p| p.team == t as u8 && p.ready)
                });
                let all_teams_have_player = (0..tc).all(|t| {
                    detail.players.iter().any(|p| p.team == t as u8)
                });
                state.status_message = match detail.status.as_str() {
                    "waiting" if !all_teams_have_player => {
                        "Waiting for more players...".to_string()
                    }
                    "waiting" if !all_teams_have_ready => {
                        "Waiting for all teams to ready up...".to_string()
                    }
                    "waiting" => "All ready - launch when ready!".to_string(),
                    "launching" => "Launching server...".to_string(),
                    other => format!("Status: {}", other),
                };

                state.detail = Some(detail);
                state.detail_changed = true;
                async_state.pending_detail = None;
            }
            Ok(Err(e)) => {
                warn!("Failed to get game detail: {} — returning to menu", e);
                async_state.pending_detail = None;
                // Game likely deleted by creator — kick back to MainMenu
                next_state.set(GameState::MainMenu);
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
    player_name: Res<PlayerName>,
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

    // Compute team layout info for launch readiness check
    let mut teams_ready = true;
    let mut missing_teams: Vec<u8> = Vec::new();

    commands.entity(panel_entity).with_children(|panel| {
        if let Some(ref detail) = state.detail {
            let team_count = detail.team_count.unwrap_or(2);
            let ppt = detail.players_per_team.unwrap_or(1);

            // Group players by team
            for team_idx in 0..team_count {
                let team_color = Team(team_idx).color();
                let team_players: Vec<_> = detail
                    .players
                    .iter()
                    .filter(|p| p.team == team_idx)
                    .collect();

                // Team header
                panel.spawn((
                    Text::new(format!("Team {}", team_idx + 1)),
                    TextFont { font_size: 17.0, ..default() },
                    TextColor(team_color),
                    Node {
                        margin: UiRect::top(if team_idx > 0 {
                            Val::Px(8.0)
                        } else {
                            Val::Px(0.0)
                        }),
                        ..default()
                    },
                ));

                // Player entries
                for p in &team_players {
                    let ready_icon = if p.ready { "READY" } else { "..." };
                    let text_color = if p.ready { TEXT_GREEN } else { TEXT_YELLOW };
                    panel.spawn((
                        Text::new(format!("  {}  {}", p.name, ready_icon)),
                        TextFont { font_size: 15.0, ..default() },
                        TextColor(text_color),
                    ));
                }

                // Empty slots
                let empty_slots = ppt as usize - team_players.len().min(ppt as usize);
                for _ in 0..empty_slots {
                    panel.spawn((
                        Text::new("  ???  (waiting)"),
                        TextFont { font_size: 15.0, ..default() },
                        TextColor(TEXT_GRAY),
                    ));
                }

                // Track whether this team has at least one ready player
                let has_ready = team_players.iter().any(|p| p.ready);
                if !has_ready {
                    teams_ready = false;
                    missing_teams.push(team_idx);
                }

                // Switch button: show if this team has open slots and
                // the local player is not already on this team
                let my_team = detail
                    .players
                    .iter()
                    .find(|p| p.name == player_name.0)
                    .map(|p| p.team);
                let is_my_team = my_team == Some(team_idx);
                let has_space = team_players.len() < ppt as usize;

                if has_space && !is_my_team {
                    panel
                        .spawn((
                            TeamSwitchButton(team_idx),
                            Button,
                            Node {
                                padding: UiRect::axes(Val::Px(12.0), Val::Px(4.0)),
                                margin: UiRect::left(Val::Px(16.0)),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            BackgroundColor(BG_PANEL),
                        ))
                        .with_child((
                            Text::new(format!("Switch to Team {}", team_idx + 1)),
                            TextFont { font_size: 13.0, ..default() },
                            TextColor(team_color),
                        ));
                }
            }

            // Map info
            let map_text = detail.map.as_deref().unwrap_or("random");
            panel.spawn((
                Text::new(format!("Map: {}", map_text)),
                TextFont { font_size: 16.0, ..default() },
                TextColor(TEXT_GRAY),
                Node {
                    margin: UiRect::top(Val::Px(10.0)),
                    ..default()
                },
            ));
        } else {
            panel.spawn((
                Text::new("Loading..."),
                TextFont { font_size: 16.0, ..default() },
                TextColor(TEXT_GRAY),
            ));
        }
    });

    // Update status text
    for (mut text, mut color) in &mut status_query {
        if !missing_teams.is_empty() {
            let missing_str: Vec<String> =
                missing_teams.iter().map(|t| format!("{}", t + 1)).collect();
            state.status_message =
                format!("Waiting: team(s) {} need players", missing_str.join(", "));
        }
        *text = Text::new(&state.status_message);
        color.0 = TEXT_YELLOW;
    }

    // Update launch button color — require every team to have at least one ready player.
    // `teams_ready` is false if any team lacks a ready player.
    let can_launch = state.is_creator
        && teams_ready
        && state
            .detail
            .as_ref()
            .map(|d| d.status == "waiting")
            .unwrap_or(false);

    let launching = state.status_message.contains("Launching");
    for mut bg in &mut launch_bg_query {
        bg.0 = if launching {
            BG_DISABLED
        } else if can_launch {
            BG_SUBMIT
        } else {
            BG_DISABLED
        };
    }
}

// ── Click Handlers ──────────────────────────────────────────────────────

pub fn handle_launch_button(
    query: Query<&Interaction, (Changed<Interaction>, With<LaunchButton>)>,
    mut state: ResMut<GameLobbyState>,
    game_id: Res<CurrentGameId>,
    lobby_config: Res<LobbyConfig>,
    player_name: Res<PlayerName>,
    mut async_state: NonSendMut<GameLobbyAsync>,
) {
    for interaction in &query {
        if *interaction == Interaction::Pressed {
            // Only creator can launch; every team needs at least 1 ready player
            let can_launch = state.is_creator
                && state
                    .detail
                    .as_ref()
                    .map(|d| {
                        let tc = d.team_count.unwrap_or(2) as usize;
                        d.status == "waiting"
                            && (0..tc).all(|t| {
                                d.players
                                    .iter()
                                    .any(|p| p.team == t as u8 && p.ready)
                            })
                    })
                    .unwrap_or(false);

            if can_launch && async_state.pending_launch.is_none() {
                info!("Launching game: {}", game_id.0);
                state.status_message = "Launching server...".to_string();
                state.detail_changed = true;
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

pub fn handle_team_switch_button(
    query: Query<(&Interaction, &TeamSwitchButton), (Changed<Interaction>, With<Button>)>,
    lobby_config: Res<LobbyConfig>,
    game_id: Res<CurrentGameId>,
    player_name: Res<PlayerName>,
    mut async_state: NonSendMut<GameLobbyAsync>,
) {
    for (interaction, btn) in &query {
        if *interaction == Interaction::Pressed && async_state.pending_switch.is_none() {
            info!("Switching to team {}", btn.0);
            async_state.pending_switch = Some(api::switch_team(
                &lobby_config.api_base_url,
                &game_id.0,
                &player_name.0,
                btn.0,
            ));
        }
    }
}

pub fn poll_pending_switch(
    mut state: ResMut<GameLobbyState>,
    mut async_state: NonSendMut<GameLobbyAsync>,
) {
    if let Some(ref rx) = async_state.pending_switch {
        match rx.try_recv() {
            Ok(Ok(())) => {
                info!("Team switch successful");
                state.detail_changed = true;
                async_state.pending_switch = None;
            }
            Ok(Err(e)) => {
                warn!("Failed to switch team: {}", e);
                state.status_message = format!("Switch failed: {e}");
                state.detail_changed = true;
                async_state.pending_switch = None;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                async_state.pending_switch = None;
            }
        }
    }
}

/// Sync FleetBuilderState.submitted → ready_up API call.
/// When player submits fleet in lobby mode, notify the server they're ready.
pub fn sync_ready_state(
    fleet_state: Res<FleetBuilderState>,
    lobby_config: Res<LobbyConfig>,
    game_id: Res<CurrentGameId>,
    player_name: Res<PlayerName>,
    mut state: ResMut<GameLobbyState>,
) {
    if !fleet_state.is_changed() {
        return;
    }
    if fleet_state.submitted != state.last_submitted {
        state.last_submitted = fleet_state.submitted;
        let _ = api::ready_up(
            &lobby_config.api_base_url,
            &game_id.0,
            &player_name.0,
            fleet_state.submitted,
        );
        // Optimistic local update — immediately reflect our own ready state
        if let Some(ref mut detail) = state.detail {
            for p in &mut detail.players {
                if p.name == player_name.0 {
                    p.ready = fleet_state.submitted;
                }
            }
            state.detail_changed = true;
        }
    }
}
