use bevy::picking::prelude::Pickable;
use bevy::prelude::*;

use crate::game::{Destroyed, GameState, Health, Team};
use crate::net::LocalTeam;
use crate::ship::{EngineHealth, Selected, Ship, ShipClass, ShipNumber, ShipSecrets, ShipSecretsOwner};
use crate::weapon::{Mounts, WeaponCategory, WeaponType};

pub struct FleetStatusPlugin;

impl Plugin for FleetStatusPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Playing), spawn_fleet_sidebar)
            .add_systems(OnExit(GameState::Playing), despawn_fleet_sidebar)
            .add_systems(
                Update,
                (
                    rebuild_ship_cards,
                    update_hull_bars,
                    update_engine_bars,
                    update_weapon_cells,
                    update_selection_highlight,
                    handle_card_click,
                    update_destroyed_cards,
                    update_cooldown_bars,
                )
                    .chain()
                    .run_if(in_state(GameState::Playing)),
            );
    }
}

// ── Marker components ──────────────────────────────────────────────────

#[derive(Component)]
struct FleetSidebar;

/// Tracks the number of ship cards currently spawned so we can detect changes.
#[derive(Component)]
struct SidebarShipCount(usize);

#[derive(Component)]
struct ShipCard(Entity);

#[derive(Component)]
struct HullBar(Entity);

#[derive(Component)]
struct EngineBar(Entity);

#[derive(Component)]
struct WeaponCell(Entity, usize);

#[derive(Component)]
struct CooldownBar(Entity, usize);

// ── Spawn / despawn ────────────────────────────────────────────────────

fn spawn_fleet_sidebar(mut commands: Commands) {
    commands.spawn((
        FleetSidebar,
        SidebarShipCount(0),
        GlobalZIndex(10),
        Pickable::IGNORE,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Px(200.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(4.0)),
            row_gap: Val::Px(4.0),
            ..default()
        },
    ));
}

fn despawn_fleet_sidebar(
    mut commands: Commands,
    query: Query<Entity, With<FleetSidebar>>,
) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
}

// ── Rebuild ship cards ─────────────────────────────────────────────────

fn rebuild_ship_cards(
    mut commands: Commands,
    local_team: Res<LocalTeam>,
    ships: Query<(Entity, &Team, &ShipClass, &Health, &Mounts), With<Ship>>,
    secrets: Query<(&ShipSecretsOwner, &ShipNumber), With<ShipSecrets>>,
    mut sidebar_q: Query<(Entity, &mut SidebarShipCount), With<FleetSidebar>>,
    existing_cards: Query<Entity, With<ShipCard>>,
) {
    let Some(my_team) = local_team.0 else { return };
    let Ok((sidebar_entity, mut ship_count)) = sidebar_q.single_mut() else {
        return;
    };

    // Build a map of ship entity → ship number from ShipSecrets
    let ship_numbers: std::collections::HashMap<Entity, u8> = secrets
        .iter()
        .map(|(owner, num)| (owner.0, num.0))
        .collect();

    // Collect friendly ships sorted by ShipNumber
    let mut friendly_ships: Vec<_> = ships
        .iter()
        .filter(|(_, team, _, _, _)| **team == my_team)
        .collect();
    friendly_ships.sort_by_key(|(entity, _, _, _, _)| {
        ship_numbers.get(entity).copied().unwrap_or(255)
    });

    // Only rebuild if count changed
    if friendly_ships.len() == ship_count.0 {
        return;
    }
    ship_count.0 = friendly_ships.len();

    // Despawn old cards
    for card_entity in &existing_cards {
        commands.entity(card_entity).despawn();
    }

    // Spawn new cards as children of sidebar
    commands.entity(sidebar_entity).with_children(|parent| {
        for (ship_entity, _, class, health, mounts) in &friendly_ships {
            let number = ship_numbers.get(ship_entity).copied().unwrap_or(0);
            spawn_ship_card(parent, *ship_entity, class, number, health, mounts);
        }
    });
}

