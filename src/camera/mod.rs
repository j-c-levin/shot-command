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
            .insert_resource(CameraLookAt(Vec3::ZERO))
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

/// The ground point the camera is looking at. Updated explicitly by zoom/pan/orbit.
#[derive(Resource, Debug, Clone)]
pub struct CameraLookAt(pub Vec3);

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
    mut look_at: ResMut<CameraLookAt>,
    mut query: Query<&mut Transform, With<GameCamera>>,
) {
    let Ok(mut transform) = query.single_mut() else {
        return;
    };

    let mut direction = Vec3::ZERO;

    if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
        direction.z += 1.0;
    }
    // S key is used for stop command — use only arrow keys for backward pan
    if keys.pressed(KeyCode::ArrowDown) {
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
        look_at.0 += movement;
    }
}

/// Compute the ground point (Y=0) where the camera is looking.
pub fn camera_look_ground(cam_pos: Vec3, cam_forward: Vec3) -> Vec3 {
    if cam_forward.y.abs() > 0.001 {
        let t = cam_pos.y / (-cam_forward.y);
        if t > 0.0 {
            Vec3::new(
                cam_pos.x + cam_forward.x * t,
                0.0,
                cam_pos.z + cam_forward.z * t,
            )
        } else {
            Vec3::new(cam_pos.x, 0.0, cam_pos.z)
        }
    } else {
        Vec3::new(cam_pos.x, 0.0, cam_pos.z)
    }
}

/// Compute new camera position after a zoom step.
///
/// Zoom in: camera descends, XZ stays fixed (look-at point doesn't change).
/// Zoom out: camera ascends, XZ blends toward map center.
/// Returns `(new_position, new_look_at_ground)` — caller must re-orient camera.
pub fn compute_zoom(
    cam_pos: Vec3,
    look_ground: Vec3,
    scroll_y: f32,
    min_zoom: f32,
    max_zoom: f32,
) -> Option<(Vec3, Vec3)> {
    let zooming_in = scroll_y > 0.0;

    let zoom_speed = if zooming_in { 0.85 } else { 1.18 };
    let old_height = cam_pos.y;
    let new_height = (old_height * zoom_speed).clamp(min_zoom, max_zoom);

    if (new_height - old_height).abs() < 0.01 {
        return None;
    }

    if zooming_in {
        // Zoom in: keep the same look-at point. Camera descends straight toward
        // its look-at point, maintaining the viewing angle.
        let height_ratio = new_height / old_height;
        let new_x = look_ground.x + (cam_pos.x - look_ground.x) * height_ratio;
        let new_z = look_ground.z + (cam_pos.z - look_ground.z) * height_ratio;
        Some((Vec3::new(new_x, new_height, new_z), look_ground))
    } else {
        // Zoom out: keep same look-at point but blend it toward map center.
        // At max zoom, look-at converges to (0, 0, 0).
        let height_ratio = new_height / old_height;
        let new_x = look_ground.x + (cam_pos.x - look_ground.x) * height_ratio;
        let new_z = look_ground.z + (cam_pos.z - look_ground.z) * height_ratio;

        // Blend the look-at point toward center as we zoom out
        let zoom_frac = ((new_height - min_zoom) / (max_zoom - min_zoom)).clamp(0.0, 1.0);
        let blend = zoom_frac * 0.15; // gentle pull toward center
        let new_look_x = look_ground.x * (1.0 - blend);
        let new_look_z = look_ground.z * (1.0 - blend);
        let new_look = Vec3::new(new_look_x, 0.0, new_look_z);

        // Recompute camera XZ offset from the new look-at point
        let offset_x = new_x - look_ground.x;
        let offset_z = new_z - look_ground.z;
        let final_x = new_look.x + offset_x;
        let final_z = new_look.z + offset_z;

        Some((Vec3::new(final_x, new_height, final_z), new_look))
    }
}

