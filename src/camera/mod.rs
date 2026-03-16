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

/// Supreme Commander-style zoom: scroll zooms toward the point on the ground
/// plane (Y=0) under the mouse cursor. Zoom in pulls toward cursor, zoom out
/// pushes away from cursor.
fn camera_zoom(
    scroll: Res<AccumulatedMouseScroll>,
    settings: Res<CameraSettings>,
    windows: Query<&Window>,
    mut cam_query: Query<(&mut Transform, &Camera, &GlobalTransform), With<GameCamera>>,
) {
    if scroll.delta.y.abs() < 0.001 {
        return;
    }

    let Ok(window) = windows.single() else {
        return;
    };
    let Ok((mut transform, camera, global_tf)) = cam_query.single_mut() else {
        return;
    };

    // Get mouse cursor position → ray → ground plane intersection
    let cursor_pos = window.cursor_position().unwrap_or(Vec2::new(
        window.width() / 2.0,
        window.height() / 2.0,
    ));

    let ground_target = camera
        .viewport_to_world(global_tf, cursor_pos)
        .ok()
        .and_then(|ray| {
            // Intersect ray with Y=0 plane
            let ray_origin = ray.origin;
            let ray_dir = ray.direction.as_vec3();
            if ray_dir.y.abs() < 0.001 {
                return None;
            }
            let t = -ray_origin.y / ray_dir.y;
            if t < 0.0 {
                return None;
            }
            Some(ray_origin + ray_dir * t)
        })
        .unwrap_or_else(|| {
            // Fallback: use camera forward intersection with ground
            let forward = transform.forward().as_vec3();
            if forward.y.abs() > 0.001 {
                let dist = transform.translation.y / (-forward.y).max(0.001);
                transform.translation + forward * dist
            } else {
                Vec3::ZERO
            }
        });

    // Zoom factor — multiplicative for smooth feel at all distances
    let zoom_factor = 1.0 - scroll.delta.y * 0.1;
    let zoom_factor = zoom_factor.clamp(0.8, 1.2);

    // Zoom in: toward cursor. Zoom out: toward map center (so max zoom shows whole map).
    let zooming_in = scroll.delta.y > 0.0;
    let anchor = if zooming_in {
        ground_target
    } else {
        Vec3::ZERO // map center
    };

    let offset = transform.translation - anchor;
    let new_offset = offset * zoom_factor;
    let new_pos = anchor + new_offset;

    if new_pos.y > settings.min_zoom && new_pos.y < settings.max_zoom {
        transform.translation = new_pos;
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
