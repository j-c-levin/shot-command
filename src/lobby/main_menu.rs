use std::sync::mpsc::Receiver;

use bevy::prelude::*;

use super::api;
use super::{CurrentGameId, GameInfo, LobbyConfig, PlayerName};
use crate::game::GameState;
use crate::net::client::ClientConnectAddress;

// ── Constants ───────────────────────────────────────────────────────────

const BG_DARK: Color = Color::srgba(0.08, 0.08, 0.12, 0.95);
const BG_PANEL: Color = Color::srgb(0.12, 0.12, 0.18);
const BG_ENTRY: Color = Color::srgb(0.16, 0.16, 0.24);
const BG_BUTTON: Color = Color::srgb(0.2, 0.2, 0.35);
const BG_SUBMIT: Color = Color::srgb(0.15, 0.55, 0.2);
const BG_POPUP: Color = Color::srgba(0.0, 0.0, 0.0, 0.8);
const BG_POPUP_INNER: Color = Color::srgb(0.15, 0.15, 0.22);
const TEXT_WHITE: Color = Color::WHITE;
const TEXT_GRAY: Color = Color::srgb(0.6, 0.6, 0.6);
const TEXT_YELLOW: Color = Color::srgb(1.0, 1.0, 0.4);
const TEXT_RED: Color = Color::srgb(1.0, 0.3, 0.3);
const BG_DISABLED: Color = Color::srgb(0.3, 0.3, 0.3);

const POLL_INTERVAL_SECS: f32 = 3.0;

// ── Resources ───────────────────────────────────────────────────────────

/// Main menu display state (Send + Sync, can be a normal Resource).
#[derive(Resource)]
pub struct MainMenuState {
    pub games: Vec<GameInfo>,
    pub maps: Vec<String>,
    pub poll_timer: Timer,
    pub error: Option<String>,
    pub create_dialog_open: bool,
    pub selected_map: Option<String>,
    /// Tracks whether the game list has changed and UI needs rebuild.
    pub games_changed: bool,
}

/// Async HTTP receivers for main menu (not Send+Sync, uses NonSend).
pub struct MainMenuAsync {
    pub pending_list: Option<Receiver<Result<Vec<GameInfo>, String>>>,
    pub pending_maps: Option<Receiver<Result<Vec<String>, String>>>,
    pub pending_create: Option<Receiver<Result<String, String>>>,
    pub pending_join: Option<(String, Receiver<Result<(), String>>)>,
}

// ── Marker Components ───────────────────────────────────────────────────

#[derive(Component)]
pub struct MainMenuRoot;

#[derive(Component)]
pub struct GameListPanel;

#[derive(Component)]
pub struct GameRow(pub String);

#[derive(Component)]
pub struct JoinButton(pub String);

#[derive(Component)]
pub struct CreateGameButton;

#[derive(Component)]
pub struct DirectConnectButton;

#[derive(Component)]
pub struct RefreshButton;

#[derive(Component)]
pub struct CreateDialogOverlay;

#[derive(Component)]
pub struct MapPickerOption(pub Option<String>);

#[derive(Component)]
pub struct CreateConfirmButton;

#[derive(Component)]
pub struct ErrorText;

// ── Spawn / Despawn ─────────────────────────────────────────────────────