/// Zoom system: adjusts camera height and re-orients to maintain look-at point.
/// Zoom in targets the cursor ground point. Zoom out uses existing look-at.
fn camera_zoom(
    scroll: Res<AccumulatedMouseScroll>,
    settings: Res<CameraSettings>,
    windows: Query<&Window>,
    mut look_at: ResMut<CameraLookAt>,
    mut cam_query: Query<(&mut Transform, &Camera, &GlobalTransform), With<GameCamera>>,
) {
    if scroll.delta.y.abs() < 0.001 {
        return;
    }

    let Ok((mut transform, camera, global_tf)) = cam_query.single_mut() else {
        return;
    };

    let zooming_in = scroll.delta.y > 0.0;

    // For zoom-in: use cursor ground point as the anchor (zoom toward mouse)
    // For zoom-out: use current look-at (zoom out centers on map via compute_zoom)
    let anchor = if zooming_in {
        let cursor_ground = windows
            .single()
            .ok()
            .and_then(|w| w.cursor_position())
            .and_then(|cursor_pos| {
                camera.viewport_to_world(global_tf, cursor_pos).ok()
            })
            .and_then(|ray| {
                let dir = ray.direction.as_vec3();
                if dir.y.abs() < 0.001 {
                    return None;
                }
                let t = -ray.origin.y / dir.y;
                if t < 0.0 {
                    return None;
                }
                Some(Vec3::new(
                    ray.origin.x + dir.x * t,
                    0.0,
                    ray.origin.z + dir.z * t,
                ))
            });
        cursor_ground.unwrap_or(look_at.0)
    } else {
        look_at.0
    };

    let Some((new_pos, _)) = compute_zoom(
        transform.translation,
        anchor,
        scroll.delta.y,
        settings.min_zoom,
        settings.max_zoom,
    ) else {
        return;
    };

    // Compute the actual ground look-at from the new position, preserving the
    // camera's current viewing direction (forward vector stays the same).
    let cam_forward = transform.forward().as_vec3();
    transform.translation = new_pos;
    let actual_look = camera_look_ground(new_pos, cam_forward);
    look_at.0 = actual_look;
    transform.look_at(actual_look, Vec3::Y);
}

fn camera_rotate(
    mouse_button: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    settings: Res<CameraSettings>,
    look_at: Res<CameraLookAt>,
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

    let yaw = -mouse_motion.delta.x * settings.rotate_speed;
    let rotation = Quat::from_rotation_y(yaw);

    let offset = transform.translation - look_at.0;
    let rotated_offset = rotation * offset;
    transform.translation = look_at.0 + rotated_offset;
    transform.look_at(look_at.0, Vec3::Y);
}