fn spawn_ship_card(
    parent: &mut ChildSpawnerCommands<'_>,
    ship_entity: Entity,
    class: &ShipClass,
    number: u8,
    health: &Health,
    mounts: &Mounts,
) {
    let profile = class.profile();
    let class_name = match class {
        ShipClass::Battleship => "Battleship",
        ShipClass::Destroyer => "Destroyer",
        ShipClass::Scout => "Scout",
    };

    parent
        .spawn((
            ShipCard(ship_entity),
            Interaction::default(),
            Node {
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(4.0)),
                row_gap: Val::Px(2.0),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.1, 0.1, 0.15, 0.9)),
            BorderColor::all(Color::srgba(0.3, 0.3, 0.4, 0.8)),
        ))
        .with_children(|card| {
            // Row 1: Ship number + class name
            card.spawn((
                Text::new(format!("{}  {}", number, class_name)),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                Pickable::IGNORE,
            ));

            // Row 2: Hull bar
            spawn_hull_bar(card, ship_entity, health.hp, profile.hp);

            // Row 3: Engine bar
            spawn_engine_bar(card, ship_entity, profile.engine_hp, profile.engine_hp);

            // Row 4: Weapon slots row
            card.spawn((
                Pickable::IGNORE,
                Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(3.0),
                    flex_wrap: FlexWrap::Wrap,
                    ..default()
                },
            ))
            .with_children(|row| {
                for (i, mount) in mounts.0.iter().enumerate() {
                    spawn_weapon_cell(row, ship_entity, i, mount);
                }
            });
        });
}

fn bar_row_node() -> Node {
    Node {
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        column_gap: Val::Px(4.0),
        height: Val::Px(12.0),
        ..default()
    }
}

fn bar_label(label: &str) -> impl Bundle {
    (
        Text::new(label.to_string()),
        TextFont {
            font_size: 10.0,
            ..default()
        },
        TextColor(Color::srgba(0.7, 0.7, 0.7, 1.0)),
        Pickable::IGNORE,
        Node {
            width: Val::Px(26.0),
            ..default()
        },
    )
}

fn bar_background() -> impl Bundle {
    (
        Pickable::IGNORE,
        Node {
            width: Val::Px(140.0),
            height: Val::Px(8.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.15, 0.15, 0.15, 1.0)),
    )
}

