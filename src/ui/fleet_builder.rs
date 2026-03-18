use bevy::prelude::*;
use bevy_replicon::prelude::*;

use crate::fleet::{
    fleet_cost, hull_cost, ship_spec_cost, weapon_cost, AutoFleet, ShipSpec, FLEET_BUDGET,
    validate_fleet,
};
use crate::net::client::CurrentLobbyState;
use crate::net::commands::{CancelSubmission, FleetSubmission, LobbyState};
use crate::ship::ShipClass;
use crate::weapon::{MountSize, WeaponType};

// ── Constants ───────────────────────────────────────────────────────────

const BG_DARK: Color = Color::srgba(0.08, 0.08, 0.12, 0.95);
const BG_PANEL: Color = Color::srgb(0.12, 0.12, 0.18);
const BG_ENTRY: Color = Color::srgb(0.16, 0.16, 0.24);
const BG_ENTRY_SELECTED: Color = Color::srgb(0.25, 0.25, 0.45);
const BG_BUTTON: Color = Color::srgb(0.2, 0.2, 0.35);
const BG_SUBMIT: Color = Color::srgb(0.15, 0.55, 0.2);
const BG_CANCEL: Color = Color::srgb(0.7, 0.4, 0.1);
const BG_DISABLED: Color = Color::srgb(0.3, 0.3, 0.3);
const BG_POPUP: Color = Color::srgba(0.0, 0.0, 0.0, 0.8);
const BG_POPUP_INNER: Color = Color::srgb(0.15, 0.15, 0.22);
const TEXT_WHITE: Color = Color::WHITE;
const TEXT_RED: Color = Color::srgb(1.0, 0.3, 0.3);
const TEXT_GREEN: Color = Color::srgb(0.3, 1.0, 0.3);
const TEXT_YELLOW: Color = Color::srgb(1.0, 1.0, 0.4);
const TEXT_GRAY: Color = Color::srgb(0.6, 0.6, 0.6);

const ALL_SHIP_CLASSES: [ShipClass; 3] = [
    ShipClass::Battleship,
    ShipClass::Destroyer,
    ShipClass::Scout,
];

const ALL_WEAPONS: [WeaponType; 9] = [
    WeaponType::Railgun,
    WeaponType::HeavyVLS,
    WeaponType::HeavyCannon,
    WeaponType::SearchRadar,
    WeaponType::LaserPD,
    WeaponType::LightVLS,
    WeaponType::Cannon,
    WeaponType::NavRadar,
    WeaponType::CWIS,
];

// ── Resources ───────────────────────────────────────────────────────────

#[derive(Resource, Debug, Default)]
pub struct FleetBuilderState {
    pub ships: Vec<ShipSpec>,
    pub selected_ship: Option<usize>,
    pub submitted: bool,
    pub popup: Option<PopupKind>,
}

/// Controls how the fleet builder submit button behaves.
#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub enum FleetBuilderMode {
    /// Connected to server, submit triggers network event.
    Online,
    /// In lobby, submit just validates and stores locally.
    Lobby,
}

impl Default for FleetBuilderMode {
    fn default() -> Self {
        Self::Online
    }
}

#[derive(Debug, Clone)]
pub enum PopupKind {
    AddShip,
    ChangeWeapon { ship_index: usize, slot_index: usize },
}

// ── Marker Components ───────────────────────────────────────────────────

#[derive(Component)]
pub struct FleetUiRoot;

#[derive(Component)]
pub struct BudgetText;

#[derive(Component)]
pub struct FleetListPanel;

#[derive(Component)]
pub struct ShipDetailPanel;

#[derive(Component)]
pub struct SubmitButton;

#[derive(Component)]
pub struct StatusText;

#[derive(Component)]
pub struct AddShipButton;

#[derive(Component)]
pub struct ShipEntry(pub usize);

#[derive(Component)]
pub struct RemoveShipButton(pub usize);

#[derive(Component)]
pub struct CloneShipButton(pub usize);

