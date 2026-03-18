pub mod data;

use bevy::prelude::*;
use rand::Rng;
use serde::{Deserialize, Serialize};

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(MapBounds {
            half_extents: Vec2::splat(500.0),
        })
        .add_systems(
            Startup,
            (spawn_ground_plane, spawn_asteroids, spawn_boundary_markers),
        );
    }
}

#[derive(Resource, Clone, Debug)]
pub struct MapBounds {
    pub half_extents: Vec2,
}

impl MapBounds {
    pub fn contains(&self, pos: Vec2) -> bool {
        pos.x.abs() <= self.half_extents.x && pos.y.abs() <= self.half_extents.y
    }

    pub fn clamp(&self, pos: Vec2) -> Vec2 {
        Vec2::new(
            pos.x.clamp(-self.half_extents.x, self.half_extents.x),
            pos.y.clamp(-self.half_extents.y, self.half_extents.y),
        )
    }

    pub fn size(&self) -> Vec2 {
        self.half_extents * 2.0
    }
}

#[derive(Component, Serialize, Deserialize)]
pub struct Asteroid;

#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct AsteroidSize {
    pub radius: f32,
}

#[derive(Component)]
pub struct GroundPlane;

fn spawn_ground_plane(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    bounds: Res<MapBounds>,
) {
    let size = bounds.size();
    commands.spawn((
        GroundPlane,
        Mesh3d(meshes.add(Plane3d::new(Vec3::Y, Vec2::new(size.x / 2.0, size.y / 2.0)))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.02, 0.02, 0.05),
            perceptual_roughness: 1.0,
            ..default()
        })),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));
}

fn spawn_asteroids(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    bounds: Res<MapBounds>,
) {
    let mut rng = rand::rng();
    let asteroid_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.3, 0.25, 0.2),
        perceptual_roughness: 0.9,
        ..default()
    });

    let asteroid_count = 12;
    let min_distance_from_edge = 50.0;
    let min_distance_from_center = 100.0;

    for _ in 0..asteroid_count {
        let radius = rng.random_range(15.0..40.0);

        let pos = loop {
            let candidate = Vec2::new(
                rng.random_range(
                    (-bounds.half_extents.x + min_distance_from_edge)
                        ..(bounds.half_extents.x - min_distance_from_edge),
                ),
                rng.random_range(
                    (-bounds.half_extents.y + min_distance_from_edge)
                        ..(bounds.half_extents.y - min_distance_from_edge),
                ),
            );
            if candidate.length() > min_distance_from_center {
                break candidate;
            }
        };

        commands.spawn((
            Asteroid,
            AsteroidSize { radius },
            Mesh3d(meshes.add(Sphere::new(radius))),
            MeshMaterial3d(asteroid_material.clone()),
            Transform::from_xyz(pos.x, 0.0, pos.y),
        ));
    }
}

fn spawn_boundary_markers(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    bounds: Res<MapBounds>,
) {
    let marker_material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.3, 0.3, 0.8, 0.3),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });

    let hx = bounds.half_extents.x;
    let hy = bounds.half_extents.y;
    let wall_height = 20.0;
    let wall_thickness = 2.0;

    let walls = [
        (
            Vec3::new(0.0, wall_height / 2.0, hy),
            Vec3::new(hx * 2.0, wall_height, wall_thickness),
        ),
        (
            Vec3::new(0.0, wall_height / 2.0, -hy),
            Vec3::new(hx * 2.0, wall_height, wall_thickness),
        ),
        (
            Vec3::new(hx, wall_height / 2.0, 0.0),
            Vec3::new(wall_thickness, wall_height, hy * 2.0),
        ),
        (
            Vec3::new(-hx, wall_height / 2.0, 0.0),
            Vec3::new(wall_thickness, wall_height, hy * 2.0),
        ),
    ];

    for (pos, size) in walls {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(size.x, size.y, size.z))),
            MeshMaterial3d(marker_material.clone()),
            Transform::from_translation(pos),
            Pickable::IGNORE,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_contains_origin() {
        let bounds = MapBounds {
            half_extents: Vec2::splat(500.0),
        };
        assert!(bounds.contains(Vec2::ZERO));
    }

    #[test]
    fn bounds_contains_edge() {
        let bounds = MapBounds {
            half_extents: Vec2::splat(500.0),
        };
        assert!(bounds.contains(Vec2::new(500.0, 500.0)));
        assert!(bounds.contains(Vec2::new(-500.0, -500.0)));
    }

    #[test]
    fn bounds_rejects_outside() {
        let bounds = MapBounds {
            half_extents: Vec2::splat(500.0),
        };
        assert!(!bounds.contains(Vec2::new(501.0, 0.0)));
        assert!(!bounds.contains(Vec2::new(0.0, -501.0)));
    }

    #[test]
    fn bounds_clamp_inside_unchanged() {
        let bounds = MapBounds {
            half_extents: Vec2::splat(500.0),
        };
        let pos = Vec2::new(100.0, -200.0);
        assert_eq!(bounds.clamp(pos), pos);
    }

    #[test]
    fn bounds_clamp_outside() {
        let bounds = MapBounds {
            half_extents: Vec2::splat(500.0),
        };
        assert_eq!(
            bounds.clamp(Vec2::new(600.0, -700.0)),
            Vec2::new(500.0, -500.0)
        );
    }

    #[test]
    fn bounds_size() {
        let bounds = MapBounds {
            half_extents: Vec2::new(500.0, 300.0),
        };
        assert_eq!(bounds.size(), Vec2::new(1000.0, 600.0));
    }
}