fn bar_fill(pct: f32, color: Color) -> impl Bundle {
    (
        Pickable::IGNORE,
        Node {
            width: Val::Percent(pct * 100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        BackgroundColor(color),
    )
}

fn hull_fill_color(pct: f32) -> Color {
    if pct < 0.25 {
        Color::srgba(0.9, 0.15, 0.15, 1.0)
    } else {
        Color::srgba(0.2, 0.8, 0.2, 1.0)
    }
}

fn spawn_hull_bar(
    parent: &mut ChildSpawnerCommands<'_>,
    ship_entity: Entity,
    current: u16,
    max: u16,
) {
    let pct = if max > 0 { current as f32 / max as f32 } else { 0.0 };

    parent
        .spawn((Pickable::IGNORE, bar_row_node()))
        .with_children(|row| {
            row.spawn(bar_label("Hull"));
            row.spawn(bar_background()).with_children(|bg| {
                bg.spawn((HullBar(ship_entity), bar_fill(pct, hull_fill_color(pct))));
            });
        });
}

fn spawn_engine_bar(
    parent: &mut ChildSpawnerCommands<'_>,
    ship_entity: Entity,
    current: u16,
    max: u16,
) {
    let pct = if max > 0 { current as f32 / max as f32 } else { 0.0 };
    let color = if current == 0 {
        Color::srgba(0.5, 0.1, 0.1, 1.0)
    } else {
        Color::srgba(0.2, 0.4, 0.9, 1.0)
    };

    parent
        .spawn((Pickable::IGNORE, bar_row_node()))
        .with_children(|row| {
            row.spawn(bar_label("Eng"));
            row.spawn(bar_background()).with_children(|bg| {
                bg.spawn((EngineBar(ship_entity), bar_fill(pct, color)));
            });
        });
}

fn weapon_abbreviation(wt: &WeaponType) -> &'static str {
    match wt {
        WeaponType::HeavyCannon => "HC",
        WeaponType::Cannon => "CN",
        WeaponType::Railgun => "RG",
        WeaponType::HeavyVLS => "HV",
        WeaponType::LightVLS => "LV",
        WeaponType::LaserPD => "LP",
        WeaponType::CWIS => "CW",
        WeaponType::SearchRadar => "SR",
        WeaponType::NavRadar => "NR",
    }
}

fn weapon_status_text(
    wt: &WeaponType,
    ammo: u16,
    tubes_loaded: u8,
) -> String {
    let abbr = weapon_abbreviation(wt);
    match wt.category() {
        WeaponCategory::Missile => {
            let total_tubes = wt.profile().tubes;
            format!("{}{}/{}", abbr, tubes_loaded, total_tubes)
        }
        WeaponCategory::PointDefense => {
            if *wt == WeaponType::LaserPD {
                // Energy-based, no ammo count
                abbr.to_string()
            } else {
                format!("{}{}", abbr, ammo)
            }
        }
        WeaponCategory::Sensor => abbr.to_string(),
        WeaponCategory::Cannon => format!("{}{}", abbr, ammo),
    }
}

fn spawn_weapon_cell(
    parent: &mut ChildSpawnerCommands<'_>,
    ship_entity: Entity,
    mount_index: usize,
    mount: &crate::weapon::Mount,
) {
    let (text, dot_color) = if let Some(ref ws) = mount.weapon {
        let txt = weapon_status_text(&ws.weapon_type, ws.ammo, ws.tubes_loaded);
        let color = if mount.hp > 0 {
            Color::srgba(0.2, 0.9, 0.2, 1.0) // green = online
        } else {
            Color::srgba(0.9, 0.15, 0.15, 1.0) // red = offline
        };
        (txt, color)
    } else {
        ("--".to_string(), Color::srgba(0.4, 0.4, 0.4, 1.0)) // gray = empty
    };

    parent
        .spawn((
            WeaponCell(ship_entity, mount_index),
            Pickable::IGNORE,
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(2.0),
                ..default()
            },
        ))
        .with_children(|cell| {
            // Status dot
            cell.spawn((
                Pickable::IGNORE,
                Node {
                    width: Val::Px(6.0),
                    height: Val::Px(6.0),
                    ..default()
                },
                BackgroundColor(dot_color),
            ));

            // Weapon text
            cell.spawn((
                Text::new(text),
                TextFont {
                    font_size: 10.0,
                    ..default()
                },
                TextColor(Color::srgba(0.85, 0.85, 0.85, 1.0)),
                Pickable::IGNORE,
            ));

            // Cooldown bar fill
            cell.spawn((
                CooldownBar(ship_entity, mount_index),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    bottom: Val::Px(0.0),
                    width: Val::Percent(0.0),
                    height: Val::Px(2.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(1.0, 1.0, 0.3, 0.7)),
                Pickable::IGNORE,
            ));
        });
}

// ── Per-frame update systems ───────────────────────────────────────────

fn update_hull_bars(
    ships: Query<(&Health, &ShipClass), With<Ship>>,
    mut bars: Query<(&HullBar, &mut Node, &mut BackgroundColor)>,
) {
    for (hull_bar, mut node, mut bg) in &mut bars {
        let Ok((health, class)) = ships.get(hull_bar.0) else {
            continue;
        };
        let max = class.profile().hp;
        let pct = if max > 0 {
            health.hp as f32 / max as f32
        } else {
            0.0
        };
        node.width = Val::Percent(pct * 100.0);
        bg.0 = if pct < 0.25 {
            Color::srgba(0.9, 0.15, 0.15, 1.0)
        } else {
            Color::srgba(0.2, 0.8, 0.2, 1.0)
        };
    }
}

fn update_engine_bars(
    ships: Query<&EngineHealth, With<Ship>>,
    mut bars: Query<(&EngineBar, &mut Node, &mut BackgroundColor)>,
) {
    for (engine_bar, mut node, mut bg) in &mut bars {
        let Ok(engine) = ships.get(engine_bar.0) else {
            continue;
        };
        let pct = if engine.max_hp > 0 {
            engine.hp as f32 / engine.max_hp as f32
        } else {
            0.0
        };
        node.width = Val::Percent(pct * 100.0);
        bg.0 = if engine.hp == 0 {
            Color::srgba(0.5, 0.1, 0.1, 1.0) // dark red when offline
        } else {
            Color::srgba(0.2, 0.4, 0.9, 1.0) // blue when online
        };
    }
}