#[derive(Component)]
pub struct ShipPickerOption(pub ShipClass);

#[derive(Component)]
pub struct PopupOverlay;

#[derive(Component)]
pub struct ChangeWeaponButton {
    pub ship_index: usize,
    pub slot_index: usize,
}

#[derive(Component)]
pub struct RemoveWeaponButton {
    pub ship_index: usize,
    pub slot_index: usize,
}

#[derive(Component)]
pub struct WeaponPickerOption(pub Option<WeaponType>);

#[derive(Component)]
pub struct PopupCloseButton;

#[derive(Component)]
pub struct SaveFleetButton;

#[derive(Component)]
pub struct LoadFleetButton;

// ── Spawn / Despawn ─────────────────────────────────────────────────────

pub fn spawn_fleet_ui(
    mut commands: Commands,
    auto_fleet: Option<Res<AutoFleet>>,
    mut state: ResMut<FleetBuilderState>,
) {
    // If entering from lobby (AutoFleet exists), populate the fleet builder with those ships
    if let Some(ref fleet) = auto_fleet {
        state.ships = fleet.0.clone();
        state.submitted = true;
    }
    commands
        .spawn((
            FleetUiRoot,
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
            spawn_fleet_builder_content(root);
        });
}

/// Spawn the fleet builder panels as children of the given parent entity.
/// Used by both FleetComposition (standalone) and GameLobby (embedded).
pub fn spawn_fleet_builder_content(parent: &mut ChildSpawnerCommands<'_>) {
    // ── Header bar ──
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Px(60.0),
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            padding: UiRect::horizontal(Val::Px(20.0)),
            ..default()
        })
        .with_children(|header| {
            header.spawn((
                Text::new("FLEET COMPOSITION"),
                TextFont {
                    font_size: 28.0,
                    ..default()
                },
                TextColor(TEXT_WHITE),
            ));
            header.spawn((
                BudgetText,
                Text::new(format!("Budget: 0 / {}", FLEET_BUDGET)),
                TextFont {
                    font_size: 22.0,
                    ..default()
                },
                TextColor(TEXT_WHITE),
            ));
        });

    // ── Main area (two panels side by side) ──
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_grow: 1.0,
            flex_direction: FlexDirection::Row,
            padding: UiRect::all(Val::Px(10.0)),
            column_gap: Val::Px(10.0),
            ..default()
        })
        .with_children(|main_area| {
            // Fleet list panel (left)
            main_area.spawn((
                FleetListPanel,
                Node {
                    width: Val::Percent(35.0),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(10.0)),
                    row_gap: Val::Px(6.0),
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
                BackgroundColor(BG_PANEL),
            ));

            // Ship detail panel (right)
            main_area.spawn((
                ShipDetailPanel,
                Node {
                    width: Val::Percent(65.0),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(10.0)),
                    row_gap: Val::Px(6.0),
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
                BackgroundColor(BG_PANEL),
            ));
        });

    // ── Bottom bar ──
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Px(60.0),
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            padding: UiRect::horizontal(Val::Px(20.0)),
            ..default()
        })
        .with_children(|bottom| {
            // Submit button
            bottom
                .spawn((
                    SubmitButton,
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(24.0), Val::Px(10.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(BG_DISABLED),
                ))
                .with_child((
                    Text::new("Submit Fleet"),
                    TextFont {
                        font_size: 20.0,
                        ..default()
                    },
                    TextColor(TEXT_WHITE),
                ));

            // Save/Load buttons
            bottom.spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(8.0),
                ..default()
            }).with_children(|btns| {
                btns.spawn((
                    SaveFleetButton,
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(16.0), Val::Px(10.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(BG_BUTTON),
                )).with_child((
                    Text::new("Save"),
                    TextFont { font_size: 16.0, ..default() },
                    TextColor(TEXT_WHITE),
                ));
                btns.spawn((
                    LoadFleetButton,
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(16.0), Val::Px(10.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(BG_BUTTON),
                )).with_child((
                    Text::new("Load"),
                    TextFont { font_size: 16.0, ..default() },
                    TextColor(TEXT_WHITE),
                ));
            });

            // Status text
            bottom.spawn((
                StatusText,
                Text::new("Composing fleet..."),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(TEXT_GRAY),
            ));
        });
}

