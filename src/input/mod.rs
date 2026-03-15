use bevy::prelude::*;
use bevy_replicon::shared::message::client_event::ClientTriggerExt;

use crate::game::Team;
use crate::map::GroundPlane;
use crate::net::commands::{FacingLockCommand, FacingUnlockCommand, MoveCommand};
use crate::net::LocalTeam;
use crate::ship::{FacingLocked, Selected, SelectionIndicator, Ship};

pub struct InputPlugin;

/// Resource: when true, next right-click sets facing lock direction
#[derive(Resource, Default)]
pub struct LockMode(pub bool);

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LockMode>()
            .add_systems(Startup, (setup_selection_indicator, setup_lock_mode_hud))
            .add_systems(
                Update,
                (update_selection_indicator, handle_keyboard, update_lock_mode_hud),
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
    ship_query: Query<(Entity, &Team), With<Ship>>,
    selected_query: Query<Entity, With<Selected>>,
) {
    let clicked_entity = click.event_target();
    let Ok((entity, team)) = ship_query.get(clicked_entity) else {
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

    if click.button != PointerButton::Secondary {
        return;
    }

    // Lock mode: next right-click sets facing
    if lock_mode.0 {
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
    selected_query: Query<Entity, With<Selected>>,
    locked_query: Query<Entity, (With<Selected>, With<FacingLocked>)>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        for entity in &selected_query {
            commands.entity(entity).remove::<Selected>();
        }
        lock_mode.0 = false;
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
    }
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
