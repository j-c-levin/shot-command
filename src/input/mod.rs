use std::collections::HashMap;

use bevy::prelude::*;
use bevy_replicon::shared::message::client_event::ClientTriggerExt;

use crate::game::Team;
use crate::map::GroundPlane;
use crate::net::commands::{
    CancelMissilesCommand, ClearTargetCommand, FacingLockCommand, FacingUnlockCommand,
    FireMissileCommand, JoinSquadCommand, MoveCommand, TargetCommand,
};
use crate::net::LocalTeam;
use crate::ship::{
    FacingLocked, Selected, SelectionIndicator, Ship, ShipNumber, ShipSecrets, ShipSecretsOwner,
    SquadMember, TargetDesignation,
};
use crate::weapon::{Mounts, WeaponCategory};

pub struct InputPlugin;

/// Resource: when true, next right-click sets facing lock direction
#[derive(Resource, Default)]
pub struct LockMode(pub bool);

/// Resource: when true, next left-click on enemy designates target
#[derive(Resource, Default)]
pub struct TargetMode(pub bool);

/// Resource: when true, clicks queue missile launches
#[derive(Resource, Default)]
pub struct MissileMode(pub bool);

/// Resource: when true, next click on friendly ship or number-key assigns squad
#[derive(Resource, Default)]
pub struct JoinMode(pub bool);

/// Resource: when true, right-click issues move commands
#[derive(Resource, Default)]
pub struct MoveMode(pub bool);

/// Tracks numbered enemy assignments for K/M mode keyboard targeting.
#[derive(Resource, Debug, Default)]
pub struct EnemyNumbers {
    pub assignments: HashMap<Entity, u8>,
    pub active: bool,
}

/// Marker for the mode indicator text in the bottom-left corner.
#[derive(Component)]
pub struct ModeIndicatorText;

/// Marker for ships highlighted as squad followers of the selected leader.
#[derive(Component)]
pub struct SquadHighlight;

/// Marker for the weapon range indicator ring.
#[derive(Component)]
struct RangeIndicator;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LockMode>()
            .init_resource::<TargetMode>()
            .init_resource::<MissileMode>()
            .init_resource::<JoinMode>()
            .init_resource::<MoveMode>()
            .init_resource::<EnemyNumbers>()
            .add_systems(
                Startup,
                (
                    setup_selection_indicator,
                    setup_range_indicator,
                    setup_mode_indicator,
                ),
            )
            .add_systems(
                Update,
                (
                    update_selection_indicator,
                    update_range_indicator,
                    handle_keyboard,
                    handle_number_keys,
                    update_squad_highlights,
                    update_mode_indicator,
                    update_enemy_numbers,
                ),
            );
    }
}

fn setup_selection_indicator(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        SelectionIndicator,
        Mesh3d(meshes.add(Torus::new(14.0, 16.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgba(0.2, 1.0, 0.3, 0.6),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        })),
        Transform::from_xyz(0.0, -1000.0, 0.0),
        Visibility::Hidden,
        Pickable::IGNORE,
    ));
}

