use std::collections::HashMap;

use bevy::prelude::*;
use bevy_replicon::shared::message::client_event::ClientTriggerExt;

use crate::camera::GameCamera;
use crate::game::Team;
use crate::map::GroundPlane;
use crate::net::commands::{
    CancelMissilesCommand, ClearTargetCommand, FacingLockCommand, FacingUnlockCommand,
    FireMissileCommand, JoinSquadCommand, MoveCommand, RadarToggleCommand,
    TargetByContactCommand, TargetCommand,
};
use crate::net::LocalTeam;
use crate::radar::{ContactKind, ContactLevel, ContactSourceShip, ContactTeam, RadarContact};
use crate::ship::{
    FacingLocked, Selected, Ship, ShipNumber, ShipSecrets, ShipSecretsOwner,
    SquadMember, TargetDesignation, rotate_offset, ship_heading,
};
use crate::weapon::{Mounts, WeaponCategory};

pub struct InputPlugin;

/// The current input mode. Only one mode can be active at a time.
#[derive(Resource, Default, PartialEq, Eq, Clone, Copy, Debug)]
pub enum InputMode {
    #[default]
    Normal,
    Move,
    Lock,
    Target,
    Missile,
    Join,
}

/// Tracks a right-click drag gesture for move+facing commands.
#[derive(Resource, Debug, Default)]
pub struct MoveGestureState {
    /// Whether a right-click drag is currently active.
    pub active: bool,
    /// Ground XZ position where the right-click landed.
    pub destination: Vec2,
    /// Screen position of the initial click (for drag distance threshold).
    pub screen_start: Vec2,
    /// Whether shift was held when the gesture started (append waypoint).
    pub append: bool,
}

/// Tracks numbered enemy assignments for K/M mode keyboard targeting.
/// Keys are the entity to use for targeting (ship entity or contact entity).
/// `source_map` tracks source_ship → number for stable numbering across
/// contact re-detection (when a ship leaves and re-enters radar range).
#[derive(Resource, Debug, Default)]
pub struct EnemyNumbers {
    pub assignments: HashMap<Entity, u8>,
    /// Maps source ship entity → assigned number, for stable contact numbering.
    pub source_numbers: HashMap<Entity, u8>,
}

/// Marker for the mode indicator text in the bottom-left corner.
#[derive(Component)]
pub struct ModeIndicatorText;

/// Marker for ships highlighted as squad followers of the selected leader.
#[derive(Component)]
pub struct SquadHighlight;


impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InputMode>()
            .init_resource::<MoveGestureState>()
            .init_resource::<EnemyNumbers>()
            .add_systems(
                Startup,
                setup_mode_indicator,
            )
            .add_systems(
                Update,
                (
                    draw_selection_gizmos,
                    draw_range_gizmos,
                    draw_gesture_preview,
                    // handle_keyboard toggles modes → update_enemy_numbers populates
                    // assignments → handle_number_keys reads them. Must be ordered.
                    (handle_keyboard, update_enemy_numbers, handle_number_keys).chain(),
                    handle_move_gesture,
                    update_squad_highlights,
                    update_mode_indicator,
                ),
            );
    }
}

/// Rotation to lay a circle flat on the XZ ground plane (rotate 90 degrees around X).
fn ground_circle_isometry(center: Vec3, _radius: f32) -> Isometry3d {
    Isometry3d::new(
        center,
        Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
    )
}

/// Draw a green gizmo circle under the selected ship and gray circles under squad highlights.
fn draw_selection_gizmos(
    mut gizmos: Gizmos,
    selected_query: Query<&Transform, (With<Selected>, With<Ship>)>,
    highlight_query: Query<&Transform, With<SquadHighlight>>,
) {
    // Green circle for selected ship
    for ship_tf in &selected_query {
        let center = Vec3::new(ship_tf.translation.x, 1.0, ship_tf.translation.z);
        gizmos.circle(ground_circle_isometry(center, 15.0), 15.0, Color::srgba(0.2, 1.0, 0.3, 0.6));
    }

    // Gray circles for squad-highlighted followers
    for tf in &highlight_query {
        let center = Vec3::new(tf.translation.x, 1.0, tf.translation.z);
        gizmos.circle(ground_circle_isometry(center, 14.0), 14.0, Color::srgba(0.5, 0.5, 0.5, 0.4));
    }
}

