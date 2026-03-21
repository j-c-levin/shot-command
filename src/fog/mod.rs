use bevy::prelude::*;

use crate::game::{Detected, EnemyVisibility, GameState, Team};
use crate::map::{Asteroid, AsteroidSize};
use crate::net::LocalTeam;
use crate::ship::{Ship, ShipClass, ship_xz_position};

pub struct FogPlugin;

impl Plugin for FogPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (detect_enemies, fade_enemies)
                .chain()
                .run_if(in_state(GameState::Playing)),
        );
    }
}

/// Marker for a visual-only "ghost" entity that fades out after a replicated
/// enemy ship is despawned by bevy_replicon. Client-only, no serde needed.
#[derive(Component)]
pub struct FadeOutGhost;

pub struct FogClientPlugin;

impl Plugin for FogClientPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(on_enemy_ship_removed);
        app.add_systems(
            Update,
            fade_out_ghosts.run_if(in_state(GameState::Playing)),
        );
    }
}

/// Observer that fires when a Ship component is removed from an entity.
/// When replicon despawns an enemy ship, Bevy fires removal observers BEFORE the
/// entity is fully gone, so we can still read its components. We spawn a visual-only
/// "ghost" entity at the same position that fades out over FADE_DURATION.
fn on_enemy_ship_removed(
    trigger: On<Remove, Ship>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    query: Query<(&Transform, &ShipClass, &Team)>,
    local_team: Res<LocalTeam>,
) {
    let entity = trigger.entity;
    let Ok((transform, class, team)) = query.get(entity) else {
        return;
    };

    // Only create ghosts for enemy ships
    let Some(my_team) = local_team.0 else {
        return;
    };
    if *team == my_team {
        return;
    }

    let color = Color::srgb(1.0, 0.2, 0.2);

    let (ship_mesh, mesh_transform) = class.create_mesh(&mut meshes);

    let material = materials.add(StandardMaterial {
        base_color: color.with_alpha(1.0),
        emissive: color.into(),
        alpha_mode: AlphaMode::Blend,
        ..default()
    });

    commands
        .spawn((
            FadeOutGhost,
            EnemyVisibility { opacity: 1.0 },
            *transform,
            Visibility::Visible,
        ))
        .with_child((Mesh3d(ship_mesh), MeshMaterial3d(material), mesh_transform));
}

/// Fades ghost entities toward opacity 0.0 and despawns them when done.
fn fade_out_ghosts(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<
        (Entity, &mut EnemyVisibility, &Children),
        With<FadeOutGhost>,
    >,
    child_query: Query<&MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (entity, mut ev, children) in &mut query {
        ev.opacity = fade_opacity(ev.opacity, 0.0, time.delta_secs(), FADE_DURATION);

        if ev.opacity > 0.001 {
            for child in children.iter() {
                if let Ok(mat_handle) = child_query.get(child) {
                    if let Some(material) = materials.get_mut(mat_handle) {
                        material.base_color = material.base_color.with_alpha(ev.opacity);
                    }
                }
            }
        } else {
            crate::game::try_despawn(&mut commands, entity);
        }
    }
}

const FADE_DURATION: f32 = 0.5;

/// Pure function: checks if `target` is within `vision_range` of `observer`
/// and not blocked by any asteroid.
pub fn is_in_los(observer: Vec2, target: Vec2, vision_range: f32, asteroids: &[(Vec2, f32)]) -> bool {
    let dist = (target - observer).length();
    if dist > vision_range {
        return false;
    }
    !ray_blocked_by_asteroid(observer, target, asteroids)
}

/// Pure function: moves opacity toward target at constant rate over fade_duration.
pub fn fade_opacity(current: f32, target: f32, dt: f32, fade_duration: f32) -> f32 {
    if fade_duration <= 0.0 {
        return target;
    }
    let rate = 1.0 / fade_duration;
    if current < target {
        (current + rate * dt).min(target)
    } else {
        (current - rate * dt).max(target)
    }
}

/// Checks if a ray from `start` to `end` (both in world XZ space) is blocked by any asteroid.
pub fn ray_blocked_by_asteroid(
    start: Vec2,
    end: Vec2,
    asteroids: &[(Vec2, f32)],
) -> bool {
    let ray_dir = end - start;
    let ray_len = ray_dir.length();
    if ray_len < 0.001 {
        return false;
    }
    let ray_norm = ray_dir / ray_len;

    for &(center, radius) in asteroids {
        let to_center = center - start;
        let proj = to_center.dot(ray_norm);

        if proj < 0.0 || proj > ray_len {
            continue;
        }

        let closest = start + ray_norm * proj;
        let dist_sq = (closest - center).length_squared();

        if dist_sq < radius * radius {
            return true;
        }
    }
    false
}