pub fn on_ship_clicked(
    click: On<Pointer<Click>>,
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    local_team: Res<LocalTeam>,
    mut lock_mode: ResMut<LockMode>,
    mut target_mode: ResMut<TargetMode>,
    mut missile_mode: ResMut<MissileMode>,
    mut join_mode: ResMut<JoinMode>,
    mut move_mode: ResMut<MoveMode>,
    ship_query: Query<(Entity, &Team, &Transform), With<Ship>>,
    selected_query: Query<Entity, With<Selected>>,
) {
    let clicked_entity = click.event_target();
    let Ok((entity, team, transform)) = ship_query.get(clicked_entity) else {
        return;
    };

    let Some(my_team) = local_team.0 else {
        return;
    };

    // Alt+right-click on own ship: unlock facing
    if click.button == PointerButton::Secondary
        && keys.pressed(KeyCode::AltLeft)
        && *team == my_team
    {
        commands.client_trigger(FacingUnlockCommand { ship: entity });
        return;
    }

    // Right-click modes on enemy ships
    if click.button == PointerButton::Secondary && *team != my_team {
        // Missile mode: right-click on enemy queues a missile
        if missile_mode.0 {
            let target_pos = Vec2::new(transform.translation.x, transform.translation.z);
            for selected_ship in &selected_query {
                commands.client_trigger(FireMissileCommand {
                    ship: selected_ship,
                    target_point: target_pos,
                    target_entity: Some(entity),
                });
            }
            return;
        }

        // Target mode: right-click on enemy designates target
        if target_mode.0 {
            for selected_ship in &selected_query {
                commands.client_trigger(TargetCommand {
                    ship: selected_ship,
                    target: entity,
                });
            }
            target_mode.0 = false;
            return;
        }
    }

    if click.button != PointerButton::Primary {
        return;
    }

    // Join mode: left-click on friendly ship assigns squad
    if join_mode.0 && *team == my_team {
        for selected_ship in &selected_query {
            if selected_ship != entity {
                commands.client_trigger(JoinSquadCommand {
                    ship: selected_ship,
                    leader: entity,
                });
            }
        }
        join_mode.0 = false;
        return;
    }

    if *team != my_team {
        return;
    }

    // Deselect previous and reset all modes
    for prev in &selected_query {
        commands.entity(prev).remove::<Selected>();
    }
    lock_mode.0 = false;
    target_mode.0 = false;
    missile_mode.0 = false;
    join_mode.0 = false;
    move_mode.0 = false;

    commands.entity(entity).insert(Selected);
}

pub fn on_ground_clicked(
    click: On<Pointer<Click>>,
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut lock_mode: ResMut<LockMode>,
    mut target_mode: ResMut<TargetMode>,
    mut missile_mode: ResMut<MissileMode>,
    mut join_mode: ResMut<JoinMode>,
    mut move_mode: ResMut<MoveMode>,
    local_team: Res<LocalTeam>,
    ground_query: Query<Entity, With<GroundPlane>>,
    selected_query: Query<(Entity, &Transform, &Team), With<Selected>>,
) {
    let clicked_entity = click.event_target();
    if ground_query.get(clicked_entity).is_err() {
        return;
    }
    let Some(hit_pos) = click.hit.position else {
        return;
    };
    let destination = Vec2::new(hit_pos.x, hit_pos.z);

    let Some(my_team) = local_team.0 else {
        return;
    };

    // Missile mode: right-click on ground queues a missile at that position
    if click.button == PointerButton::Secondary && missile_mode.0 {
        for (entity, _transform, team) in &selected_query {
            if *team != my_team {
                continue;
            }
            commands.client_trigger(FireMissileCommand {
                ship: entity,
                target_point: destination,
                target_entity: None,
            });
        }
        // Don't exit missile mode
        return;
    }

    // Alt+right-click or lock-mode right-click: set facing direction and lock
    if click.button == PointerButton::Secondary
        && (keys.pressed(KeyCode::AltLeft) || lock_mode.0)
    {
        for (entity, transform, team) in &selected_query {
            if *team != my_team {
                continue;
            }
            let pos = Vec2::new(transform.translation.x, transform.translation.z);
            let dir = (destination - pos).normalize_or_zero();
            if dir != Vec2::ZERO {
                commands.client_trigger(FacingLockCommand {
                    ship: entity,
                    direction: dir,
                });
            }
        }
        lock_mode.0 = false;
        return;
    }

    // Left-click on ground (not in any mode): deselect and exit all modes
    if click.button == PointerButton::Primary {
        for (entity, _, _) in &selected_query {
            commands.entity(entity).remove::<Selected>();
        }
        lock_mode.0 = false;
        target_mode.0 = false;
        missile_mode.0 = false;
        join_mode.0 = false;
        move_mode.0 = false;
        return;
    }

    if click.button != PointerButton::Secondary {
        return;
    }

    // Target mode: right-click on ground does nothing (only enemy clicks target)
    if target_mode.0 {
        return;
    }

    // Move mode required for right-click move commands
    if !move_mode.0 {
        return;
    }

    // Shift+right-click: append waypoint
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);

    for (entity, _transform, team) in &selected_query {
        if *team != my_team {
            continue;
        }
        commands.client_trigger(MoveCommand {
            ship: entity,
            destination,
            append: shift,
        });
    }
}