pub fn on_ship_clicked(
    click: On<Pointer<Click>>,
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    local_team: Res<LocalTeam>,
    mut mode: ResMut<InputMode>,
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
        if *mode == InputMode::Missile {
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
        if *mode == InputMode::Target {
            for selected_ship in &selected_query {
                commands.client_trigger(TargetCommand {
                    ship: selected_ship,
                    target: entity,
                });
            }
            *mode = InputMode::Normal;
            return;
        }
    }

    if click.button != PointerButton::Primary {
        return;
    }

    // Join mode: left-click on friendly ship assigns squad
    if *mode == InputMode::Join && *team == my_team {
        for selected_ship in &selected_query {
            if selected_ship != entity {
                commands.client_trigger(JoinSquadCommand {
                    ship: selected_ship,
                    leader: entity,
                });
            }
        }
        *mode = InputMode::Normal;
        return;
    }

    if *team != my_team {
        return;
    }

    // Deselect previous and reset all modes
    for prev in &selected_query {
        commands.entity(prev).remove::<Selected>();
    }
    *mode = InputMode::Normal;

    commands.entity(entity).insert(Selected);
}

pub fn on_ground_clicked(
    click: On<Pointer<Click>>,
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut mode: ResMut<InputMode>,
    local_team: Res<LocalTeam>,
    ground_query: Query<Entity, With<GroundPlane>>,
    selected_query: Query<(Entity, &Transform, &Team), With<Selected>>,
) {
    let clicked_entity = click.event_target();
    let Some(hit_pos) = click.hit.position else {
        return;
    };
    let destination = Vec2::new(hit_pos.x, hit_pos.z);

    let Some(my_team) = local_team.0 else {
        return;
    };

    let is_ground = ground_query.get(clicked_entity).is_ok();

    // Alt+right-click or lock-mode right-click: set facing direction and lock.
    // Works on any surface (ground, asteroid, etc.) — direction is what matters.
    if click.button == PointerButton::Secondary
        && (keys.pressed(KeyCode::AltLeft) || *mode == InputMode::Lock)
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
        *mode = InputMode::Normal;
        return;
    }

    // Everything below requires clicking the ground plane
    if !is_ground {
        return;
    }

    // Missile mode: right-click on ground queues a missile at that position
    if click.button == PointerButton::Secondary && *mode == InputMode::Missile {
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

    // Left-click on ground (not in any mode): deselect and exit all modes
    if click.button == PointerButton::Primary {
        for (entity, _, _) in &selected_query {
            commands.entity(entity).remove::<Selected>();
        }
        *mode = InputMode::Normal;
        return;
    }

    // Right-click move commands are now handled by handle_move_gesture system
    // (press+release detection with drag-to-face). The observer no longer handles them.
}

fn handle_keyboard(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut mode: ResMut<InputMode>,
    selected_query: Query<(Entity, &Transform), With<Selected>>,
    locked_query: Query<Entity, (With<Selected>, With<FacingLocked>)>,
    secrets_query: Query<(&ShipSecretsOwner, Option<&TargetDesignation>), With<ShipSecrets>>,
    mounts_query: Query<&Mounts, With<Selected>>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        for (entity, _) in &selected_query {
            commands.entity(entity).remove::<Selected>();
        }
        if *mode == InputMode::Missile {
            for (entity, _) in &selected_query {
                commands.client_trigger(CancelMissilesCommand { ship: entity });
            }
        }
        *mode = InputMode::Normal;
    }

    // Space: toggle move mode
    if keys.just_pressed(KeyCode::Space) {
        if *mode == InputMode::Move {
            *mode = InputMode::Normal;
        } else {
            *mode = InputMode::Move;
        }
    }

    if keys.just_pressed(KeyCode::KeyL) {
        if locked_query.iter().next().is_some() {
            for entity in &locked_query {
                commands.client_trigger(FacingUnlockCommand { ship: entity });
            }
            *mode = InputMode::Normal;
        } else if *mode == InputMode::Lock {
            *mode = InputMode::Normal;
        } else {
            *mode = InputMode::Lock;
        }
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
            if *mode == InputMode::Target {
                *mode = InputMode::Normal;
            } else {
                *mode = InputMode::Target;
            }
        } else {
            *mode = InputMode::Normal;
        }
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
            if *mode == InputMode::Missile {
                *mode = InputMode::Normal;
            } else {
                *mode = InputMode::Missile;
            }
        } else {
            *mode = InputMode::Normal;
        }
    }

    if keys.just_pressed(KeyCode::KeyJ) {
        // Toggle join mode (only if a ship is selected)
        if selected_query.iter().next().is_some() {
            if *mode == InputMode::Join {
                *mode = InputMode::Normal;
            } else {
                *mode = InputMode::Join;
            }
        } else {
            *mode = InputMode::Normal;
        }
    }

    // S key: full stop
    if keys.just_pressed(KeyCode::KeyS) {
        for (entity, transform) in &selected_query {
            let pos = Vec2::new(transform.translation.x, transform.translation.z);
            commands.client_trigger(MoveCommand {
                ship: entity,
                destination: pos,
                append: false,
                facing: None,
            });
            commands.client_trigger(FacingUnlockCommand { ship: entity });
            commands.client_trigger(ClearTargetCommand { ship: entity });
            commands.client_trigger(CancelMissilesCommand { ship: entity });
        }
        *mode = InputMode::Normal;
    }

    // R key: toggle radar for all selected ships
    if keys.just_pressed(KeyCode::KeyR) {
        for (entity, _) in &selected_query {
            commands.client_trigger(RadarToggleCommand { ship: entity });
        }
    }
}

