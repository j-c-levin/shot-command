use bevy::prelude::*;

use crate::game::Team;
use crate::map::GroundPlane;
use crate::ship::{MovementTarget, Selected, SelectionIndicator, Ship};

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_selection_indicator)
            .add_systems(
                Update,
                (update_selection_indicator, handle_keyboard_deselect),
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
    ));
}

pub fn on_ship_clicked(
    click: On<Pointer<Click>>,
    mut commands: Commands,
    ship_query: Query<(Entity, &Team), With<Ship>>,
    selected_query: Query<Entity, With<Selected>>,
) {
    if click.button != PointerButton::Primary {
        return;
    }

    let clicked_entity = click.event_target();

    let Ok((entity, team)) = ship_query.get(clicked_entity) else {
        return;
    };

    if *team != Team::PLAYER {
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
    ground_query: Query<Entity, With<GroundPlane>>,
    selected_query: Query<Entity, With<Selected>>,
) {
    if click.button != PointerButton::Secondary {
        return;
    }

    let clicked_entity = click.event_target();

    if ground_query.get(clicked_entity).is_err() {
        return;
    }

    let Some(hit_pos) = click.hit.position else {
        return;
    };

    let destination = Vec2::new(hit_pos.x, hit_pos.z);

    for entity in &selected_query {
        commands.entity(entity).insert(MovementTarget { destination });
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

fn handle_keyboard_deselect(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    selected_query: Query<Entity, With<Selected>>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        for entity in &selected_query {
            commands.entity(entity).remove::<Selected>();
        }
    }
}