fn update_selection_indicator(
    selected_query: Query<&Transform, (With<Selected>, With<Ship>, Without<SelectionIndicator>)>,
    mut indicator_query: Query<
        (&mut Transform, &mut Visibility),
        (With<SelectionIndicator>, Without<Ship>),
    >,
) {
    let Ok((mut indicator_transform, mut visibility)) = indicator_query.single_mut() else {
        return;
    };

    if let Some(ship_transform) = selected_query.iter().next() {
        indicator_transform.translation = Vec3::new(
            ship_transform.translation.x,
            ship_transform.translation.y,
            ship_transform.translation.z,
        );
        *visibility = Visibility::Visible;
    } else {
        *visibility = Visibility::Hidden;
    }
}

fn handle_keyboard(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut lock_mode: ResMut<LockMode>,
    mut target_mode: ResMut<TargetMode>,
    mut missile_mode: ResMut<MissileMode>,
    mut join_mode: ResMut<JoinMode>,
    mut move_mode: ResMut<MoveMode>,
    selected_query: Query<(Entity, &Transform), With<Selected>>,
    locked_query: Query<Entity, (With<Selected>, With<FacingLocked>)>,
    secrets_query: Query<(&ShipSecretsOwner, Option<&TargetDesignation>), With<ShipSecrets>>,
    mounts_query: Query<&Mounts, With<Selected>>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        for (entity, _) in &selected_query {
            commands.entity(entity).remove::<Selected>();
        }
        if missile_mode.0 {
            for (entity, _) in &selected_query {
                commands.client_trigger(CancelMissilesCommand { ship: entity });
            }
        }
        lock_mode.0 = false;
        target_mode.0 = false;
        missile_mode.0 = false;
        join_mode.0 = false;
        move_mode.0 = false;
    }

    // Space: toggle move mode
    if keys.just_pressed(KeyCode::Space) {
        move_mode.0 = !move_mode.0;
        if move_mode.0 {
            lock_mode.0 = false;
            target_mode.0 = false;
            missile_mode.0 = false;
            join_mode.0 = false;
        }
    }

    if keys.just_pressed(KeyCode::KeyL) {
        if locked_query.iter().next().is_some() {
            for entity in &locked_query {
                commands.client_trigger(FacingUnlockCommand { ship: entity });
            }
            lock_mode.0 = false;
        } else {
            lock_mode.0 = !lock_mode.0;
        }
        target_mode.0 = false;
        missile_mode.0 = false;
        join_mode.0 = false;
        move_mode.0 = false;
    }

    if keys.just_pressed(KeyCode::KeyK) {
        let mut has_target = false;
        for (selected_entity, _) in &selected_query {
            for (owner, target) in &secrets_query {
                if owner.0 == selected_entity && target.is_some() {
                    commands.client_trigger(ClearTargetCommand {
                        ship: selected_entity,
                    });
                    has_target = true;
                }
            }
        }
        if !has_target {
            target_mode.0 = !target_mode.0;
        } else {
            target_mode.0 = false;
        }
        lock_mode.0 = false;
        missile_mode.0 = false;
        join_mode.0 = false;
        move_mode.0 = false;
    }

    if keys.just_pressed(KeyCode::KeyM) {
        let has_vls = mounts_query.iter().any(|mounts| {
            mounts.0.iter().any(|m| {
                m.weapon
                    .as_ref()
                    .is_some_and(|w| w.weapon_type.category() == WeaponCategory::Missile)
            })
        });
        if has_vls {
            missile_mode.0 = !missile_mode.0;
        } else {
            missile_mode.0 = false;
        }
        lock_mode.0 = false;
        target_mode.0 = false;
        join_mode.0 = false;
        move_mode.0 = false;
    }

    if keys.just_pressed(KeyCode::KeyJ) {
        // Toggle join mode (only if a ship is selected)
        if selected_query.iter().next().is_some() {
            join_mode.0 = !join_mode.0;
        } else {
            join_mode.0 = false;
        }
        lock_mode.0 = false;
        target_mode.0 = false;
        missile_mode.0 = false;
        move_mode.0 = false;
    }

    // S key: full stop
    if keys.just_pressed(KeyCode::KeyS) {
        for (entity, transform) in &selected_query {
            let pos = Vec2::new(transform.translation.x, transform.translation.z);
            commands.client_trigger(MoveCommand {
                ship: entity,
                destination: pos,
                append: false,
            });
            commands.client_trigger(FacingUnlockCommand { ship: entity });
            commands.client_trigger(ClearTargetCommand { ship: entity });
            commands.client_trigger(CancelMissilesCommand { ship: entity });
        }
        lock_mode.0 = false;
        target_mode.0 = false;
        missile_mode.0 = false;
        join_mode.0 = false;
        move_mode.0 = false;
    }
}

