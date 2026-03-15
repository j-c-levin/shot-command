use bevy::prelude::*;

use crate::game::Team;
use crate::input::on_ship_clicked;
use crate::map::{Asteroid, AsteroidSize};
use crate::net::LocalTeam;
use crate::ship::{Ship, ShipClass, ShipSecrets, ShipSecretsOwner, TargetDesignation};
use crate::weapon::missile::{Missile, MissileVelocity};
use crate::weapon::projectile::Projectile;

/// System that watches for newly replicated ship entities (via `Added<Ship>` filter)
/// and spawns the appropriate mesh + material as a child entity.
pub fn materialize_ships(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    local_team: Res<LocalTeam>,
    query: Query<(Entity, &ShipClass, &Team), Added<Ship>>,
) {
    for (entity, class, team) in &query {
        let is_own_team = local_team
            .0
            .map(|lt| lt == *team)
            .unwrap_or(false);

        let color = if is_own_team {
            Color::srgb(0.2, 0.6, 1.0)
        } else {
            Color::srgb(1.0, 0.2, 0.2)
        };

        let (ship_mesh, mesh_transform) = class.create_mesh(&mut meshes);

        let ship_material = if is_own_team {
            materials.add(StandardMaterial {
                base_color: color,
                emissive: color.into(),
                alpha_mode: AlphaMode::Opaque,
                ..default()
            })
        } else {
            materials.add(StandardMaterial {
                base_color: color.with_alpha(1.0),
                emissive: color.into(),
                alpha_mode: AlphaMode::Blend,
                ..default()
            })
        };

        commands
            .entity(entity)
            .insert(Visibility::Visible)
            .with_child((
                Mesh3d(ship_mesh),
                MeshMaterial3d(ship_material),
                mesh_transform,
            ))
            .observe(on_ship_clicked);

        info!(
            "Materialized {:?} ship for team {} (own={})",
            class, team.0, is_own_team
        );
    }
}

/// System that watches for newly replicated asteroid entities (via `Added<Asteroid>` filter)
/// and spawns the appropriate mesh + material as a child entity.
pub fn materialize_asteroids(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    query: Query<(Entity, &AsteroidSize), Added<Asteroid>>,
) {
    for (entity, size) in &query {
        let asteroid_mesh = meshes.add(Sphere::new(size.radius));
        let asteroid_material = materials.add(StandardMaterial {
            base_color: Color::srgb(0.3, 0.25, 0.2),
            perceptual_roughness: 0.9,
            ..default()
        });

        commands
            .entity(entity)
            .insert(Visibility::Visible)
            .with_child((
                Mesh3d(asteroid_mesh),
                MeshMaterial3d(asteroid_material),
                Transform::IDENTITY,
            ));

        info!("Materialized asteroid with radius {}", size.radius);
    }
}

/// System that watches for newly replicated projectile entities and spawns
/// a small glowing sphere mesh as a child entity.
pub fn materialize_projectiles(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    query: Query<Entity, Added<Projectile>>,
) {
    for entity in &query {
        let projectile_mesh = meshes.add(Sphere::new(0.5).mesh().uv(8, 8));
        let projectile_material = materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.7, 0.1),
            emissive: LinearRgba::new(4.0, 2.8, 0.4, 1.0),
            unlit: true,
            alpha_mode: AlphaMode::Opaque,
            ..default()
        });

        commands
            .entity(entity)
            .insert(Visibility::Visible)
            .with_child((
                Mesh3d(projectile_mesh),
                MeshMaterial3d(projectile_material),
                Transform::IDENTITY,
            ));
    }
}

/// System that watches for newly replicated missile entities and spawns
/// a small glowing cone mesh as a child entity, oriented along the velocity.
pub fn materialize_missiles(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    query: Query<(Entity, &MissileVelocity), Added<Missile>>,
) {
    for (entity, velocity) in &query {
        let missile_mesh = meshes.add(Cone {
            radius: 1.5,
            height: 3.0,
        });
        let missile_material = materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.4, 0.0),
            emissive: LinearRgba::new(5.0, 2.0, 0.0, 1.0),
            unlit: true,
            alpha_mode: AlphaMode::Opaque,
            ..default()
        });

        // Orient cone to point along velocity direction
        let vel_dir = velocity.0.normalize_or_zero();
        let child_transform = if vel_dir != Vec3::ZERO {
            // Cone points along +Y by default, rotate to match velocity
            let rotation = Quat::from_rotation_arc(Vec3::Y, vel_dir);
            Transform::from_rotation(rotation)
        } else {
            Transform::IDENTITY
        };

        commands
            .entity(entity)
            .insert(Visibility::Visible)
            .with_child((
                Mesh3d(missile_mesh),
                MeshMaterial3d(missile_material),
                child_transform,
            ));
    }
}

// ── Targeting Indicators ────────────────────────────────────────────────

/// Marker component for target indicator entities (client-only visuals).
#[derive(Component)]
pub struct TargetIndicator {
    pub owner: Entity,
}

/// Cached mesh/material handles for target indicators (avoids per-frame allocation).
#[derive(Resource)]
pub struct TargetIndicatorAssets {
    pub mesh: Handle<Mesh>,
    pub material: Handle<StandardMaterial>,
}

/// Startup system that creates the torus mesh + red material for target indicators.
pub fn init_target_indicator_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(TargetIndicatorAssets {
        mesh: meshes.add(Torus::new(6.0, 8.0)),
        material: materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.15, 0.1, 0.7),
            emissive: LinearRgba::new(2.0, 0.3, 0.2, 1.0),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
    });
}

/// System that renders a red torus at the position of each targeted enemy ship.
/// Only shows targets for the local player's team.
pub fn update_target_indicators(
    mut commands: Commands,
    assets: Res<TargetIndicatorAssets>,
    local_team: Res<LocalTeam>,
    secrets_query: Query<
        (&ShipSecretsOwner, &TargetDesignation),
        With<ShipSecrets>,
    >,
    ship_query: Query<(&Transform, &Team), With<Ship>>,
    indicator_query: Query<(Entity, &TargetIndicator)>,
) {
    // Despawn all existing target indicators
    for (entity, _) in &indicator_query {
        commands.entity(entity).despawn();
    }

    let Some(my_team) = local_team.0 else { return };

    for (owner, target_designation) in &secrets_query {
        // Only show for own-team ships
        let Ok((_, team)) = ship_query.get(owner.0) else {
            continue;
        };
        if *team != my_team {
            continue;
        }

        // Look up the target ship's position
        let Ok((target_transform, _)) = ship_query.get(target_designation.0) else {
            continue;
        };

        let pos = target_transform.translation;
        commands.spawn((
            TargetIndicator { owner: owner.0 },
            Mesh3d(assets.mesh.clone()),
            MeshMaterial3d(assets.material.clone()),
            Transform::from_xyz(pos.x, 1.0, pos.z),
        ));
    }
}