fn detect_enemies(
    mut commands: Commands,
    local_team: Res<LocalTeam>,
    player_ships: Query<(&Transform, &ShipClass, &Team), With<Ship>>,
    enemy_query: Query<(Entity, &Transform, &Team, Option<&Detected>), With<Ship>>,
    asteroid_query: Query<(&Transform, &AsteroidSize), With<Asteroid>>,
) {
    let my_team = match local_team.0 {
        Some(t) => t,
        None => return,
    };

    let asteroids: Vec<(Vec2, f32)> = asteroid_query
        .iter()
        .map(|(t, s)| (Vec2::new(t.translation.x, t.translation.z), s.radius))
        .collect();

    for (enemy_entity, enemy_transform, enemy_team, maybe_detected) in &enemy_query {
        if *enemy_team == my_team {
            continue;
        }

        let enemy_pos = ship_xz_position(enemy_transform);
        let mut seen = false;

        for (player_transform, class, player_team) in &player_ships {
            if *player_team != my_team {
                continue;
            }
            let player_pos = ship_xz_position(player_transform);
            if is_in_los(player_pos, enemy_pos, class.profile().vision_range, &asteroids) {
                seen = true;
                break;
            }
        }

        // Only insert/remove when state changes to avoid unnecessary commands
        if seen && maybe_detected.is_none() {
            commands.entity(enemy_entity).insert(Detected);
        } else if !seen && maybe_detected.is_some() {
            commands.entity(enemy_entity).remove::<Detected>();
        }
    }
}

fn fade_enemies(
    time: Res<Time>,
    mut query: Query<
        (&mut EnemyVisibility, &mut Visibility, &Children, Option<&Detected>),
        With<Ship>,
    >,
    child_query: Query<&MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (mut ev, mut visibility, children, detected) in &mut query {
        let target = if detected.is_some() { 1.0 } else { 0.0 };
        ev.opacity = fade_opacity(ev.opacity, target, time.delta_secs(), FADE_DURATION);

        if ev.opacity > 0.0 {
            *visibility = Visibility::Visible;
            // Update material alpha on the child mesh entity
            for child in children.iter() {
                if let Ok(mat_handle) = child_query.get(child) {
                    if let Some(material) = materials.get_mut(mat_handle) {
                        material.base_color = material.base_color.with_alpha(ev.opacity);
                    }
                }
            }
        } else {
            *visibility = Visibility::Hidden;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ray_not_blocked_without_obstacles() {
        let asteroids: Vec<(Vec2, f32)> = vec![];
        assert!(!ray_blocked_by_asteroid(Vec2::ZERO, Vec2::new(100.0, 0.0), &asteroids));
    }

    #[test]
    fn ray_blocked_by_asteroid_in_path() {
        let asteroids = vec![(Vec2::new(50.0, 0.0), 10.0)];
        assert!(ray_blocked_by_asteroid(Vec2::ZERO, Vec2::new(100.0, 0.0), &asteroids));
    }

    #[test]
    fn ray_not_blocked_by_asteroid_off_path() {
        let asteroids = vec![(Vec2::new(50.0, 50.0), 10.0)];
        assert!(!ray_blocked_by_asteroid(Vec2::ZERO, Vec2::new(100.0, 0.0), &asteroids));
    }

    #[test]
    fn ray_not_blocked_by_asteroid_behind_start() {
        let asteroids = vec![(Vec2::new(-50.0, 0.0), 10.0)];
        assert!(!ray_blocked_by_asteroid(Vec2::ZERO, Vec2::new(100.0, 0.0), &asteroids));
    }

    #[test]
    fn in_range_and_unblocked_is_detected() {
        assert!(is_in_los(Vec2::ZERO, Vec2::new(100.0, 0.0), 200.0, &[]));
    }

    #[test]
    fn out_of_range_is_not_detected() {
        assert!(!is_in_los(Vec2::ZERO, Vec2::new(300.0, 0.0), 200.0, &[]));
    }

    #[test]
    fn blocked_by_asteroid_is_not_detected() {
        let asteroids = vec![(Vec2::new(50.0, 0.0), 10.0)];
        assert!(!is_in_los(Vec2::ZERO, Vec2::new(100.0, 0.0), 200.0, &asteroids));
    }

    #[test]
    fn same_position_is_in_los() {
        assert!(is_in_los(Vec2::ZERO, Vec2::ZERO, 200.0, &[]));
    }

    #[test]
    fn opacity_fades_toward_target() {
        let result = fade_opacity(0.0, 1.0, 0.25, 0.5);
        assert!((result - 0.5).abs() < 0.01);
    }

    #[test]
    fn opacity_clamps_at_target() {
        let result = fade_opacity(0.0, 1.0, 10.0, 0.5);
        assert_eq!(result, 1.0);
    }

    #[test]
    fn opacity_fades_out() {
        let result = fade_opacity(1.0, 0.0, 0.5, 0.5);
        assert_eq!(result, 0.0);
    }
}