// ── Range Indicator ─────────────────────────────────────────────────────

fn setup_range_indicator(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Unit-radius torus — we scale it at runtime to match weapon range
    commands.spawn((
        RangeIndicator,
        Mesh3d(meshes.add(Torus::new(0.98, 1.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 1.0, 1.0, 0.15),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        })),
        Transform::from_xyz(0.0, -1000.0, 0.0),
        Visibility::Hidden,
        Pickable::IGNORE,
    ));
}

fn update_range_indicator(
    target_mode: Res<TargetMode>,
    missile_mode: Res<MissileMode>,
    selected_query: Query<(&Transform, &Mounts), (With<Selected>, With<Ship>)>,
    mut indicator_query: Query<
        (&mut Transform, &mut Visibility),
        (With<RangeIndicator>, Without<Ship>),
    >,
) {
    let Ok((mut indicator_tf, mut visibility)) = indicator_query.single_mut() else {
        return;
    };

    // Determine which category to show range for
    let category = if target_mode.0 {
        Some(WeaponCategory::Cannon)
    } else if missile_mode.0 {
        Some(WeaponCategory::Missile)
    } else {
        None
    };

    let Some(category) = category else {
        *visibility = Visibility::Hidden;
        return;
    };

    let Some((ship_tf, mounts)) = selected_query.iter().next() else {
        *visibility = Visibility::Hidden;
        return;
    };

    // Find max range across all weapons of this category.
    // Missiles use fuel range; cannons use firing range.
    let max_range = mounts
        .0
        .iter()
        .filter_map(|m| {
            let w = m.weapon.as_ref()?;
            let profile = w.weapon_type.profile();
            if w.weapon_type.category() != category {
                return None;
            }
            let range = if category == WeaponCategory::Missile {
                profile.missile_fuel
            } else {
                profile.firing_range
            };
            Some(range)
        })
        .fold(0.0_f32, f32::max);

    if max_range < 1.0 {
        *visibility = Visibility::Hidden;
        return;
    }

    indicator_tf.translation = Vec3::new(
        ship_tf.translation.x,
        0.5,
        ship_tf.translation.z,
    );
    indicator_tf.scale = Vec3::splat(max_range);
    *visibility = Visibility::Visible;
}

// ── Number-Key Ship Selection ────────────────────────────────────────────