// ── Range Gizmos ────────────────────────────────────────────────────────

/// Draw weapon range circles as gizmos when in target or missile mode.
fn draw_range_gizmos(
    mut gizmos: Gizmos,
    mode: Res<InputMode>,
    selected_query: Query<(&Transform, &Mounts), (With<Selected>, With<Ship>)>,
) {
    // Determine which category to show range for
    let category = if *mode == InputMode::Target {
        Some(WeaponCategory::Cannon)
    } else if *mode == InputMode::Missile {
        Some(WeaponCategory::Missile)
    } else {
        None
    };

    let Some(category) = category else {
        return;
    };

    let Some((ship_tf, mounts)) = selected_query.iter().next() else {
        return;
    };

    let center = Vec3::new(ship_tf.translation.x, 1.0, ship_tf.translation.z);

    // Collect distinct ranges for this weapon category
    let mut ranges_seen = Vec::new();
    for mount in &mounts.0 {
        let Some(w) = mount.weapon.as_ref() else { continue };
        if w.weapon_type.category() != category {
            continue;
        }
        let profile = w.weapon_type.profile();
        let range = if category == WeaponCategory::Missile {
            profile.missile_fuel
        } else {
            profile.firing_range
        };
        if range >= 1.0 && !ranges_seen.iter().any(|&r: &f32| (r - range).abs() < 0.1) {
            ranges_seen.push(range);
        }
    }

    let range_color = if category == WeaponCategory::Missile {
        Color::srgba(1.0, 0.5, 0.1, 0.3)
    } else {
        Color::srgba(1.0, 0.8, 0.2, 0.3)
    };

    for range in &ranges_seen {
        gizmos.circle(ground_circle_isometry(center, *range), *range, range_color);
    }
}

// ── Move Gesture (right-click drag for facing) ─────────────────────────

/// Raycast from screen cursor to ground (Y=0) plane. Returns XZ position.
fn cursor_to_ground(
    window: &Window,
    camera: &Camera,
    camera_global_tf: &GlobalTransform,
) -> Option<Vec2> {
    let cursor_pos = window.cursor_position()?;
    let ray = camera.viewport_to_world(camera_global_tf, cursor_pos).ok()?;
    let dir = ray.direction.as_vec3();
    if dir.y.abs() < 0.001 {
        return None;
    }
    let t = -ray.origin.y / dir.y;
    if t < 0.0 {
        return None;
    }
    Some(Vec2::new(ray.origin.x + dir.x * t, ray.origin.z + dir.z * t))
}

/// Minimum screen-space drag distance (pixels) to activate facing.
const DRAG_THRESHOLD_PX: f32 = 5.0;