fn update_weapon_cells(
    ships: Query<&Mounts, With<Ship>>,
    cells: Query<(&WeaponCell, &Children)>,
    mut texts: Query<&mut Text>,
    mut bg_colors: Query<&mut BackgroundColor>,

) {
    for (weapon_cell, cell_children) in &cells {
        let Ok(mounts) = ships.get(weapon_cell.0) else {
            continue;
        };
        let Some(mount) = mounts.0.get(weapon_cell.1) else {
            continue;
        };

        let (new_text, dot_color) = if let Some(ref ws) = mount.weapon {
            let txt = weapon_status_text(&ws.weapon_type, ws.ammo, ws.tubes_loaded);
            let color = if mount.hp > 0 {
                Color::srgba(0.2, 0.9, 0.2, 1.0)
            } else {
                Color::srgba(0.9, 0.15, 0.15, 1.0)
            };
            (txt, color)
        } else {
            ("--".to_string(), Color::srgba(0.4, 0.4, 0.4, 1.0))
        };

        // cell_children[0] = dot, cell_children[1] = text
        if cell_children.len() >= 2 {
            // Update dot color
            if let Ok(mut bg) = bg_colors.get_mut(cell_children[0]) {
                bg.0 = dot_color;
            }
            // Update text
            if let Ok(mut text) = texts.get_mut(cell_children[1]) {
                **text = new_text;
            }
        }
    }
}

// ── Selection highlighting ───────────────────────────────────────────

fn update_selection_highlight(
    ships: Query<(), With<Selected>>,
    mut cards: Query<(&ShipCard, &mut BorderColor)>,
) {
    for (card, mut border) in &mut cards {
        *border = if ships.get(card.0).is_ok() {
            BorderColor::all(Color::srgba(0.2, 1.0, 0.2, 0.9))
        } else {
            BorderColor::all(Color::srgba(0.3, 0.3, 0.4, 0.8))
        };
    }
}

fn handle_card_click(
    mut commands: Commands,
    cards: Query<(&ShipCard, &Interaction), Changed<Interaction>>,
    selected: Query<Entity, With<Selected>>,
) {
    for (card, interaction) in &cards {
        if *interaction == Interaction::Pressed {
            // Deselect all ships
            for entity in &selected {
                commands.entity(entity).remove::<Selected>();
            }
            // Select the clicked card's ship
            commands.entity(card.0).insert(Selected);
        }
    }
}

// ── Destroyed ship graying ───────────────────────────────────────────

fn update_destroyed_cards(
    ships: Query<(), With<Destroyed>>,
    mut cards: Query<(&ShipCard, &mut BackgroundColor)>,
) {
    for (card, mut bg) in &mut cards {
        if ships.get(card.0).is_ok() {
            bg.0 = Color::srgba(0.15, 0.15, 0.15, 0.5);
        }
    }
}

// ── Cooldown bars ────────────────────────────────────────────────────

fn update_cooldown_bars(
    ships: Query<&Mounts, With<Ship>>,
    mut bars: Query<(&CooldownBar, &mut Node)>,
) {
    for (cb, mut node) in &mut bars {
        let Ok(mounts) = ships.get(cb.0) else {
            continue;
        };
        let Some(mount) = mounts.0.get(cb.1) else {
            continue;
        };
        let progress = if let Some(ref ws) = mount.weapon {
            let profile = ws.weapon_type.profile();
            match ws.weapon_type.category() {
                WeaponCategory::Cannon | WeaponCategory::Sensor => {
                    if profile.fire_rate_secs > 0.0 {
                        1.0 - (ws.cooldown / profile.fire_rate_secs)
                    } else {
                        1.0
                    }
                }
                WeaponCategory::PointDefense => {
                    if ws.weapon_type == WeaponType::CWIS {
                        1.0
                    } else if profile.fire_rate_secs > 0.0 {
                        1.0 - (ws.cooldown / profile.fire_rate_secs)
                    } else {
                        1.0
                    }
                }
                WeaponCategory::Missile => {
                    let total_tubes = profile.tubes;
                    if total_tubes > 0 && ws.tubes_loaded < total_tubes {
                        if profile.fire_rate_secs > 0.0 {
                            1.0 - (ws.tube_reload_timer / profile.fire_rate_secs)
                        } else {
                            1.0
                        }
                    } else {
                        1.0
                    }
                }
            }
        } else {
            0.0
        };
        node.width = Val::Percent(progress.clamp(0.0, 1.0) * 100.0);
    }
}