pub fn despawn_fleet_ui(
    mut commands: Commands,
    roots: Query<Entity, With<FleetUiRoot>>,
    mut state: ResMut<FleetBuilderState>,
) {
    for entity in &roots {
        commands.entity(entity).despawn();
    }
    *state = FleetBuilderState::default();
}

// ── Helpers ──────────────────────────────────────────────────────────────

/// Despawn all children of a parent entity.
fn clear_children(commands: &mut Commands, parent: Entity, children_query: &Query<&Children>) {
    if let Ok(children) = children_query.get(parent) {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }
}

// ── Rebuild fleet list ──────────────────────────────────────────────────

pub fn rebuild_fleet_list(
    mut commands: Commands,
    state: Res<FleetBuilderState>,
    panel_query: Query<Entity, With<FleetListPanel>>,
    children_query: Query<&Children>,
) {
    if !state.is_changed() {
        return;
    }

    let Ok(panel_entity) = panel_query.single() else {
        return;
    };

    clear_children(&mut commands, panel_entity, &children_query);

    commands.entity(panel_entity).with_children(|panel| {
        // Title
        panel.spawn((
            Text::new("YOUR FLEET"),
            TextFont {
                font_size: 20.0,
                ..default()
            },
            TextColor(TEXT_WHITE),
        ));

        // Ship entries
        for (i, spec) in state.ships.iter().enumerate() {
            let is_selected = state.selected_ship == Some(i);
            let bg = if is_selected {
                BG_ENTRY_SELECTED
            } else {
                BG_ENTRY
            };
            let cost = ship_spec_cost(spec);

            panel
                .spawn((
                    ShipEntry(i),
                    Button,
                    Node {
                        width: Val::Percent(100.0),
                        padding: UiRect::all(Val::Px(8.0)),
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(bg),
                ))
                .with_children(|entry| {
                    entry.spawn((
                        Text::new(format!("{}. {:?} ({}pts)", i + 1, spec.class, cost)),
                        TextFont {
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(TEXT_WHITE),
                    ));

                    // Button group (clone + remove)
                    entry
                        .spawn(Node {
                            column_gap: Val::Px(4.0),
                            ..default()
                        })
                        .with_children(|btns| {
                            // Clone button
                            btns.spawn((
                                CloneShipButton(i),
                                Button,
                                Node {
                                    padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.2, 0.3, 0.5)),
                            ))
                            .with_child((
                                Text::new("C"),
                                TextFont {
                                    font_size: 14.0,
                                    ..default()
                                },
                                TextColor(TEXT_WHITE),
                            ));

                            // Remove button (small X)
                            btns.spawn((
                                RemoveShipButton(i),
                                Button,
                                Node {
                                    padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.6, 0.15, 0.15)),
                            ))
                            .with_child((
                                Text::new("X"),
                                TextFont {
                                    font_size: 14.0,
                                    ..default()
                                },
                                TextColor(TEXT_WHITE),
                            ));
                        });
                });
        }

        // Add ship button
        if !state.submitted {
            panel
                .spawn((
                    AddShipButton,
                    Button,
                    Node {
                        width: Val::Percent(100.0),
                        padding: UiRect::all(Val::Px(10.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        margin: UiRect::top(Val::Px(8.0)),
                        ..default()
                    },
                    BackgroundColor(BG_BUTTON),
                ))
                .with_child((
                    Text::new("+ Add Ship"),
                    TextFont {
                        font_size: 18.0,
                        ..default()
                    },
                    TextColor(TEXT_GREEN),
                ));
        }
    });
}

// ── Rebuild ship detail ─────────────────────────────────────────────────

pub fn rebuild_ship_detail(
    mut commands: Commands,
    state: Res<FleetBuilderState>,
    panel_query: Query<Entity, With<ShipDetailPanel>>,
    children_query: Query<&Children>,
) {
    if !state.is_changed() {
        return;
    }

    let Ok(panel_entity) = panel_query.single() else {
        return;
    };

    clear_children(&mut commands, panel_entity, &children_query);

    let Some(idx) = state.selected_ship else {
        commands.entity(panel_entity).with_children(|panel| {
            panel.spawn((
                Text::new("Select a ship to view details"),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(TEXT_GRAY),
            ));
        });
        return;
    };

    let Some(spec) = state.ships.get(idx) else {
        return;
    };

    let layout = spec.class.mount_layout();

    commands.entity(panel_entity).with_children(|panel| {
        // Ship header
        panel.spawn((
            Text::new(format!(
                "{:?} - {} pts (hull {})",
                spec.class,
                ship_spec_cost(spec),
                hull_cost(spec.class)
            )),
            TextFont {
                font_size: 22.0,
                ..default()
            },
            TextColor(TEXT_WHITE),
        ));

        // Mount slots
        panel.spawn((
            Text::new("WEAPON MOUNTS"),
            TextFont {
                font_size: 18.0,
                ..default()
            },
            TextColor(TEXT_YELLOW),
            Node {
                margin: UiRect::top(Val::Px(12.0)),
                ..default()
            },
        ));

        for (slot_idx, (mount_size, _pos)) in layout.iter().enumerate() {
            let weapon_opt = spec.loadout.get(slot_idx).copied().flatten();

            panel
                .spawn(Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::all(Val::Px(8.0)),
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|row| {
                    // Slot info
                    let weapon_text = match weapon_opt {
                        Some(w) => format!("{:?}", w),
                        None => "Empty".to_string(),
                    };
                    let cost_text = match weapon_opt {
                        Some(w) => format!(" ({}pts)", weapon_cost(w)),
                        None => String::new(),
                    };

                    row.spawn((
                        Text::new(format!(
                            "Slot {} [{:?}]: {}{}",
                            slot_idx + 1,
                            mount_size,
                            weapon_text,
                            cost_text,
                        )),
                        TextFont {
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(TEXT_WHITE),
                    ));

                    if !state.submitted {
                        // Buttons container
                        row.spawn(Node {
                            column_gap: Val::Px(4.0),
                            ..default()
                        })
                        .with_children(|btns| {
                            // Change button
                            btns.spawn((
                                ChangeWeaponButton {
                                    ship_index: idx,
                                    slot_index: slot_idx,
                                },
                                Button,
                                Node {
                                    padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                                    ..default()
                                },
                                BackgroundColor(BG_BUTTON),
                            ))
                            .with_child((
                                Text::new("Change"),
                                TextFont {
                                    font_size: 14.0,
                                    ..default()
                                },
                                TextColor(TEXT_WHITE),
                            ));

                            // Remove weapon button (only if weapon equipped)
                            if weapon_opt.is_some() {
                                btns.spawn((
                                    RemoveWeaponButton {
                                        ship_index: idx,
                                        slot_index: slot_idx,
                                    },
                                    Button,
                                    Node {
                                        padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgb(0.6, 0.15, 0.15)),
                                ))
                                .with_child((
                                    Text::new("Remove"),
                                    TextFont {
                                        font_size: 14.0,
                                        ..default()
                                    },
                                    TextColor(TEXT_WHITE),
                                ));
                            }
                        });
                    }
                });
        }

        // Remove ship button at the bottom
        if !state.submitted {
            panel
                .spawn((
                    RemoveShipButton(idx),
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(16.0), Val::Px(8.0)),
                        margin: UiRect::top(Val::Px(16.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.6, 0.15, 0.15)),
                ))
                .with_child((
                    Text::new("Remove Ship"),
                    TextFont {
                        font_size: 16.0,
                        ..default()
                    },
                    TextColor(TEXT_WHITE),
                ));
        }
    });
}

// ── Popup system ────────────────────────────────────────────────────────

pub fn spawn_popup(
    mut commands: Commands,
    state: Res<FleetBuilderState>,
    existing_popups: Query<Entity, With<PopupOverlay>>,
) {
    if !state.is_changed() {
        return;
    }

    // Always despawn existing popups first
    for entity in &existing_popups {
        commands.entity(entity).despawn();
    }

    let Some(ref popup_kind) = state.popup else {
        return;
    };

    commands
        .spawn((
            PopupOverlay,
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
            // Inner popup box
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
                    match popup_kind {
                        PopupKind::AddShip => {
                            inner.spawn((
                                Text::new("SELECT SHIP CLASS"),
                                TextFont {
                                    font_size: 22.0,
                                    ..default()
                                },
                                TextColor(TEXT_WHITE),
                            ));

                            for class in ALL_SHIP_CLASSES {
                                let cost = hull_cost(class);
                                let slots = class.mount_layout().len();

                                inner
                                    .spawn((
                                        ShipPickerOption(class),
                                        Button,
                                        Node {
                                            width: Val::Percent(100.0),
                                            padding: UiRect::all(Val::Px(10.0)),
                                            flex_direction: FlexDirection::Column,
                                            ..default()
                                        },
                                        BackgroundColor(BG_BUTTON),
                                    ))
                                    .with_children(|btn| {
                                        btn.spawn((
                                            Text::new(format!("{:?} - {} pts", class, cost)),
                                            TextFont {
                                                font_size: 18.0,
                                                ..default()
                                            },
                                            TextColor(TEXT_WHITE),
                                        ));
                                        btn.spawn((
                                            Text::new(format!("{} mount slots", slots)),
                                            TextFont {
                                                font_size: 14.0,
                                                ..default()
                                            },
                                            TextColor(TEXT_GRAY),
                                        ));
                                    });
                            }
                        }
                        PopupKind::ChangeWeapon {
                            ship_index,
                            slot_index,
                        } => {
                            let slot_size = state
                                .ships
                                .get(*ship_index)
                                .and_then(|spec| {
                                    spec.class
                                        .mount_layout()
                                        .get(*slot_index)
                                        .map(|(size, _)| *size)
                                })
                                .unwrap_or(MountSize::Small);

                            inner.spawn((
                                Text::new(format!("SELECT WEAPON [{:?} slot]", slot_size)),
                                TextFont {
                                    font_size: 22.0,
                                    ..default()
                                },
                                TextColor(TEXT_WHITE),
                            ));

                            // "Empty" option
                            inner
                                .spawn((
                                    WeaponPickerOption(None),
                                    Button,
                                    Node {
                                        width: Val::Percent(100.0),
                                        padding: UiRect::all(Val::Px(10.0)),
                                        ..default()
                                    },
                                    BackgroundColor(BG_BUTTON),
                                ))
                                .with_child((
                                    Text::new("Empty (0 pts)"),
                                    TextFont {
                                        font_size: 18.0,
                                        ..default()
                                    },
                                    TextColor(TEXT_GRAY),
                                ));

                            // Compatible weapons
                            for weapon in ALL_WEAPONS {
                                if !slot_size.fits(weapon.mount_size()) {
                                    continue;
                                }
                                let cost = weapon_cost(weapon);

                                inner
                                    .spawn((
                                        WeaponPickerOption(Some(weapon)),
                                        Button,
                                        Node {
                                            width: Val::Percent(100.0),
                                            padding: UiRect::all(Val::Px(10.0)),
                                            flex_direction: FlexDirection::Column,
                                            ..default()
                                        },
                                        BackgroundColor(BG_BUTTON),
                                    ))
                                    .with_children(|btn| {
                                        btn.spawn((
                                            Text::new(format!("{:?} - {} pts", weapon, cost)),
                                            TextFont {
                                                font_size: 18.0,
                                                ..default()
                                            },
                                            TextColor(TEXT_WHITE),
                                        ));
                                        btn.spawn((
                                            Text::new(format!(
                                                "[{:?}]",
                                                weapon.mount_size()
                                            )),
                                            TextFont {
                                                font_size: 14.0,
                                                ..default()
                                            },
                                            TextColor(TEXT_GRAY),
                                        ));
                                    });
                            }
                        }
                    }

                    // Close button at bottom
                    inner
                        .spawn((
                            PopupCloseButton,
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

// ── Click Handlers ──────────────────────────────────────────────────────

pub fn handle_add_ship_button(
    query: Query<&Interaction, (Changed<Interaction>, With<AddShipButton>)>,
    mut state: ResMut<FleetBuilderState>,
) {
    for interaction in &query {
        if *interaction == Interaction::Pressed {
            if !state.submitted {
                state.popup = Some(PopupKind::AddShip);
            }
        }
    }
}

pub fn handle_ship_entry_click(
    query: Query<(&Interaction, &ShipEntry), (Changed<Interaction>, With<Button>)>,
    mut state: ResMut<FleetBuilderState>,
) {
    for (interaction, entry) in &query {
        if *interaction == Interaction::Pressed {
            state.selected_ship = Some(entry.0);
        }
    }
}

pub fn handle_remove_ship_button(
    query: Query<
        (&Interaction, &RemoveShipButton),
        (Changed<Interaction>, With<Button>, Without<WeaponPickerOption>),
    >,
    mut state: ResMut<FleetBuilderState>,
) {
    for (interaction, btn) in &query {
        if *interaction == Interaction::Pressed && !state.submitted {
            let idx = btn.0;
            if idx < state.ships.len() {
                state.ships.remove(idx);
                // Fix selected_ship index
                match state.selected_ship {
                    Some(sel) if sel == idx => {
                        state.selected_ship = if state.ships.is_empty() {
                            None
                        } else {
                            Some(sel.min(state.ships.len() - 1))
                        };
                    }
                    Some(sel) if sel > idx => {
                        state.selected_ship = Some(sel - 1);
                    }
                    _ => {}
                }
            }
        }
    }
}

pub fn handle_clone_ship_button(
    query: Query<(&Interaction, &CloneShipButton), (Changed<Interaction>, With<Button>)>,
    mut state: ResMut<FleetBuilderState>,
) {
    for (interaction, btn) in &query {
        if *interaction == Interaction::Pressed && !state.submitted {
            let idx = btn.0;
            if let Some(spec) = state.ships.get(idx).cloned() {
                state.ships.push(spec);
                state.selected_ship = Some(state.ships.len() - 1);
            }
        }
    }
}

pub fn handle_ship_picker_option(
    query: Query<(&Interaction, &ShipPickerOption), (Changed<Interaction>, With<Button>)>,
    mut state: ResMut<FleetBuilderState>,
) {
    for (interaction, option) in &query {
        if *interaction == Interaction::Pressed {
            let class = option.0;
            let slot_count = class.mount_layout().len();
            let new_ship = ShipSpec {
                class,
                loadout: vec![None; slot_count],
            };
            state.ships.push(new_ship);
            state.selected_ship = Some(state.ships.len() - 1);
            state.popup = None;
        }
    }
}

pub fn handle_change_weapon_button(
    query: Query<(&Interaction, &ChangeWeaponButton), (Changed<Interaction>, With<Button>)>,
    mut state: ResMut<FleetBuilderState>,
) {
    for (interaction, btn) in &query {
        if *interaction == Interaction::Pressed && !state.submitted {
            state.popup = Some(PopupKind::ChangeWeapon {
                ship_index: btn.ship_index,
                slot_index: btn.slot_index,
            });
        }
    }
}

pub fn handle_remove_weapon_button(
    query: Query<
        (&Interaction, &RemoveWeaponButton),
        (Changed<Interaction>, With<Button>, Without<WeaponPickerOption>),
    >,
    mut state: ResMut<FleetBuilderState>,
) {
    for (interaction, btn) in &query {
        if *interaction == Interaction::Pressed && !state.submitted {
            if let Some(spec) = state.ships.get_mut(btn.ship_index) {
                if let Some(slot) = spec.loadout.get_mut(btn.slot_index) {
                    *slot = None;
                }
            }
        }
    }
}

pub fn handle_weapon_picker_option(
    query: Query<
        (&Interaction, &WeaponPickerOption),
        (Changed<Interaction>, With<Button>),
    >,
    mut state: ResMut<FleetBuilderState>,
) {
    for (interaction, option) in &query {
        if *interaction == Interaction::Pressed {
            // Assign weapon (or None for "Empty") from the current popup context
            if let Some(PopupKind::ChangeWeapon {
                ship_index,
                slot_index,
            }) = &state.popup
            {
                let si = *ship_index;
                let sli = *slot_index;
                if let Some(spec) = state.ships.get_mut(si) {
                    if let Some(slot) = spec.loadout.get_mut(sli) {
                        *slot = option.0;
                    }
                }
            }
            state.popup = None;
        }
    }
}

pub fn handle_popup_close(
    query: Query<&Interaction, (Changed<Interaction>, With<PopupCloseButton>)>,
    mut state: ResMut<FleetBuilderState>,
) {
    for interaction in &query {
        if *interaction == Interaction::Pressed {
            state.popup = None;
        }
    }
}

pub fn handle_submit_button(
    query: Query<&Interaction, (Changed<Interaction>, With<SubmitButton>)>,
    mut state: ResMut<FleetBuilderState>,
    mut commands: Commands,
    mode: Option<Res<FleetBuilderMode>>,
) {
    let mode = mode.map(|m| m.clone()).unwrap_or(FleetBuilderMode::Online);

    for interaction in &query {
        if *interaction == Interaction::Pressed {
            if state.submitted {
                // Cancel submission
                if mode == FleetBuilderMode::Online {
                    commands.client_trigger(CancelSubmission);
                }
                state.submitted = false;
            } else {
                // Validate and submit
                let validation = validate_fleet(&state.ships);
                if validation.is_ok() {
                    if mode == FleetBuilderMode::Online {
                        commands.client_trigger(FleetSubmission {
                            ships: state.ships.clone(),
                        });
                    }
                    state.submitted = true;
                }
            }
        }
    }
}

pub fn handle_save_fleet(
    query: Query<&Interaction, (Changed<Interaction>, With<SaveFleetButton>)>,
    state: Res<FleetBuilderState>,
) {
    for interaction in &query {
        if *interaction == Interaction::Pressed && !state.ships.is_empty() {
            let dir = std::path::Path::new("fleets");
            if std::fs::create_dir_all(dir).is_err() {
                warn!("Failed to create fleets directory");
                return;
            }
            let filename = format!("fleets/fleet_{}.ron", chrono_timestamp());
            match ron::ser::to_string_pretty(&state.ships, ron::ser::PrettyConfig::default()) {
                Ok(data) => {
                    if let Err(e) = std::fs::write(&filename, &data) {
                        warn!("Failed to save fleet: {e}");
                    } else {
                        info!("Fleet saved to {filename}");
                    }
                }
                Err(e) => warn!("Failed to serialize fleet: {e}"),
            }
        }
    }
}

pub fn handle_load_fleet(
    query: Query<&Interaction, (Changed<Interaction>, With<LoadFleetButton>)>,
    mut state: ResMut<FleetBuilderState>,
) {
    for interaction in &query {
        if *interaction == Interaction::Pressed {
            // Load the most recent fleet file
            let dir = std::path::Path::new("fleets");
            if !dir.exists() {
                warn!("No fleets directory found");
                return;
            }
            let mut files: Vec<_> = std::fs::read_dir(dir)
                .into_iter()
                .flatten()
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().is_some_and(|ext| ext == "ron"))
                .collect();
            files.sort_by_key(|e| std::cmp::Reverse(e.metadata().ok().and_then(|m| m.modified().ok())));

            if let Some(entry) = files.first() {
                match std::fs::read_to_string(entry.path()) {
                    Ok(data) => match ron::from_str::<Vec<ShipSpec>>(&data) {
                        Ok(ships) => {
                            info!("Fleet loaded from {:?} ({} ships)", entry.path(), ships.len());
                            state.ships = ships;
                            state.selected_ship = None;
                            state.submitted = false;
                        }
                        Err(e) => warn!("Failed to parse fleet file: {e}"),
                    },
                    Err(e) => warn!("Failed to read fleet file: {e}"),
                }
            } else {
                warn!("No fleet files found in fleets/");
            }
        }
    }
}

/// Simple timestamp for fleet filenames.
fn chrono_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{secs}")
}

// ── Update displays ─────────────────────────────────────────────────────

pub fn update_budget_text(
    state: Res<FleetBuilderState>,
    mut query: Query<(&mut Text, &mut TextColor), With<BudgetText>>,
) {
    if !state.is_changed() {
        return;
    }

    let spent = fleet_cost(&state.ships);
    let color = if spent > FLEET_BUDGET {
        TEXT_RED
    } else {
        TEXT_WHITE
    };

    for (mut text, mut text_color) in &mut query {
        *text = Text::new(format!("Budget: {} / {}", spent, FLEET_BUDGET));
        text_color.0 = color;
    }
}

pub fn update_status_text(
    lobby_state: Res<CurrentLobbyState>,
    mut query: Query<(&mut Text, &mut TextColor), With<StatusText>>,
    mut state: ResMut<FleetBuilderState>,
) {
    if !lobby_state.is_changed() {
        return;
    }

    // If the server rejected the submission, reset submitted so the player can edit
    if let Some(LobbyState::Rejected(_)) = &lobby_state.0 {
        state.submitted = false;
    }

    let (msg, color) = match &lobby_state.0 {
        None => ("Composing fleet...".to_string(), TEXT_GRAY),
        Some(LobbyState::Composing) => ("Composing fleet...".to_string(), TEXT_GRAY),
        Some(LobbyState::WaitingForOpponent) => {
            ("Fleet submitted. Waiting for opponent...".to_string(), TEXT_YELLOW)
        }
        Some(LobbyState::OpponentSubmitted) => {
            ("Opponent has submitted. Compose your fleet!".to_string(), TEXT_YELLOW)
        }
        Some(LobbyState::OpponentComposing) => {
            ("Opponent is still composing their fleet.".to_string(), TEXT_YELLOW)
        }
        Some(LobbyState::Countdown(secs)) => {
            (format!("Starting in {:.0}s...", secs), TEXT_GREEN)
        }
        Some(LobbyState::Rejected(reason)) => {
            (format!("Rejected: {}", reason), TEXT_RED)
        }
    };

    for (mut text, mut text_color) in &mut query {
        *text = Text::new(msg.clone());
        text_color.0 = color;
    }
}

pub fn update_submit_button(
    state: Res<FleetBuilderState>,
    mut query: Query<&mut BackgroundColor, With<SubmitButton>>,
    children_query: Query<&Children, With<SubmitButton>>,
    mut child_text_query: Query<&mut Text, Without<SubmitButton>>,
) {
    if !state.is_changed() {
        return;
    }

    let is_valid = validate_fleet(&state.ships).is_ok();

    let (bg, label) = if state.submitted {
        (BG_CANCEL, "Cancel Submission")
    } else if is_valid {
        (BG_SUBMIT, "Submit Fleet")
    } else {
        (BG_DISABLED, "Submit Fleet")
    };

    for mut bg_color in &mut query {
        bg_color.0 = bg;
    }

    // Update child text entities
    for children in &children_query {
        for child in children.iter() {
            if let Ok(mut text) = child_text_query.get_mut(child) {
                *text = Text::new(label);
            }
        }
    }
}