/// System that detects right-click press/release for move+facing gestures.
/// On press: record destination and screen position.
/// On release: if drag < threshold → move only; if >= threshold → move + facing.
fn handle_move_gesture(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    local_team: Res<LocalTeam>,
    mode: Res<InputMode>,
    mut gesture: ResMut<MoveGestureState>,
    windows: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform), With<GameCamera>>,
    selected_query: Query<(Entity, &Team), (With<Selected>, With<Ship>)>,
) {
    let Some(my_team) = local_team.0 else { return };

    // Don't start gesture in modes that consume right-click differently
    let right_click_consumed = *mode == InputMode::Missile || *mode == InputMode::Target
        || keys.pressed(KeyCode::AltLeft) || *mode == InputMode::Lock;

    // Right-click press: start gesture
    if mouse.just_pressed(MouseButton::Right) && *mode == InputMode::Move && !right_click_consumed {
        let Ok(window) = windows.single() else { return };
        let Ok((camera, global_tf)) = camera_query.single() else { return };

        if let Some(ground_pos) = cursor_to_ground(window, camera, global_tf) {
            let screen_start = window.cursor_position().unwrap_or(Vec2::ZERO);
            let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
            gesture.active = true;
            gesture.destination = ground_pos;
            gesture.screen_start = screen_start;
            gesture.append = shift;
        }
    }

    // Right-click release: finalize gesture
    if mouse.just_released(MouseButton::Right) && gesture.active {
        gesture.active = false;

        let Ok(window) = windows.single() else { return };
        let screen_end = window.cursor_position().unwrap_or(gesture.screen_start);
        let drag_dist = screen_end.distance(gesture.screen_start);

        let facing = if drag_dist >= DRAG_THRESHOLD_PX {
            // Raycast current cursor to ground to get the drag endpoint
            let Ok((camera, global_tf)) = camera_query.single() else { return };
            if let Some(ground_end) = cursor_to_ground(window, camera, global_tf) {
                let dir = (ground_end - gesture.destination).normalize_or_zero();
                if dir != Vec2::ZERO { Some(dir) } else { None }
            } else {
                None
            }
        } else {
            None
        };

        for (entity, team) in &selected_query {
            if *team != my_team {
                continue;
            }
            commands.client_trigger(MoveCommand {
                ship: entity,
                destination: gesture.destination,
                append: gesture.append,
                facing,
            });
        }
    }
}

