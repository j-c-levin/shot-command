use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(CameraSettings::default())
            .add_systems(Startup, spawn_camera)
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
            max_zoom: 800.0,
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

    commands.spawn(AmbientLight {
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
        direction.z -= 1.0;
    }
    if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
        direction.z += 1.0;
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

    let zoom_amount = -scroll.delta.y * settings.zoom_speed;
    let forward = transform.forward().as_vec3();
    let new_pos = transform.translation + forward * zoom_amount;

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
