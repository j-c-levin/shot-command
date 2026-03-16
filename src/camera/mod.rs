use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;

use crate::game::GameState;
use crate::net::LocalTeam;
use crate::game::Team;
use crate::ship::Ship;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(CameraSettings::default())
            .add_systems(Startup, spawn_camera)
            .add_systems(OnEnter(GameState::Playing), center_camera_on_fleet)
            .add_systems(Update, (camera_pan, camera_zoom, camera_rotate));
    }
}

#[derive(Component)]
pub struct GameCamera;

#[derive(Resource)]
pub struct CameraSettings {
    pub pan_speed: f32,
    pub zoom_speed: f32,
    pub rotate_speed: f32,
    pub min_zoom: f32,
    pub max_zoom: f32,
}

impl Default for CameraSettings {
    fn default() -> Self {
        Self {
            pan_speed: 300.0,
            zoom_speed: 50.0,
            rotate_speed: 0.005,
            min_zoom: 50.0,
            max_zoom: 1500.0,
        }
    }
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        GameCamera,
        Camera3d::default(),
        Transform::from_xyz(0.0, 400.0, 200.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 3000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(200.0, 500.0, 100.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.1, 0.1, 0.2),
        brightness: 200.0,
        ..default()
    });
}

fn camera_pan(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    settings: Res<CameraSettings>,
    mut query: Query<&mut Transform, With<GameCamera>>,
) {
    let Ok(mut transform) = query.single_mut() else {
        return;
    };

    let mut direction = Vec3::ZERO;

    if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
        direction.z += 1.0;
    }
    if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
        direction.z -= 1.0;
    }
    if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
        direction.x -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
        direction.x += 1.0;
    }

    if direction.length_squared() > 0.0 {
        direction = direction.normalize();

        let forward = transform.forward().as_vec3();
        let forward_xz = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
        let right_xz = Vec3::new(-forward_xz.z, 0.0, forward_xz.x);

        let movement = (forward_xz * direction.z + right_xz * direction.x)
            * settings.pan_speed
            * time.delta_secs();

        transform.translation += movement;
    }
}

/// Supreme Commander-style zoom:
/// - Scroll up: zoom in toward the ground point under the cursor
/// - Scroll down: zoom out, pulling toward map center so max zoom sees everything
///
/// Algorithm: record ground point under cursor BEFORE zoom, change camera height,
/// then pan so the same ground point stays under the cursor (zoom in only).
fn camera_zoom(
    scroll: Res<AccumulatedMouseScroll>,
    settings: Res<CameraSettings>,
    mut query: Query<&mut Transform, With<GameCamera>>,
) {
    if scroll.delta.y.abs() < 0.001 {
        return;
    }

    let Ok(mut transform) = query.single_mut() else {
        return;
    };

    let zooming_in = scroll.delta.y > 0.0;

    // Current height and look-at point (where camera forward hits Y=0)
    let forward = transform.forward().as_vec3();
    let look_ground = if forward.y.abs() > 0.001 {
        let t = transform.translation.y / (-forward.y);
        if t > 0.0 {
            transform.translation + forward * t
        } else {
            Vec3::ZERO
        }
    } else {
        Vec3::ZERO
    };

    // Zoom: adjust height by a percentage
    let zoom_speed = if zooming_in { 0.85 } else { 1.15 };
    let old_height = transform.translation.y;
    let new_height = (old_height * zoom_speed).clamp(settings.min_zoom, settings.max_zoom);

    if (new_height - old_height).abs() < 0.01 {
        return;
    }

    let height_ratio = new_height / old_height;

    if zooming_in {
        // Zoom in: keep the look-at ground point fixed, move camera closer to it
        // Camera XZ moves toward look_ground by the same ratio as height shrinks
        let new_x = look_ground.x + (transform.translation.x - look_ground.x) * height_ratio;
        let new_z = look_ground.z + (transform.translation.z - look_ground.z) * height_ratio;
        transform.translation = Vec3::new(new_x, new_height, new_z);
    } else {
        // Zoom out: pull camera XZ toward map center (0, 0) so max zoom centers the map
        // Blend toward center: as we zoom out, gradually pull XZ toward origin
        // so max zoom centers on the map
        let center_blend = ((new_height - settings.min_zoom)
            / (settings.max_zoom - settings.min_zoom))
            .clamp(0.0, 1.0);
        let final_x = transform.translation.x * (1.0 - center_blend * 0.1);
        let final_z = transform.translation.z * (1.0 - center_blend * 0.1);
        transform.translation = Vec3::new(final_x, new_height, final_z);
    }
}

fn camera_rotate(
    mouse_button: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    settings: Res<CameraSettings>,
    mut query: Query<&mut Transform, With<GameCamera>>,
) {
    if !mouse_button.pressed(MouseButton::Middle) {
        return;
    }

    if mouse_motion.delta.length_squared() < 0.001 {
        return;
    }

    let Ok(mut transform) = query.single_mut() else {
        return;
    };

    // Orbit around the point where the camera ray hits the ground plane (Y=0)
    let forward = transform.forward().as_vec3();
    let look_target = if forward.y.abs() > 0.001 {
        let dist = transform.translation.y / (-forward.y).max(0.001);
        transform.translation + forward * dist
    } else {
        Vec3::ZERO
    };

    let yaw = -mouse_motion.delta.x * settings.rotate_speed;
    let rotation = Quat::from_rotation_y(yaw);

    let offset = transform.translation - look_target;
    let rotated_offset = rotation * offset;
    transform.translation = look_target + rotated_offset;
    transform.look_at(look_target, Vec3::Y);
}

/// Center the camera above the player's fleet when entering Playing.
/// TeamAssignment is sent by the server after fleet spawning, so ships
/// should already be replicated by the time this runs.
fn center_camera_on_fleet(
    local_team: Res<LocalTeam>,
    ships: Query<(&Transform, &Team), With<Ship>>,
    mut camera: Query<&mut Transform, (With<GameCamera>, Without<Ship>)>,
) {
    let Some(my_team) = local_team.0 else {
        return;
    };

    let mut sum = Vec3::ZERO;
    let mut count = 0u32;
    for (transform, team) in &ships {
        if *team == my_team {
            sum += transform.translation;
            count += 1;
        }
    }

    if count == 0 {
        return;
    }

    let center = sum / count as f32;
    let Ok(mut cam) = camera.single_mut() else {
        return;
    };

    let height = 400.0;
    let offset_z = 200.0;
    cam.translation = Vec3::new(center.x, height, center.z + offset_z);
    cam.look_at(Vec3::new(center.x, 0.0, center.z), Vec3::Y);
}
