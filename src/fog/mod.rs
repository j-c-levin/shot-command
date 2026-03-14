use bevy::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::game::{GameState, Revealed, Team};
use crate::map::{Asteroid, AsteroidSize, MapBounds};
use crate::ship::{Ship, ShipStats, ship_xz_position};

pub struct FogPlugin;

impl Plugin for FogPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, init_fog)
            .add_systems(
                Update,
                (
                    update_fog_grid,
                    sync_entity_visibility,
                    update_ship_rendering,
                    update_fog_overlay,
                )
                    .chain()
                    .run_if(in_state(GameState::Playing)),
            );
    }
}

const GRID_RESOLUTION: usize = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellVisibility {
    Hidden,
    Explored,
    Visible,
}

#[derive(Resource)]
pub struct VisibilityGrid {
    pub cells: Vec<Vec<CellVisibility>>,
    pub cell_size: Vec2,
    pub grid_size: usize,
    pub origin: Vec2,
}

impl VisibilityGrid {
    pub fn new(bounds: &MapBounds) -> Self {
        let grid_size = GRID_RESOLUTION;
        let cell_size = bounds.size() / grid_size as f32;
        let origin = -bounds.half_extents;

        Self {
            cells: vec![vec![CellVisibility::Hidden; grid_size]; grid_size],
            cell_size,
            grid_size,
            origin,
        }
    }

    pub fn world_to_grid(&self, world_pos: Vec2) -> Option<(usize, usize)> {
        let local = world_pos - self.origin;
        let gx = (local.x / self.cell_size.x) as i32;
        let gy = (local.y / self.cell_size.y) as i32;

        if gx >= 0 && gy >= 0 && (gx as usize) < self.grid_size && (gy as usize) < self.grid_size
        {
            Some((gx as usize, gy as usize))
        } else {
            None
        }
    }

    pub fn grid_to_world(&self, gx: usize, gy: usize) -> Vec2 {
        self.origin
            + Vec2::new(
                (gx as f32 + 0.5) * self.cell_size.x,
                (gy as f32 + 0.5) * self.cell_size.y,
            )
    }

    pub fn clear_visible(&mut self) {
        for row in &mut self.cells {
            for cell in row.iter_mut() {
                if *cell == CellVisibility::Visible {
                    *cell = CellVisibility::Explored;
                }
            }
        }
    }

    pub fn mark_visible(&mut self, gx: usize, gy: usize) {
        if gx < self.grid_size && gy < self.grid_size {
            self.cells[gx][gy] = CellVisibility::Visible;
        }
    }
}

#[derive(Component)]
struct FogOverlay;

#[derive(Resource)]
struct FogTextureHandle(Handle<Image>);