/// Maps digit key codes to ship numbers (1-9).
fn digit_key_to_number(key: KeyCode) -> Option<u8> {
    match key {
        KeyCode::Digit1 => Some(1),
        KeyCode::Digit2 => Some(2),
        KeyCode::Digit3 => Some(3),
        KeyCode::Digit4 => Some(4),
        KeyCode::Digit5 => Some(5),
        KeyCode::Digit6 => Some(6),
        KeyCode::Digit7 => Some(7),
        KeyCode::Digit8 => Some(8),
        KeyCode::Digit9 => Some(9),
        _ => None,
    }
}

/// System that handles 1-9 key presses to select ships by their ShipNumber.
/// In join mode, the number key targets a ship for the join command instead.
fn handle_number_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    local_team: Res<LocalTeam>,
    mut lock_mode: ResMut<LockMode>,
    mut target_mode: ResMut<TargetMode>,
    mut missile_mode: ResMut<MissileMode>,
    mut join_mode: ResMut<JoinMode>,
    mut move_mode: ResMut<MoveMode>,
    ship_query: Query<(Entity, &Team, &Transform), With<Ship>>,
    selected_query: Query<Entity, With<Selected>>,
    secrets_query: Query<(&ShipSecretsOwner, &ShipNumber), With<ShipSecrets>>,
    enemy_numbers: Res<EnemyNumbers>,
) {
    let Some(my_team) = local_team.0 else {
        return;
    };

    let digit_keys = [
        KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3,
        KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6,
        KeyCode::Digit7, KeyCode::Digit8, KeyCode::Digit9,
    ];

    for &key in &digit_keys {
        if !keys.just_pressed(key) {
            continue;
        }
        let Some(number) = digit_key_to_number(key) else {
            continue;
        };

        // K mode: target enemy by number
        if target_mode.0 && enemy_numbers.active {
            let enemy = enemy_numbers.assignments.iter().find(|&(_, &n)| n == number).map(|(&e, _)| e);
            if let Some(enemy_entity) = enemy {
                for selected_ship in &selected_query {
                    commands.client_trigger(TargetCommand {
                        ship: selected_ship,
                        target: enemy_entity,
                    });
                }
                target_mode.0 = false;
            }
            return;
        }

        // M mode: fire missile at enemy by number
        if missile_mode.0 && enemy_numbers.active {
            let enemy = enemy_numbers.assignments.iter().find(|&(_, &n)| n == number).map(|(&e, _)| e);
            if let Some(enemy_entity) = enemy {
                if let Ok((_, _, transform)) = ship_query.get(enemy_entity) {
                    let target_pos = Vec2::new(transform.translation.x, transform.translation.z);
                    for selected_ship in &selected_query {
                        commands.client_trigger(FireMissileCommand {
                            ship: selected_ship,
                            target_point: target_pos,
                            target_entity: Some(enemy_entity),
                        });
                    }
                }
            }
            // Stay in M mode
            return;
        }

        // Find the ship on our team with this number via ShipSecrets
        // (ShipNumber lives only on ShipSecrets, not on Ship entities).
        let target_ship = secrets_query
            .iter()
            .find(|(_, sn)| sn.0 == number)
            .and_then(|(owner, _)| {
                let (entity, team, _) = ship_query.get(owner.0).ok()?;
                if *team == my_team { Some(entity) } else { None }
            });

        let Some(target_ship) = target_ship else {
            continue;
        };

        // In join mode: assign squad instead of selecting
        if join_mode.0 {
            for selected_ship in &selected_query {
                if selected_ship != target_ship {
                    commands.client_trigger(JoinSquadCommand {
                        ship: selected_ship,
                        leader: target_ship,
                    });
                }
            }
            join_mode.0 = false;
            return;
        }

        // Normal mode: select this ship
        for prev in &selected_query {
            commands.entity(prev).remove::<Selected>();
        }
        lock_mode.0 = false;
        target_mode.0 = false;
        missile_mode.0 = false;
        join_mode.0 = false;
        move_mode.0 = false;

        commands.entity(target_ship).insert(Selected);
        return;
    }
}