pub fn spawn_main_menu(mut commands: Commands, lobby_config: Res<LobbyConfig>) {
    // Fire initial API calls
    let pending_list = api::list_games(&lobby_config.api_base_url);
    let pending_maps = api::fetch_maps(&lobby_config.api_base_url);

    commands.insert_resource(MainMenuState {
        games: Vec::new(),
        maps: Vec::new(),
        poll_timer: Timer::from_seconds(POLL_INTERVAL_SECS, TimerMode::Repeating),
        error: None,
        create_dialog_open: false,
        selected_map: None,
        games_changed: true,
    });

    commands.queue(move |world: &mut World| {
        world.insert_non_send_resource(MainMenuAsync {
            pending_list: Some(pending_list),
            pending_maps: Some(pending_maps),
            pending_create: None,
            pending_join: None,
        });
    });

    commands
        .spawn((
            MainMenuRoot,
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
            // ── Title bar ──
            root.spawn(Node {
                width: Val::Percent(100.0),
                height: Val::Px(80.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            })
            .with_child((
                Text::new("NEBULOUS SHOT COMMAND"),
                TextFont {
                    font_size: 36.0,
                    ..default()
                },
                TextColor(TEXT_WHITE),
            ));

            // ── Button bar (Create Game, Direct Connect) ──
            root.spawn(Node {
                width: Val::Percent(100.0),
                height: Val::Px(60.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                column_gap: Val::Px(20.0),
                padding: UiRect::horizontal(Val::Px(20.0)),
                ..default()
            })
            .with_children(|bar| {
                // Create Game button
                bar.spawn((
                    CreateGameButton,
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(24.0), Val::Px(10.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(BG_SUBMIT),
                ))
                .with_child((
                    Text::new("Create Game"),
                    TextFont {
                        font_size: 20.0,
                        ..default()
                    },
                    TextColor(TEXT_WHITE),
                ));

                // Direct Connect button
                bar.spawn((
                    DirectConnectButton,
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(24.0), Val::Px(10.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(BG_BUTTON),
                ))
                .with_child((
                    Text::new("Direct Connect"),
                    TextFont {
                        font_size: 20.0,
                        ..default()
                    },
                    TextColor(TEXT_WHITE),
                ));
            });

            // ── "OPEN GAMES" header + Refresh ──
            root.spawn(Node {
                width: Val::Percent(100.0),
                height: Val::Px(40.0),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                padding: UiRect::horizontal(Val::Px(20.0)),
                ..default()
            })
            .with_children(|header| {
                header.spawn((
                    Text::new("OPEN GAMES"),
                    TextFont {
                        font_size: 22.0,
                        ..default()
                    },
                    TextColor(TEXT_YELLOW),
                ));

                header
                    .spawn((
                        RefreshButton,
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(16.0), Val::Px(6.0)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(BG_BUTTON),
                    ))
                    .with_child((
                        Text::new("Refresh"),
                        TextFont {
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(TEXT_WHITE),
                    ));
            });

            // ── Error text ──
            root.spawn((
                ErrorText,
                Text::new(""),
                TextFont {
                    font_size: 16.0,
                    ..default()
                },
                TextColor(TEXT_RED),
                Node {
                    padding: UiRect::horizontal(Val::Px(20.0)),
                    ..default()
                },
            ));

            // ── Game list panel ──
            root.spawn((
                GameListPanel,
                Node {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(20.0)),
                    row_gap: Val::Px(6.0),
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
                BackgroundColor(BG_PANEL),
            ));
        });
}

pub fn despawn_main_menu(mut commands: Commands, roots: Query<Entity, With<MainMenuRoot>>) {
    for entity in &roots {
        commands.entity(entity).despawn();
    }
    commands.remove_resource::<MainMenuState>();
    commands.queue(|world: &mut World| {
        world.remove_non_send_resource::<MainMenuAsync>();
    });
}

// ── Polling Systems ─────────────────────────────────────────────────────

pub fn poll_game_list(
    time: Res<Time>,
    lobby_config: Res<LobbyConfig>,
    mut state: ResMut<MainMenuState>,
    mut async_state: NonSendMut<MainMenuAsync>,
) {
    // Check pending receiver
    if let Some(ref rx) = async_state.pending_list {
        match rx.try_recv() {
            Ok(Ok(games)) => {
                state.games = games;
                state.games_changed = true;
                async_state.pending_list = None;
                state.error = None;
            }
            Ok(Err(e)) => {
                warn!("Failed to list games: {}", e);
                state.error = Some(format!("Failed to list games: {e}"));
                async_state.pending_list = None;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                async_state.pending_list = None;
            }
        }
    }

    // Poll timer for periodic refresh
    state.poll_timer.tick(time.delta());
    if state.poll_timer.just_finished() && async_state.pending_list.is_none() {
        async_state.pending_list = Some(api::list_games(&lobby_config.api_base_url));
    }
}

pub fn poll_maps(
    mut state: ResMut<MainMenuState>,
    mut async_state: NonSendMut<MainMenuAsync>,
) {
    if let Some(ref rx) = async_state.pending_maps {
        match rx.try_recv() {
            Ok(Ok(maps)) => {
                state.maps = maps;
                async_state.pending_maps = None;
            }
            Ok(Err(e)) => {
                warn!("Failed to fetch maps: {}", e);
                async_state.pending_maps = None;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                async_state.pending_maps = None;
            }
        }
    }
}

pub fn poll_pending_create(
    mut state: ResMut<MainMenuState>,
    mut async_state: NonSendMut<MainMenuAsync>,
    mut commands: Commands,
    mut next_state: ResMut<NextState<GameState>>,
) {
    // Poll pending create
    if let Some(ref rx) = async_state.pending_create {
        match rx.try_recv() {
            Ok(Ok(game_id)) => {
                info!("Created game: {}", game_id);
                commands.insert_resource(CurrentGameId(game_id));
                async_state.pending_create = None;
                state.create_dialog_open = false;
                next_state.set(GameState::GameLobby);
            }
            Ok(Err(e)) => {
                warn!("Failed to create game: {}", e);
                state.error = Some(format!("Failed to create game: {e}"));
                state.create_dialog_open = false;
                async_state.pending_create = None;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                async_state.pending_create = None;
            }
        }
    }

    // Poll pending join
    let join_done = if let Some((ref game_id, ref rx)) = async_state.pending_join {
        match rx.try_recv() {
            Ok(Ok(())) => {
                info!("Joined game: {}", game_id);
                commands.insert_resource(CurrentGameId(game_id.clone()));
                Some(Ok(()))
            }
            Ok(Err(e)) => {
                warn!("Failed to join game: {}", e);
                Some(Err(format!("Failed to join game: {e}")))
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => None,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                Some(Err("join disconnected".into()))
            }
        }
    } else {
        None
    };

    if let Some(result) = join_done {
        async_state.pending_join = None;
        match result {
            Ok(()) => {
                next_state.set(GameState::GameLobby);
            }
            Err(e) => {
                state.error = Some(e);
            }
        }
    }
}

/// Gray out buttons and update text while async operations are in flight.
pub fn update_button_states(
    async_state: NonSend<MainMenuAsync>,
    mut join_buttons: Query<(&mut BackgroundColor, &Children), With<JoinButton>>,
    mut create_button: Query<(&mut BackgroundColor, &Children), (With<CreateGameButton>, Without<JoinButton>)>,
    mut text_query: Query<&mut Text>,
) {
    let joining = async_state.pending_join.is_some();
    let creating = async_state.pending_create.is_some();

    for (mut bg, children) in &mut join_buttons {
        bg.0 = if joining { BG_DISABLED } else { BG_SUBMIT };
        for child in children.iter() {
            if let Ok(mut text) = text_query.get_mut(child) {
                **text = if joining { "Joining...".to_string() } else { "Join".to_string() };
            }
        }
    }

    for (mut bg, children) in &mut create_button {
        bg.0 = if creating { BG_DISABLED } else { BG_SUBMIT };
        for child in children.iter() {
            if let Ok(mut text) = text_query.get_mut(child) {
                **text = if creating { "Creating...".to_string() } else { "Create Game".to_string() };
            }
        }
    }
}

// ── Rebuild Game List ───────────────────────────────────────────────────

/// Despawn all children of a parent entity.
fn clear_children(commands: &mut Commands, parent: Entity, children_query: &Query<&Children>) {
    if let Ok(children) = children_query.get(parent) {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }
}

pub fn rebuild_game_list(
    mut commands: Commands,
    mut state: ResMut<MainMenuState>,
    panel_query: Query<Entity, With<GameListPanel>>,
    children_query: Query<&Children>,
) {
    if !state.games_changed {
        return;
    }
    state.games_changed = false;

    let Ok(panel_entity) = panel_query.single() else {
        return;
    };

    clear_children(&mut commands, panel_entity, &children_query);

    commands.entity(panel_entity).with_children(|panel| {
        if state.games.is_empty() {
            panel.spawn((
                Text::new("No open games"),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(TEXT_GRAY),
            ));
            return;
        }

        for game in &state.games {
            let game_id = game.game_id.clone();

            panel
                .spawn((
                    GameRow(game_id.clone()),
                    Node {
                        width: Val::Percent(100.0),
                        padding: UiRect::all(Val::Px(10.0)),
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(BG_ENTRY),
                ))
                .with_children(|row| {
                    // Game info text
                    let map_text = game.map.as_deref().unwrap_or("random");
                    row.spawn((
                        Text::new(format!(
                            "{}  {}/2  {}",
                            game.creator, game.player_count, map_text
                        )),
                        TextFont {
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(TEXT_WHITE),
                    ));

                    // Join button
                    row.spawn((
                        JoinButton(game_id),
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(16.0), Val::Px(6.0)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(BG_SUBMIT),
                    ))
                    .with_child((
                        Text::new("Join"),
                        TextFont {
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(TEXT_WHITE),
                    ));
                });
        }
    });
}

pub fn update_error_text(
    state: Res<MainMenuState>,
    mut query: Query<(&mut Text, &mut TextColor), With<ErrorText>>,
) {
    if !state.is_changed() {
        return;
    }

    for (mut text, mut color) in &mut query {
        match &state.error {
            Some(e) => {
                *text = Text::new(e.clone());
                color.0 = TEXT_RED;
            }
            None => {
                *text = Text::new("");
            }
        }
    }
}

// ── Click Handlers ──────────────────────────────────────────────────────

pub fn handle_join_button(
    query: Query<(&Interaction, &JoinButton), (Changed<Interaction>, With<Button>)>,
    lobby_config: Res<LobbyConfig>,
    player_name: Res<PlayerName>,
    mut async_state: NonSendMut<MainMenuAsync>,
) {
    for (interaction, btn) in &query {
        if *interaction == Interaction::Pressed && async_state.pending_join.is_none() {
            let game_id = btn.0.clone();
            info!("Joining game: {}", game_id);
            let rx = api::join_game(&lobby_config.api_base_url, &game_id, &player_name.0);
            async_state.pending_join = Some((game_id, rx));
        }
    }
}

pub fn handle_create_button(
    query: Query<&Interaction, (Changed<Interaction>, With<CreateGameButton>)>,
    mut state: ResMut<MainMenuState>,
) {
    for interaction in &query {
        if *interaction == Interaction::Pressed {
            state.create_dialog_open = true;
        }
    }
}

pub fn handle_direct_connect(
    query: Query<&Interaction, (Changed<Interaction>, With<DirectConnectButton>)>,
    mut commands: Commands,
    mut next_state: ResMut<NextState<GameState>>,
) {
    for interaction in &query {
        if *interaction == Interaction::Pressed {
            // Connect to localhost default
            commands.insert_resource(ClientConnectAddress("127.0.0.1:5000".to_string()));
            next_state.set(GameState::Connecting);
        }
    }
}

pub fn handle_refresh(
    query: Query<&Interaction, (Changed<Interaction>, With<RefreshButton>)>,
    lobby_config: Res<LobbyConfig>,
    mut async_state: NonSendMut<MainMenuAsync>,
) {
    for interaction in &query {
        if *interaction == Interaction::Pressed {
            if async_state.pending_list.is_none() {
                async_state.pending_list = Some(api::list_games(&lobby_config.api_base_url));
            }
        }
    }
}

// ── Create Game Dialog ──────────────────────────────────────────────────

pub fn spawn_create_dialog(
    mut commands: Commands,
    state: Res<MainMenuState>,
    existing: Query<Entity, With<CreateDialogOverlay>>,
    async_state: NonSend<MainMenuAsync>,
) {
    if !state.is_changed() {
        return;
    }

    // Despawn existing dialog
    for entity in &existing {
        commands.entity(entity).despawn();
    }

    if !state.create_dialog_open {
        return;
    }

    commands
        .spawn((
            CreateDialogOverlay,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(BG_POPUP),
            GlobalZIndex(10),
        ))
        .with_children(|overlay| {
            overlay
                .spawn((
                    Node {
                        width: Val::Px(400.0),
                        max_height: Val::Percent(80.0),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(16.0)),
                        row_gap: Val::Px(8.0),
                        overflow: Overflow::scroll_y(),
                        ..default()
                    },
                    BackgroundColor(BG_POPUP_INNER),
                ))
                .with_children(|inner| {
                    inner.spawn((
                        Text::new("CREATE GAME"),
                        TextFont {
                            font_size: 22.0,
                            ..default()
                        },
                        TextColor(TEXT_WHITE),
                    ));

                    if async_state.pending_create.is_some() {
                        inner.spawn((
                            Text::new("Creating game..."),
                            TextFont { font_size: 18.0, ..default() },
                            TextColor(TEXT_YELLOW),
                        ));
                        return;
                    }

                    inner.spawn((
                        Text::new("Select Map:"),
                        TextFont {
                            font_size: 18.0,
                            ..default()
                        },
                        TextColor(TEXT_YELLOW),
                    ));

                    // "Random" option (no map)
                    inner
                        .spawn((
                            MapPickerOption(None),
                            Button,
                            Node {
                                width: Val::Percent(100.0),
                                padding: UiRect::axes(Val::Px(16.0), Val::Px(12.0)),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            BackgroundColor(BG_BUTTON),
                        ))
                        .with_child((
                            Text::new("Random (no map)"),
                            TextFont {
                                font_size: 18.0,
                                ..default()
                            },
                            TextColor(TEXT_GRAY),
                        ));

                    // Map options
                    for map_name in &state.maps {
                        inner
                            .spawn((
                                MapPickerOption(Some(map_name.clone())),
                                Button,
                                Node {
                                    width: Val::Percent(100.0),
                                    padding: UiRect::all(Val::Px(10.0)),
                                    ..default()
                                },
                                BackgroundColor(BG_BUTTON),
                            ))
                            .with_child((
                                Text::new(map_name.clone()),
                                TextFont {
                                    font_size: 18.0,
                                    ..default()
                                },
                                TextColor(TEXT_WHITE),
                            ));
                    }

                    // Cancel button
                    inner
                        .spawn((
                            CreateConfirmButton,
                            Button,
                            Node {
                                width: Val::Percent(100.0),
                                padding: UiRect::all(Val::Px(8.0)),
                                margin: UiRect::top(Val::Px(8.0)),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.4, 0.15, 0.15)),
                        ))
                        .with_child((
                            Text::new("Cancel"),
                            TextFont {
                                font_size: 16.0,
                                ..default()
                            },
                            TextColor(TEXT_WHITE),
                        ));
                });
        });
}

pub fn handle_map_picker_option(
    query: Query<(&Interaction, &MapPickerOption), (Changed<Interaction>, With<Button>)>,
    mut state: ResMut<MainMenuState>,
    lobby_config: Res<LobbyConfig>,
    player_name: Res<PlayerName>,
    mut async_state: NonSendMut<MainMenuAsync>,
) {
    for (interaction, option) in &query {
        if *interaction == Interaction::Pressed && async_state.pending_create.is_none() {
            state.selected_map = option.0.clone();

            // Create the game (keep dialog open to show feedback)
            info!("Creating game with map: {:?}", state.selected_map);
            let rx = api::create_game(
                &lobby_config.api_base_url,
                &player_name.0,
                state.selected_map.as_deref(),
                None,
                None,
            );
            async_state.pending_create = Some(rx);
        }
    }
}

pub fn handle_create_confirm_close(
    query: Query<&Interaction, (Changed<Interaction>, With<CreateConfirmButton>)>,
    mut state: ResMut<MainMenuState>,
) {
    for interaction in &query {
        if *interaction == Interaction::Pressed {
            state.create_dialog_open = false;
        }
    }
}