fn init_fog(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    bounds: Res<MapBounds>,
) {
    let grid = VisibilityGrid::new(&bounds);

    let size = Extent3d {
        width: GRID_RESOLUTION as u32,
        height: GRID_RESOLUTION as u32,
        depth_or_array_layers: 1,
    };
    let mut image = Image::new_fill(size, TextureDimension::D2, &[0, 0, 0, 200], TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD);
    image.sampler = bevy::image::ImageSampler::nearest();
    let image_handle = images.add(image);

    let fog_material = materials.add(StandardMaterial {
        base_color_texture: Some(image_handle.clone()),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });

    let map_size = bounds.size();
    commands.spawn((
        FogOverlay,
        Mesh3d(meshes.add(Plane3d::new(Vec3::Y, Vec2::new(map_size.x / 2.0, map_size.y / 2.0)))),
        MeshMaterial3d(fog_material),
        Transform::from_xyz(0.0, 15.0, 0.0),
        Pickable::IGNORE,
    ));

    commands.insert_resource(FogTextureHandle(image_handle));
    commands.insert_resource(grid);
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

fn update_fog_grid(
    mut grid: ResMut<VisibilityGrid>,
    player_ships: Query<(&Transform, &ShipStats, &Team), With<Ship>>,
    asteroid_query: Query<(&Transform, &AsteroidSize), With<Asteroid>>,
) {
    grid.clear_visible();

    let asteroids: Vec<(Vec2, f32)> = asteroid_query
        .iter()
        .map(|(t, s)| (Vec2::new(t.translation.x, t.translation.z), s.radius))
        .collect();

    for (transform, stats, team) in &player_ships {
        if *team != Team::PLAYER {
            continue;
        }

        let ship_pos = ship_xz_position(transform);
        let vision = stats.vision_range;

        for gx in 0..grid.grid_size {
            for gy in 0..grid.grid_size {
                let cell_pos = grid.grid_to_world(gx, gy);
                let dist = (cell_pos - ship_pos).length();

                if dist > vision {
                    continue;
                }

                if !ray_blocked_by_asteroid(ship_pos, cell_pos, &asteroids) {
                    grid.mark_visible(gx, gy);
                }
            }
        }
    }
}

fn sync_entity_visibility(
    mut commands: Commands,
    grid: Res<VisibilityGrid>,
    enemy_query: Query<(Entity, &Transform, &Team), (With<Ship>, Without<Revealed>)>,
    revealed_query: Query<(Entity, &Transform, &Team), (With<Ship>, With<Revealed>)>,
) {
    // Add Revealed to enemies in visible cells
    for (entity, transform, team) in &enemy_query {
        if *team == Team::PLAYER {
            continue;
        }
        let pos = ship_xz_position(transform);
        if let Some((gx, gy)) = grid.world_to_grid(pos) {
            if grid.cells[gx][gy] == CellVisibility::Visible {
                commands.entity(entity).insert(Revealed);
            }
        }
    }

    // Remove Revealed from enemies no longer in visible cells
    for (entity, transform, team) in &revealed_query {
        if *team == Team::PLAYER {
            continue;
        }
        let pos = ship_xz_position(transform);
        if let Some((gx, gy)) = grid.world_to_grid(pos) {
            if grid.cells[gx][gy] != CellVisibility::Visible {
                commands.entity(entity).remove::<Revealed>();
            }
        }
    }
}

fn update_ship_rendering(
    mut query: Query<(Entity, &Team, &mut Visibility), With<Ship>>,
    revealed_query: Query<(), With<Revealed>>,
) {
    for (entity, team, mut visibility) in &mut query {
        if *team == Team::PLAYER {
            continue;
        }
        if revealed_query.get(entity).is_ok() {
            *visibility = Visibility::Visible;
        } else {
            *visibility = Visibility::Hidden;
        }
    }
}

fn update_fog_overlay(
    grid: Res<VisibilityGrid>,
    fog_handle: Res<FogTextureHandle>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(image) = images.get_mut(&fog_handle.0) else {
        return;
    };

    let Some(data) = &mut image.data else {
        return;
    };

    for gy in 0..grid.grid_size {
        for gx in 0..grid.grid_size {
            let idx = (gy * grid.grid_size + gx) * 4;
            let alpha = match grid.cells[gx][gy] {
                CellVisibility::Hidden => 200,
                CellVisibility::Explored => 140,
                CellVisibility::Visible => 0,
            };
            data[idx] = 0;
            data[idx + 1] = 0;
            data[idx + 2] = 0;
            data[idx + 3] = alpha;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_world_roundtrip() {
        let bounds = MapBounds {
            half_extents: Vec2::splat(500.0),
        };
        let grid = VisibilityGrid::new(&bounds);

        // Center of grid should map to world origin area
        let center = grid.grid_to_world(50, 50);
        assert!(center.x.abs() < grid.cell_size.x);
        assert!(center.y.abs() < grid.cell_size.y);
    }

    #[test]
    fn grid_world_to_grid_in_bounds() {
        let bounds = MapBounds {
            half_extents: Vec2::splat(500.0),
        };
        let grid = VisibilityGrid::new(&bounds);

        assert!(grid.world_to_grid(Vec2::ZERO).is_some());
        assert!(grid.world_to_grid(Vec2::new(499.0, 499.0)).is_some());
    }

    #[test]
    fn grid_world_to_grid_out_of_bounds() {
        let bounds = MapBounds {
            half_extents: Vec2::splat(500.0),
        };
        let grid = VisibilityGrid::new(&bounds);

        assert!(grid.world_to_grid(Vec2::new(-600.0, 0.0)).is_none());
    }

    #[test]
    fn clear_visible_becomes_explored() {
        let bounds = MapBounds {
            half_extents: Vec2::splat(500.0),
        };
        let mut grid = VisibilityGrid::new(&bounds);

        grid.mark_visible(10, 10);
        assert_eq!(grid.cells[10][10], CellVisibility::Visible);

        grid.clear_visible();
        assert_eq!(grid.cells[10][10], CellVisibility::Explored);
    }

    #[test]
    fn hidden_stays_hidden_after_clear() {
        let bounds = MapBounds {
            half_extents: Vec2::splat(500.0),
        };
        let mut grid = VisibilityGrid::new(&bounds);

        assert_eq!(grid.cells[5][5], CellVisibility::Hidden);
        grid.clear_visible();
        assert_eq!(grid.cells[5][5], CellVisibility::Hidden);
    }

    #[test]
    fn ray_not_blocked_without_obstacles() {
        let asteroids: Vec<(Vec2, f32)> = vec![];
        assert!(!ray_blocked_by_asteroid(
            Vec2::ZERO,
            Vec2::new(100.0, 0.0),
            &asteroids
        ));
    }

    #[test]
    fn ray_blocked_by_asteroid_in_path() {
        let asteroids = vec![(Vec2::new(50.0, 0.0), 10.0)];
        assert!(ray_blocked_by_asteroid(
            Vec2::ZERO,
            Vec2::new(100.0, 0.0),
            &asteroids
        ));
    }

    #[test]
    fn ray_not_blocked_by_asteroid_off_path() {
        let asteroids = vec![(Vec2::new(50.0, 50.0), 10.0)];
        assert!(!ray_blocked_by_asteroid(
            Vec2::ZERO,
            Vec2::new(100.0, 0.0),
            &asteroids
        ));
    }

    #[test]
    fn ray_not_blocked_by_asteroid_behind_start() {
        let asteroids = vec![(Vec2::new(-50.0, 0.0), 10.0)];
        assert!(!ray_blocked_by_asteroid(
            Vec2::ZERO,
            Vec2::new(100.0, 0.0),
            &asteroids
        ));
    }
}