// ── Mode Indicator ──────────────────────────────────────────────────────

fn setup_mode_indicator(mut commands: Commands) {
    commands.spawn((
        ModeIndicatorText,
        Text::new(""),
        TextFont {
            font_size: 18.0,
            ..default()
        },
        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.8)),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(8.0),
            left: Val::Px(12.0),
            ..default()
        },
    ));
}

fn update_mode_indicator(
    move_mode: Res<MoveMode>,
    target_mode: Res<TargetMode>,
    missile_mode: Res<MissileMode>,
    join_mode: Res<JoinMode>,
    lock_mode: Res<LockMode>,
    mut query: Query<(&mut Text, &mut TextColor), With<ModeIndicatorText>>,
) {
    let Ok((mut text, mut color)) = query.single_mut() else {
        return;
    };

    let (label, c) = if target_mode.0 {
        ("TARGET", Color::srgba(1.0, 0.3, 0.3, 0.9))
    } else if missile_mode.0 {
        ("MISSILE", Color::srgba(1.0, 0.5, 0.1, 0.9))
    } else if move_mode.0 {
        ("MOVE", Color::srgba(0.3, 1.0, 0.3, 0.9))
    } else if join_mode.0 {
        ("JOIN", Color::srgba(0.3, 1.0, 0.8, 0.9))
    } else if lock_mode.0 {
        ("LOCK", Color::srgba(1.0, 0.8, 0.2, 0.9))
    } else {
        ("", Color::srgba(1.0, 1.0, 1.0, 0.0))
    };

    *text = Text::new(label);
    *color = TextColor(c);
}

// ── Enemy Numbers ───────────────────────────────────────────────────────

/// System that populates/clears EnemyNumbers based on current mode.
fn update_enemy_numbers(
    target_mode: Res<TargetMode>,
    missile_mode: Res<MissileMode>,
    local_team: Res<LocalTeam>,
    mut enemy_numbers: ResMut<EnemyNumbers>,
    ship_query: Query<(Entity, &Team), With<Ship>>,
) {
    let should_be_active = target_mode.0 || missile_mode.0;

    if !should_be_active {
        if enemy_numbers.active {
            enemy_numbers.active = false;
            enemy_numbers.assignments.clear();
        }
        return;
    }

    // Already active — don't reassign (stable numbers while mode is on)
    if enemy_numbers.active {
        return;
    }

    let Some(my_team) = local_team.0 else {
        return;
    };

    // Collect visible enemy ships and sort by entity index for stability
    let mut enemies: Vec<Entity> = ship_query
        .iter()
        .filter(|(_, team)| **team != my_team)
        .map(|(e, _)| e)
        .collect();
    enemies.sort_by_key(|e| e.index());

    enemy_numbers.assignments.clear();
    for (i, entity) in enemies.iter().enumerate().take(9) {
        enemy_numbers.assignments.insert(*entity, (i + 1) as u8);
    }
    enemy_numbers.active = true;
}

// ── Squad Highlights ─────────────────────────────────────────────────────

/// System that marks squad followers of the selected leader with SquadHighlight.
fn update_squad_highlights(
    mut commands: Commands,
    selected_query: Query<Entity, With<Selected>>,
    highlight_query: Query<Entity, With<SquadHighlight>>,
    secrets_query: Query<(&ShipSecretsOwner, &SquadMember), With<ShipSecrets>>,
) {
    // Remove all existing highlights
    for entity in &highlight_query {
        commands.entity(entity).remove::<SquadHighlight>();
    }

    // Get the currently selected ship
    let Some(selected) = selected_query.iter().next() else {
        return;
    };

    // Find followers whose leader is the selected ship (via ShipSecrets)
    for (owner, squad) in &secrets_query {
        if squad.leader == selected {
            commands.entity(owner.0).insert(SquadHighlight);
        }
    }
}
