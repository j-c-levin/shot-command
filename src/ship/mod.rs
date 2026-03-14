use bevy::prelude::*;

use crate::game::{EnemyVisibility, Health, Team};
use crate::map::{Asteroid, AsteroidSize, MapBounds};

pub struct ShipPlugin;

impl Plugin for ShipPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (move_ships, clamp_ships_to_bounds).chain());
    }
}

#[derive(Component)]
pub struct Ship;

#[derive(Component, Clone, Debug)]
pub struct ShipStats {
    pub speed: f32,
    pub vision_range: f32,
    pub collision_radius: f32,
}

impl Default for ShipStats {
    fn default() -> Self {
        Self {
            speed: 80.0,
            vision_range: 200.0,
            collision_radius: 8.0,
        }
    }
}

#[derive(Component, Clone, Debug)]
pub struct MovementTarget {
    pub destination: Vec2,
}

#[derive(Component)]
pub struct Selected;

#[derive(Component)]
pub struct SelectionIndicator;

pub fn ship_xz_position(transform: &Transform) -> Vec2 {
    Vec2::new(transform.translation.x, transform.translation.z)
}

pub fn movement_direction(from: Vec2, to: Vec2) -> Option<Vec2> {
    let diff = to - from;
    let len = diff.length();
    if len < 1.0 {
        None
    } else {
        Some(diff / len)
    }
}

pub fn has_arrived(from: Vec2, to: Vec2, threshold: f32) -> bool {
    (to - from).length_squared() < threshold * threshold
}

/// Returns true if `pos` overlaps any asteroid (circle-circle test in XZ).
pub fn collides_with_asteroid(pos: Vec2, ship_radius: f32, asteroids: &[(Vec2, f32)]) -> bool {
    for &(center, radius) in asteroids {
        let min_dist = ship_radius + radius;
        if (pos - center).length_squared() < min_dist * min_dist {
            return true;
        }
    }
    false
}

fn move_ships(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut Transform, &ShipStats, &MovementTarget), (With<Ship>, Without<Asteroid>)>,
    asteroid_query: Query<(&Transform, &AsteroidSize), With<Asteroid>>,
) {
    let asteroids: Vec<(Vec2, f32)> = asteroid_query
        .iter()
        .map(|(t, s)| (Vec2::new(t.translation.x, t.translation.z), s.radius))
        .collect();

    for (entity, mut transform, stats, target) in &mut query {
        let current = ship_xz_position(&transform);
        let arrival_threshold = 5.0;

        if has_arrived(current, target.destination, arrival_threshold) {
            commands.entity(entity).remove::<MovementTarget>();
            continue;
        }

        if let Some(dir) = movement_direction(current, target.destination) {
            let movement = dir * stats.speed * time.delta_secs();
            let new_pos = current + movement;

            // Stop and cancel move if we'd collide with an asteroid
            if collides_with_asteroid(new_pos, stats.collision_radius, &asteroids) {
                commands.entity(entity).remove::<MovementTarget>();
                continue;
            }

            transform.translation.x = new_pos.x;
            transform.translation.z = new_pos.y;

            let look_target = Vec3::new(
                transform.translation.x + dir.x,
                transform.translation.y,
                transform.translation.z + dir.y,
            );
            transform.look_at(look_target, Vec3::Y);
        }
    }
}

fn clamp_ships_to_bounds(bounds: Res<MapBounds>, mut query: Query<&mut Transform, With<Ship>>) {
    for mut transform in &mut query {
        let pos = ship_xz_position(&transform);
        let clamped = bounds.clamp(pos);
        transform.translation.x = clamped.x;
        transform.translation.z = clamped.y;
    }
}

pub fn spawn_ship(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    position: Vec2,
    team: Team,
    color: Color,
) -> Entity {
    let ship_mesh = meshes.add(Cone {
        radius: 8.0,
        height: 20.0,
    });

    let is_enemy = team != Team::PLAYER;
    let ship_material = materials.add(StandardMaterial {
        base_color: if is_enemy { color.with_alpha(0.0) } else { color },
        emissive: color.into(),
        alpha_mode: if is_enemy { AlphaMode::Blend } else { AlphaMode::Opaque },
        ..default()
    });

    let initial_visibility = if is_enemy {
        Visibility::Hidden
    } else {
        Visibility::Visible
    };

    // Parent entity: Ship logic, transform, facing direction via look_at
    // Child entity: Cone mesh with fixed rotation (tip points -Z = forward)
    let mut entity_commands = commands.spawn((
        Ship,
        team,
        ShipStats::default(),
        Transform::from_xyz(position.x, 5.0, position.y),
        initial_visibility,
    ));

    // Cone tip is +Y by default; rotate PI/2 around X to point tip along -Z (forward)
    entity_commands.with_child((
        Mesh3d(ship_mesh),
        MeshMaterial3d(ship_material),
        Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
    ));

    if is_enemy {
        entity_commands.insert((EnemyVisibility::default(), Health { hp: 3 }));
    }

    entity_commands.id()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ship_xz_extracts_correctly() {
        let transform = Transform::from_xyz(10.0, 5.0, -20.0);
        assert_eq!(ship_xz_position(&transform), Vec2::new(10.0, -20.0));
    }

    #[test]
    fn movement_direction_normalizes() {
        let dir = movement_direction(Vec2::ZERO, Vec2::new(100.0, 0.0)).unwrap();
        assert!((dir.length() - 1.0).abs() < 0.001);
        assert!((dir.x - 1.0).abs() < 0.001);
    }

    #[test]
    fn movement_direction_diagonal() {
        let dir = movement_direction(Vec2::ZERO, Vec2::new(50.0, 50.0)).unwrap();
        assert!((dir.length() - 1.0).abs() < 0.001);
    }

    #[test]
    fn movement_direction_none_when_close() {
        assert!(movement_direction(Vec2::ZERO, Vec2::new(0.5, 0.3)).is_none());
    }

    #[test]
    fn arrival_at_target() {
        assert!(has_arrived(Vec2::splat(100.0), Vec2::splat(100.0), 5.0));
    }

    #[test]
    fn arrival_within_threshold() {
        assert!(has_arrived(
            Vec2::new(100.0, 100.0),
            Vec2::new(103.0, 100.0),
            5.0
        ));
    }

    #[test]
    fn arrival_outside_threshold() {
        assert!(!has_arrived(
            Vec2::new(100.0, 100.0),
            Vec2::new(200.0, 100.0),
            5.0
        ));
    }

    #[test]
    fn collision_with_overlapping_asteroid() {
        let asteroids = vec![(Vec2::new(50.0, 0.0), 20.0)];
        assert!(collides_with_asteroid(Vec2::new(40.0, 0.0), 8.0, &asteroids));
    }

    #[test]
    fn no_collision_when_clear() {
        let asteroids = vec![(Vec2::new(50.0, 0.0), 20.0)];
        assert!(!collides_with_asteroid(Vec2::ZERO, 8.0, &asteroids));
    }

    #[test]
    fn no_collision_with_no_asteroids() {
        assert!(!collides_with_asteroid(Vec2::ZERO, 8.0, &[]));
    }
}