/// Draw a preview gizmo while the move gesture drag is active.
fn draw_gesture_preview(
    mut gizmos: Gizmos,
    gesture: Res<MoveGestureState>,
    windows: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform), With<GameCamera>>,
    selected_query: Query<&Transform, (With<Selected>, With<Ship>)>,
    highlight_query: Query<Entity, With<SquadHighlight>>,
    secrets_query: Query<(&ShipSecretsOwner, &SquadMember), With<ShipSecrets>>,
) {
    if !gesture.active {
        return;
    }

    let Ok(window) = windows.single() else { return };
    let screen_now = window.cursor_position().unwrap_or(gesture.screen_start);
    let drag_dist = screen_now.distance(gesture.screen_start);

    // Always show destination circle for leader
    let dest_3d = Vec3::new(gesture.destination.x, 1.0, gesture.destination.y);
    gizmos.circle(
        Isometry3d::new(dest_3d, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
        3.0,
        Color::srgba(0.3, 0.5, 1.0, 0.7),
    );

    // Get selected ship transform for heading computation
    let selected_tf: Option<&Transform> = selected_query.iter().next();

    // Compute facing direction and rotation delta if dragging
    let facing_dir = if drag_dist >= DRAG_THRESHOLD_PX {
        let Ok((camera, global_tf)) = camera_query.single() else { return };
        cursor_to_ground(window, camera, global_tf).and_then(|ground_end| {
            let dir = (ground_end - gesture.destination).normalize_or_zero();
            if dir != Vec2::ZERO { Some(dir) } else { None }
        })
    } else {
        None
    };

    // Show facing direction line
    if let Some(dir) = facing_dir {
        let end_3d = Vec3::new(
            dest_3d.x + dir.x * 20.0,
            1.0,
            dest_3d.z + dir.y * 20.0,
        );
        gizmos.line(dest_3d, end_3d, Color::srgba(0.0, 1.0, 1.0, 0.7));
    }

    // Show follower preview positions
    // Preview follower destinations (gray circles showing where followers will end up)
    let follower_color = Color::srgba(0.5, 0.5, 0.5, 0.5);
    // Need to find followers of the selected ship via SquadHighlight (already computed)
    if let Some(leader_tf) = selected_tf {
        let current_heading = ship_heading(leader_tf);
        let rotation_delta = facing_dir.map(|dir| {
            let desired_heading = dir.y.atan2(dir.x);
            desired_heading - current_heading
        });

        // Only show for highlighted followers (followers of the selected leader)
        let highlighted: Vec<Entity> = highlight_query.iter().collect();
        for (owner, squad) in &secrets_query {
            if !highlighted.contains(&owner.0) {
                continue;
            }
            let offset = squad.offset;
            let rotated = if let Some(delta) = rotation_delta {
                rotate_offset(offset, delta)
            } else {
                offset
            };
            let follower_dest = gesture.destination + rotated;
            let follower_3d = Vec3::new(follower_dest.x, 1.0, follower_dest.y);
            gizmos.circle(
                Isometry3d::new(follower_3d, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                2.0,
                follower_color,
            );
            gizmos.line(dest_3d, follower_3d, follower_color);
        }
    }
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
    mut mode: ResMut<InputMode>,
    ship_query: Query<(Entity, &Team, &Transform), With<Ship>>,
    selected_query: Query<Entity, With<Selected>>,
    secrets_query: Query<(&ShipSecretsOwner, &ShipNumber), With<ShipSecrets>>,
    enemy_numbers: Res<EnemyNumbers>,
    contact_transform_query: Query<(&Transform, &ContactSourceShip), With<RadarContact>>,
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
        if *mode == InputMode::Target {
            let enemy = enemy_numbers.assignments.iter().find(|&(_, &n)| n == number).map(|(&e, _)| e);
            if let Some(enemy_entity) = enemy {
                for selected_ship in &selected_query {
                    // If the entity is a ship, send TargetCommand directly.
                    // If it's a RadarContact, send TargetByContactCommand so the
                    // server can resolve the source ship.
                    if ship_query.get(enemy_entity).is_ok() {
                        commands.client_trigger(TargetCommand {
                            ship: selected_ship,
                            target: enemy_entity,
                        });
                    } else {
                        commands.client_trigger(TargetByContactCommand {
                            ship: selected_ship,
                            contact: enemy_entity,
                        });
                    }
                }
                *mode = InputMode::Normal;
            }
            return;
        }

        // M mode: fire missile at enemy by number
        if *mode == InputMode::Missile {
            let enemy = enemy_numbers.assignments.iter().find(|&(_, &n)| n == number).map(|(&e, _)| e);
            if let Some(enemy_entity) = enemy {
                // Get position and target entity — either from a ship or a radar contact
                let fire_info = if let Ok((_, _, transform)) = ship_query.get(enemy_entity) {
                    let pos = Vec2::new(transform.translation.x, transform.translation.z);
                    Some((pos, Some(enemy_entity)))
                } else if let Ok((transform, source)) = contact_transform_query.get(enemy_entity) {
                    let pos = Vec2::new(transform.translation.x, transform.translation.z);
                    // Use the source ship entity as the missile tracking target
                    Some((pos, Some(source.0)))
                } else {
                    None
                };

                if let Some((target_pos, target_entity)) = fire_info {
                    for selected_ship in &selected_query {
                        commands.client_trigger(FireMissileCommand {
                            ship: selected_ship,
                            target_point: target_pos,
                            target_entity,
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
        if *mode == InputMode::Join {
            for selected_ship in &selected_query {
                if selected_ship != target_ship {
                    commands.client_trigger(JoinSquadCommand {
                        ship: selected_ship,
                        leader: target_ship,
                    });
                }
            }
            *mode = InputMode::Normal;
            return;
        }

        // Normal mode: select this ship
        for prev in &selected_query {
            commands.entity(prev).remove::<Selected>();
        }
        *mode = InputMode::Normal;

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
    mode: Res<InputMode>,
    mut query: Query<(&mut Text, &mut TextColor), With<ModeIndicatorText>>,
) {
    let Ok((mut text, mut color)) = query.single_mut() else {
        return;
    };

    let (label, c) = match *mode {
        InputMode::Target => ("TARGET", Color::srgba(1.0, 0.3, 0.3, 0.9)),
        InputMode::Missile => ("MISSILE", Color::srgba(1.0, 0.5, 0.1, 0.9)),
        InputMode::Move => ("MOVE", Color::srgba(0.3, 1.0, 0.3, 0.9)),
        InputMode::Join => ("JOIN", Color::srgba(0.3, 1.0, 0.8, 0.9)),
        InputMode::Lock => ("LOCK", Color::srgba(1.0, 0.8, 0.2, 0.9)),
        InputMode::Normal => ("", Color::srgba(1.0, 1.0, 1.0, 0.0)),
    };

    *text = Text::new(label);
    *color = TextColor(c);
}

// ── Enemy Numbers ───────────────────────────────────────────────────────

/// System that populates/clears EnemyNumbers based on current mode.
/// Assigns numbers 1-9 to visible enemy ships, then to radar-tracked contacts
/// (Track level, Ship kind) whose source isn't already numbered.
fn update_enemy_numbers(
    mode: Res<InputMode>,
    local_team: Res<LocalTeam>,
    mut enemy_numbers: ResMut<EnemyNumbers>,
    ship_query: Query<(Entity, &Team), With<Ship>>,
    contact_query: Query<
        (Entity, &ContactLevel, &ContactTeam, &ContactKind, &ContactSourceShip),
        With<RadarContact>,
    >,
) {
    let should_be_active = *mode == InputMode::Target || *mode == InputMode::Missile;

    if !should_be_active {
        if !enemy_numbers.assignments.is_empty() {
            enemy_numbers.assignments.clear();
            // Keep source_numbers so re-entering K/M mode reuses the same numbers
        }
        return;
    }

    let Some(my_team) = local_team.0 else {
        return;
    };

    // Remove assignments for entities that no longer exist
    enemy_numbers.assignments.retain(|entity, _| {
        ship_query.get(*entity).is_ok() || contact_query.get(*entity).is_ok()
    });

    // Clean up source_numbers for sources that have no ship or contact anymore
    let active_sources: std::collections::HashSet<Entity> = {
        let mut set = std::collections::HashSet::new();
        for (e, _) in &ship_query {
            set.insert(e);
        }
        for (_, _, _, _, src) in &contact_query {
            set.insert(src.0);
        }
        set
    };
    enemy_numbers.source_numbers.retain(|src, _| active_sources.contains(src));

    // Assign numbers to visible enemy ships first (stable: existing assignments kept)
    let mut new_enemies: Vec<Entity> = ship_query
        .iter()
        .filter(|(_, team)| **team != my_team)
        .filter(|(e, _)| !enemy_numbers.assignments.contains_key(e))
        .map(|(e, _)| e)
        .collect();
    new_enemies.sort_by_key(|e| e.index());

    for entity in new_enemies {
        // Reuse source number if this ship was previously tracked
        let num = if let Some(&n) = enemy_numbers.source_numbers.get(&entity) {
            if !enemy_numbers.assignments.values().any(|v| *v == n) {
                Some(n)
            } else {
                None
            }
        } else {
            None
        };
        let num = num.or_else(|| {
            (1..=9u8).find(|n| !enemy_numbers.assignments.values().any(|v| v == n))
        });
        if let Some(num) = num {
            enemy_numbers.assignments.insert(entity, num);
            enemy_numbers.source_numbers.insert(entity, num);
        }
    }

    // Collect source entities that are already numbered (either as ships or via another contact)
    let already_numbered_sources: std::collections::HashSet<Entity> = enemy_numbers
        .assignments
        .keys()
        .filter_map(|e| {
            if ship_query.get(*e).is_ok() {
                return Some(*e);
            }
            contact_query.get(*e).ok().map(|(_, _, _, _, src)| src.0)
        })
        .collect();

    // Assign numbers to radar track contacts whose source isn't already numbered
    let mut new_contacts: Vec<(Entity, Entity)> = contact_query
        .iter()
        .filter(|(_, level, team, kind, source)| {
            **level == ContactLevel::Track
                && **kind == ContactKind::Ship
                && team.0 == my_team
                && !already_numbered_sources.contains(&source.0)
        })
        .filter(|(e, _, _, _, _)| !enemy_numbers.assignments.contains_key(e))
        .map(|(e, _, _, _, src)| (e, src.0))
        .collect();
    new_contacts.sort_by_key(|(e, _)| e.index());

    for (entity, source) in new_contacts {
        // Reuse the number this source previously had
        let num = if let Some(&n) = enemy_numbers.source_numbers.get(&source) {
            if !enemy_numbers.assignments.values().any(|v| *v == n) {
                Some(n)
            } else {
                None
            }
        } else {
            None
        };
        let num = num.or_else(|| {
            (1..=9u8).find(|n| !enemy_numbers.assignments.values().any(|v| v == n))
        });
        if let Some(num) = num {
            enemy_numbers.assignments.insert(entity, num);
            enemy_numbers.source_numbers.insert(source, num);
        }
    }
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
