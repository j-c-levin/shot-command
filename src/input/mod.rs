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

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LockMode>()
            .init_resource::<TargetMode>()
            .init_resource::<MissileMode>()
            .add_systems(
                Startup,
                (
                    setup_selection_indicator,
                    setup_lock_mode_hud,
                    setup_target_mode_hud,
                    setup_missile_mode_hud,
                ),
            )
            .add_systems(
                Update,
                (
                    update_selection_indicator,
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
        Mesh3d(meshes.add(Torus::new(10.0, 12.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgba(0.2, 0.8, 1.0, 0.5),
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
    mut target_mode: ResMut<TargetMode>,
    missile_mode: Res<MissileMode>,
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

    if click.button != PointerButton::Primary {
        return;
    }

    // Missile mode: left-click on enemy queues a missile
    if missile_mode.0 && *team != my_team {
        let target_pos = Vec2::new(transform.translation.x, transform.translation.z);
        for selected_ship in &selected_query {
            commands.client_trigger(FireMissileCommand {
                ship: selected_ship,
                target_point: target_pos,
                target_entity: Some(entity),
            });
        }
        // Don't exit missile mode — allow rapid clicking for volleys
        return;
    }

    // Target mode: left-click on enemy ship designates target
    if target_mode.0 && *team != my_team {
        for selected_ship in &selected_query {
            commands.client_trigger(TargetCommand {
                ship: selected_ship,
                target: entity,
            });
        }
        target_mode.0 = false;
        return;
    }

    if *team != my_team {
        return;
    }

    // Deselect previous
    for prev in &selected_query {
        commands.entity(prev).remove::<Selected>();
    }

    commands.entity(entity).insert(Selected);
}

pub fn on_ground_clicked(
    click: On<Pointer<Click>>,
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut lock_mode: ResMut<LockMode>,
    missile_mode: Res<MissileMode>,
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

    // Missile mode: left-click on ground queues a missile at that position
    if click.button == PointerButton::Primary && missile_mode.0 {
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

    // Lock mode: left-click sets facing direction
    if click.button == PointerButton::Primary && lock_mode.0 {
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
            1.0,
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
    selected_query: Query<Entity, With<Selected>>,
    locked_query: Query<Entity, (With<Selected>, With<FacingLocked>)>,
    secrets_query: Query<(&ShipSecretsOwner, Option<&TargetDesignation>), With<ShipSecrets>>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        for entity in &selected_query {
            commands.entity(entity).remove::<Selected>();
        }
        if missile_mode.0 {
            for entity in &selected_query {
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
        for selected_entity in &selected_query {
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
        missile_mode.0 = !missile_mode.0;
        lock_mode.0 = false;
        target_mode.0 = false;
    }
}

// ── Lock Mode HUD ───────────────────────────────────────────────────────

#[derive(Component)]
struct LockModeHud;

fn setup_lock_mode_hud(mut commands: Commands) {
    commands.spawn((
        LockModeHud,
        Text::new("LOCK MODE — Left-click to set facing"),
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
        Text::new("TARGET MODE — Click enemy to designate"),
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
        Text::new("MISSILE MODE — Click enemy or ground to fire"),
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