/// Center the camera above the player's fleet when entering Playing.
/// TeamAssignment is sent by the server after fleet spawning, so ships
/// should already be replicated by the time this runs.
fn center_camera_on_fleet(
    local_team: Res<LocalTeam>,
    mut look_at: ResMut<CameraLookAt>,
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
    let look_target = Vec3::new(center.x, 0.0, center.z);
    cam.translation = Vec3::new(center.x, height, center.z + offset_z);
    cam.look_at(look_target, Vec3::Y);
    look_at.0 = look_target;
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── camera_look_ground ──────────────────────────────────────────

    #[test]
    fn look_ground_straight_down() {
        let cam_pos = Vec3::new(0.0, 400.0, 0.0);
        let cam_forward = Vec3::new(0.0, -1.0, 0.0); // looking straight down
        let ground = camera_look_ground(cam_pos, cam_forward);
        assert!((ground.x).abs() < 0.01);
        assert!((ground.z).abs() < 0.01);
        assert!((ground.y).abs() < 0.01);
    }

    #[test]
    fn look_ground_angled() {
        // Camera at (0, 400, 200) looking toward (0, 0, 0) — forward is roughly (0, -0.89, -0.45)
        let cam_pos = Vec3::new(0.0, 400.0, 200.0);
        let cam_forward = Vec3::new(0.0, -400.0, -200.0).normalize();
        let ground = camera_look_ground(cam_pos, cam_forward);
        // Should hit near origin
        assert!(ground.x.abs() < 1.0, "x={}", ground.x);
        assert!(ground.z.abs() < 1.0, "z={}", ground.z);
    }

    // ── compute_zoom ────────────────────────────────────────────────

    #[test]
    fn zoom_in_lowers_height() {
        let cam_pos = Vec3::new(0.0, 400.0, 200.0);
        let look = Vec3::new(0.0, 0.0, 0.0);
        let result = compute_zoom(cam_pos, look, 1.0, 50.0, 1500.0);
        let (new_pos, _) = result.unwrap();
        assert!(new_pos.y < cam_pos.y, "zoom in should lower height: {} vs {}", new_pos.y, cam_pos.y);
    }

    #[test]
    fn zoom_out_raises_height() {
        let cam_pos = Vec3::new(0.0, 400.0, 200.0);
        let look = Vec3::new(0.0, 0.0, 0.0);
        let result = compute_zoom(cam_pos, look, -1.0, 50.0, 1500.0);
        let (new_pos, _) = result.unwrap();
        assert!(new_pos.y > cam_pos.y, "zoom out should raise height: {} vs {}", new_pos.y, cam_pos.y);
    }

    #[test]
    fn zoom_in_preserves_look_at_point() {
        let cam_pos = Vec3::new(100.0, 400.0, 300.0);
        let look = Vec3::new(50.0, 0.0, 100.0);
        let result = compute_zoom(cam_pos, look, 1.0, 50.0, 1500.0);
        let (_, new_look) = result.unwrap();
        // Zoom in should keep the same look-at point
        assert!((new_look.x - look.x).abs() < 0.01, "look x changed: {} vs {}", new_look.x, look.x);
        assert!((new_look.z - look.z).abs() < 0.01, "look z changed: {} vs {}", new_look.z, look.z);
    }

    #[test]
    fn zoom_out_pulls_look_toward_center() {
        let cam_pos = Vec3::new(200.0, 400.0, 300.0);
        let look = Vec3::new(200.0, 0.0, 100.0);
        let result = compute_zoom(cam_pos, look, -1.0, 50.0, 1500.0);
        let (_, new_look) = result.unwrap();
        // New look-at should be closer to origin than original
        let old_dist = Vec2::new(look.x, look.z).length();
        let new_dist = Vec2::new(new_look.x, new_look.z).length();
        assert!(new_dist < old_dist, "zoom out should pull look toward center: {} vs {}", new_dist, old_dist);
    }

    #[test]
    fn zoom_clamped_at_min() {
        let cam_pos = Vec3::new(0.0, 55.0, 30.0); // near min
        let look = Vec3::ZERO;
        let result = compute_zoom(cam_pos, look, 1.0, 50.0, 1500.0);
        if let Some((new_pos, _)) = result {
            assert!(new_pos.y >= 50.0, "should not go below min zoom: {}", new_pos.y);
        }
    }

    #[test]
    fn zoom_clamped_at_max() {
        let cam_pos = Vec3::new(0.0, 1400.0, 200.0); // near max
        let look = Vec3::ZERO;
        let result = compute_zoom(cam_pos, look, -1.0, 50.0, 1500.0);
        if let Some((new_pos, _)) = result {
            assert!(new_pos.y <= 1500.0, "should not exceed max zoom: {}", new_pos.y);
        }
    }

    #[test]
    fn zoom_in_moves_xz_toward_look() {
        let cam_pos = Vec3::new(100.0, 400.0, 300.0);
        let look = Vec3::new(50.0, 0.0, 100.0);
        let result = compute_zoom(cam_pos, look, 1.0, 50.0, 1500.0);
        let (new_pos, _) = result.unwrap();
        // Camera XZ should be closer to look-at point
        let old_dist = Vec2::new(cam_pos.x - look.x, cam_pos.z - look.z).length();
        let new_dist = Vec2::new(new_pos.x - look.x, new_pos.z - look.z).length();
        assert!(new_dist < old_dist, "zoom in should move XZ toward look: {} vs {}", new_dist, old_dist);
    }

    #[test]
    fn repeated_zoom_in_out_returns_near_original() {
        let cam_pos = Vec3::new(0.0, 400.0, 200.0);
        let look = Vec3::ZERO;

        // Zoom in
        let (pos1, look1) = compute_zoom(cam_pos, look, 1.0, 50.0, 1500.0).unwrap();
        // Zoom out from new position
        let (pos2, _) = compute_zoom(pos1, look1, -1.0, 50.0, 1500.0).unwrap();

        // Should be roughly back near original (not exact due to center blend on zoom out)
        assert!((pos2.y - cam_pos.y).abs() < 20.0,
            "zoom in+out should return near original height: {} vs {}", pos2.y, cam_pos.y);
    }
}
