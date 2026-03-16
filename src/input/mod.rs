use bevy::prelude::*;
use bevy_replicon::shared::message::client_event::ClientTriggerExt;

use crate::game::Team;
use crate::map::GroundPlane;
use crate::net::commands::{
    CancelMissilesCommand, ClearTargetCommand, FacingLockCommand, FacingUnlockCommand,
    FireMissileCommand, MoveCommand, TargetCommand,
};
use crate::net::LocalTeam;
use crate::ship::{
    FacingLocked, Selected, SelectionIndicator, Ship, ShipSecrets, ShipSecretsOwner,
    TargetDesignation,
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

/// Marker for the weapon range indicator ring.
#[derive(Component)]
struct RangeIndicator;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LockMode>()
            .init_resource::<TargetMode>()
            .init_resource::<MissileMode>()
            .add_systems(
                Startup,
                (
                    setup_selection_indicator,
                    setup_range_indicator,
                    setup_lock_mode_hud,
                    setup_target_mode_hud,
                    setup_missile_mode_hud,
                ),
            )
            .add_systems(
                Update,
                (
                    update_selection_indicator,
                    update_range_indicator,
                    handle_keyboard,
                    update_lock_mode_hud,
                    update_target_mode_hud,
                    update_missile_mode_hud,
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

    commands.entity(entity).insert(Selected);
}

pub fn on_ground_clicked(
    click: On<Pointer<Click>>,
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut lock_mode: ResMut<LockMode>,
    mut target_mode: ResMut<TargetMode>,
    mut missile_mode: ResMut<MissileMode>,
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

    // Alt+right-click: set facing direction and lock
    if click.button == PointerButton::Secondary && keys.pressed(KeyCode::AltLeft) {
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

    // Lock mode: right-click sets facing direction
    if click.button == PointerButton::Secondary && lock_mode.0 {
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
        return;
    }

    if click.button != PointerButton::Secondary {
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
    }

    if keys.just_pressed(KeyCode::KeyL) {
        if locked_query.iter().next().is_some() {
            // Some selected ships are locked — unlock them via network trigger
            for entity in &locked_query {
                commands.client_trigger(FacingUnlockCommand { ship: entity });
            }
            lock_mode.0 = false;
        } else {
            // No selected ships locked — toggle lock mode
            lock_mode.0 = !lock_mode.0;
        }
        target_mode.0 = false;
        missile_mode.0 = false;
    }

    if keys.just_pressed(KeyCode::KeyK) {
        // Check if selected ship already has a target — if so, clear it
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
            // No target — toggle target mode
            target_mode.0 = !target_mode.0;
        } else {
            target_mode.0 = false;
        }
        lock_mode.0 = false;
        missile_mode.0 = false;
    }

    if keys.just_pressed(KeyCode::KeyM) {
        // Only enter missile mode if selected ship has a VLS mount
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
    }

    // S key: full stop — clear waypoints (move to self), unlock facing, clear target, cancel missiles
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

// ── Lock Mode HUD ───────────────────────────────────────────────────────

#[derive(Component)]
struct LockModeHud;

fn setup_lock_mode_hud(mut commands: Commands) {
    commands.spawn((
        LockModeHud,
        Text::new("LOCK MODE — Right-click to set facing"),
        TextFont {
            font_size: 24.0,
            ..default()
        },
        TextColor(Color::srgba(1.0, 0.8, 0.2, 0.9)),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(20.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        },
        Visibility::Hidden,
    ));
}

fn update_lock_mode_hud(
    lock_mode: Res<LockMode>,
    mut query: Query<&mut Visibility, With<LockModeHud>>,
) {
    let Ok(mut vis) = query.single_mut() else {
        return;
    };
    *vis = if lock_mode.0 {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
}

// ── Target Mode HUD ─────────────────────────────────────────────────────

#[derive(Component)]
struct TargetModeHud;

fn setup_target_mode_hud(mut commands: Commands) {
    commands.spawn((
        TargetModeHud,
        Text::new("TARGET MODE — Right-click enemy to designate"),
        TextFont {
            font_size: 24.0,
            ..default()
        },
        TextColor(Color::srgba(1.0, 0.3, 0.3, 0.9)),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(50.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        },
        Visibility::Hidden,
    ));
}

fn update_target_mode_hud(
    target_mode: Res<TargetMode>,
    mut query: Query<&mut Visibility, With<TargetModeHud>>,
) {
    let Ok(mut vis) = query.single_mut() else {
        return;
    };
    *vis = if target_mode.0 {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
}

// ── Missile Mode HUD ────────────────────────────────────────────────────

#[derive(Component)]
struct MissileModeHud;

fn setup_missile_mode_hud(mut commands: Commands) {
    commands.spawn((
        MissileModeHud,
        Text::new("MISSILE MODE — Right-click enemy or ground to fire"),
        TextFont {
            font_size: 24.0,
            ..default()
        },
        TextColor(Color::srgba(1.0, 0.5, 0.1, 0.9)),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(80.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        },
        Visibility::Hidden,
    ));
}

fn update_missile_mode_hud(
    missile_mode: Res<MissileMode>,
    mut query: Query<&mut Visibility, With<MissileModeHud>>,
) {
    let Ok(mut vis) = query.single_mut() else {
        return;
    };
    *vis = if missile_mode.0 {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
}
