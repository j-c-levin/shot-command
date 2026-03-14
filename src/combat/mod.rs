use bevy::prelude::*;

use crate::game::{Detected, GameState, Health, Team};
use crate::map::MapBounds;
use crate::ship::{Ship, ship_xz_position};

pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Startup,
            init_projectile_assets,
        )
        .add_systems(
            Update,
            (auto_target, move_projectiles, check_projectile_hits)
                .chain()
                .run_if(in_state(GameState::Playing)),
        );
    }
}

const PROJECTILE_SPEED: f32 = 400.0;
const PROJECTILE_HIT_RADIUS: f32 = 10.0;

#[derive(Resource)]
struct ProjectileAssets {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
}

fn init_projectile_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(ProjectileAssets {
        mesh: meshes.add(Sphere::new(2.0)),
        material: materials.add(StandardMaterial {
            base_color: Color::srgb(0.3, 0.6, 1.0),
            emissive: LinearRgba::new(1.5, 3.0, 5.0, 1.0),
            unlit: true,
            ..default()
        }),
    });
}

#[derive(Component)]
pub struct FireRate {
    pub timer: Timer,
}

impl Default for FireRate {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(0.5, TimerMode::Repeating),
        }
    }
}

#[derive(Component)]
pub struct Projectile {
    pub direction: Vec3,
    pub speed: f32,
    pub damage: u8,
}

/// Pure function: direction from shooter to target, normalized.
/// Returns Vec3::ZERO if from == to.
pub fn projectile_direction(from: Vec3, to: Vec3) -> Vec3 {
    (to - from).normalize_or_zero()
}

/// Pure function: hit test — is projectile within radius of target?
pub fn is_hit(projectile_pos: Vec3, target_pos: Vec3, radius: f32) -> bool {
    (projectile_pos - target_pos).length_squared() < radius * radius
}

fn auto_target(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<ProjectileAssets>,
    mut player_query: Query<(&Transform, &mut FireRate, &Team), With<Ship>>,
    enemy_query: Query<(&Transform, &Team), (With<Ship>, With<Detected>)>,
) {
    let has_enemies = !enemy_query.is_empty();

    for (player_transform, mut fire_rate, player_team) in &mut player_query {
        if *player_team != Team::PLAYER {
            continue;
        }

        // Only tick timer when enemies are detected, reset otherwise
        if !has_enemies {
            fire_rate.timer.reset();
            continue;
        }

        fire_rate.timer.tick(time.delta());

        if !fire_rate.timer.just_finished() {
            continue;
        }

        // Find closest detected enemy
        let player_pos = ship_xz_position(player_transform);
        let mut closest: Option<(f32, Vec3)> = None;

        for (enemy_transform, enemy_team) in &enemy_query {
            if *enemy_team == Team::PLAYER {
                continue;
            }
            let enemy_pos = ship_xz_position(enemy_transform);
            let dist = (enemy_pos - player_pos).length();
            match closest {
                None => closest = Some((dist, enemy_transform.translation)),
                Some((best, _)) if dist < best => {
                    closest = Some((dist, enemy_transform.translation));
                }
                _ => {}
            }
        }

        let Some((_, target_pos)) = closest else {
            continue;
        };

        let spawn_pos = player_transform.translation;
        let direction = projectile_direction(spawn_pos, target_pos);

        if direction == Vec3::ZERO {
            continue;
        }

        commands.spawn((
            Projectile {
                direction,
                speed: PROJECTILE_SPEED,
                damage: 1,
            },
            Mesh3d(assets.mesh.clone()),
            MeshMaterial3d(assets.material.clone()),
            Transform::from_translation(spawn_pos),
        ));
    }
}

fn move_projectiles(
    mut commands: Commands,
    time: Res<Time>,
    bounds: Res<MapBounds>,
    mut query: Query<(Entity, &mut Transform, &Projectile)>,
) {
    for (entity, mut transform, projectile) in &mut query {
        let movement = projectile.direction * projectile.speed * time.delta_secs();
        transform.translation += movement;

        let pos = Vec2::new(transform.translation.x, transform.translation.z);
        if !bounds.contains(pos) {
            commands.entity(entity).despawn();
        }
    }
}

fn check_projectile_hits(
    mut commands: Commands,
    projectile_query: Query<(Entity, &Transform, &Projectile)>,
    mut enemy_query: Query<(Entity, &Transform, &mut Health, &Team), With<Ship>>,
) {
    // Track already-destroyed enemies to avoid double despawn
    let mut destroyed: Vec<Entity> = Vec::new();

    for (proj_entity, proj_transform, projectile) in &projectile_query {
        for (enemy_entity, enemy_transform, mut health, team) in &mut enemy_query {
            if *team == Team::PLAYER || destroyed.contains(&enemy_entity) {
                continue;
            }

            if is_hit(proj_transform.translation, enemy_transform.translation, PROJECTILE_HIT_RADIUS) {
                commands.entity(proj_entity).despawn();
                health.hp = health.hp.saturating_sub(projectile.damage);
                if health.hp == 0 {
                    commands.entity(enemy_entity).despawn();
                    destroyed.push(enemy_entity);
                }
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projectile_direction_is_normalized() {
        let dir = projectile_direction(Vec3::ZERO, Vec3::new(100.0, 0.0, 100.0));
        assert!((dir.length() - 1.0).abs() < 0.001);
    }

    #[test]
    fn hit_detection_within_radius() {
        assert!(is_hit(Vec3::ZERO, Vec3::new(5.0, 0.0, 0.0), 10.0));
    }

    #[test]
    fn hit_detection_outside_radius() {
        assert!(!is_hit(Vec3::ZERO, Vec3::new(15.0, 0.0, 0.0), 10.0));
    }

    #[test]
    fn hit_detection_at_exact_radius() {
        // At exactly the radius boundary — should NOT hit (strict less-than)
        assert!(!is_hit(Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0), 10.0));
    }
}
