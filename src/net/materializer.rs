use bevy::prelude::*;

use crate::game::Team;
use crate::net::LocalTeam;
use crate::ship::{Ship, ShipClass};

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

        let ship_mesh = match class {
            ShipClass::Battleship => meshes.add(Cuboid::new(12.0, 8.0, 28.0)),
            ShipClass::Destroyer => meshes.add(Cone {
                radius: 8.0,
                height: 20.0,
            }),
            ShipClass::Scout => meshes.add(Sphere::new(1.0).mesh().uv(16, 16)),
        };

        let mesh_transform = match class {
            ShipClass::Battleship => Transform::IDENTITY,
            ShipClass::Destroyer => {
                Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2))
            }
            ShipClass::Scout => Transform::from_scale(Vec3::new(4.0, 3.0, 7.0)),
        };

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

        commands.entity(entity).with_child((
            Mesh3d(ship_mesh),
            MeshMaterial3d(ship_material),
            mesh_transform,
        ));

        info!(
            "Materialized {:?} ship for team {} (own={})",
            class, team.0, is_own_team
        );
    }
}
