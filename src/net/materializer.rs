use bevy::prelude::*;

use crate::game::Team;
use crate::input::{on_ship_clicked, EnemyNumbers, MoveMode, SquadHighlight};
use crate::map::{Asteroid, AsteroidSize};
use crate::net::LocalTeam;
use crate::ship::{
    Selected, Ship, ShipClass, ShipNumber, ShipSecrets, ShipSecretsOwner, TargetDesignation,
};
use crate::weapon::missile::{
    Explosion, Missile, MissileVelocity, SEEKER_HALF_ANGLE, SEEKER_MAX_RANGE,
};

/// Toggle for debug visualizations (seeker cones, PD ranges, etc).
/// Press F3 to toggle at runtime.
#[derive(Resource)]
pub struct DebugVisuals(pub bool);

impl Default for DebugVisuals {
    fn default() -> Self {
        Self(false)
    }
}

/// Marker for the seeker cone visual child.
#[derive(Component)]
pub struct SeekerConeVisual;
use crate::weapon::pd::{LaserBeam, LaserBeamTarget};
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

/// Marker for ship number label entities (UI text positioned via world-to-screen projection).
#[derive(Component)]
pub struct ShipNumberLabel {
    pub owner: Entity,
}

/// System that creates/updates floating number labels above friendly ships.
/// Reads ShipNumber from ShipSecrets (team-private). Uses world-to-screen
/// projection to position UI text nodes over ship positions.
pub fn update_ship_number_labels(
    mut commands: Commands,
    local_team: Res<LocalTeam>,
    secrets_query: Query<(&ShipSecretsOwner, &ShipNumber), With<ShipSecrets>>,
    ship_query: Query<(&Transform, &Team), With<Ship>>,
    label_query: Query<(Entity, &ShipNumberLabel)>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
) {
    // Despawn all existing labels
    for (entity, _) in &label_query {
        commands.entity(entity).despawn();
    }

    let Some(my_team) = local_team.0 else {
        return;
    };

    let Ok((camera, camera_gt)) = camera_query.single() else {
        return;
    };

    for (owner, ship_number) in &secrets_query {
        if ship_number.0 == 0 {
            continue;
        }
        let Ok((transform, team)) = ship_query.get(owner.0) else {
            continue;
        };
        if *team != my_team {
            continue;
        }

        // Project world position to screen coordinates (label floats below ship)
        let world_pos = transform.translation + Vec3::Y * -2.0;
        let Ok(screen_pos) = camera.world_to_viewport(camera_gt, world_pos) else {
            continue;
        };

        commands.spawn((
            ShipNumberLabel { owner: owner.0 },
            Text::new(format!("{}", ship_number.0)),
            TextFont {
                font_size: 14.0,
                ..default()
            },
            TextColor(Color::srgba(0.8, 0.95, 1.0, 0.85)),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(screen_pos.x - 4.0),
                top: Val::Px(screen_pos.y),
                ..default()
            },
            Pickable::IGNORE,
        ));
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

/// Marker for the laser beam mesh child so we can update it each frame.
#[derive(Component)]
pub struct LaserBeamMesh;

/// Materialize laser beam entities with a unit cuboid that gets scaled each frame.
pub fn materialize_laser_beams(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    query: Query<Entity, Added<LaserBeam>>,
) {
    for entity in &query {
        // Unit-length cuboid — scaled to beam length each frame
        let beam_mesh = meshes.add(Cuboid::new(0.3, 0.3, 1.0));
        let beam_material = materials.add(StandardMaterial {
            base_color: Color::srgb(0.5, 1.0, 0.5),
            emissive: LinearRgba::new(2.0, 8.0, 2.0, 1.0),
            unlit: true,
            ..default()
        });

        commands
            .entity(entity)
            .insert(Visibility::Visible)
            .with_child((
                LaserBeamMesh,
                Mesh3d(beam_mesh),
                MeshMaterial3d(beam_material),
                Transform::IDENTITY,
                Pickable::IGNORE,
            ));
    }
}

/// Update laser beam mesh to stretch between origin (parent Transform) and target each frame.
pub fn update_laser_beam_meshes(
    beam_query: Query<(&Transform, &LaserBeamTarget, &Children), With<LaserBeam>>,
    mut mesh_query: Query<&mut Transform, (With<LaserBeamMesh>, Without<LaserBeam>)>,
) {
    for (beam_tf, beam_target, children) in &beam_query {
        let origin = beam_tf.translation;
        let target = beam_target.0;
        let diff = target - origin;
        let length = diff.length();
        let dir = diff.normalize_or_zero();

        if length < 0.01 || dir == Vec3::ZERO {
            continue;
        }

        let rotation = Quat::from_rotation_arc(Vec3::Z, dir);
        let midpoint_offset = dir * (length / 2.0);

        for child in children.iter() {
            if let Ok(mut mesh_tf) = mesh_query.get_mut(child) {
                mesh_tf.translation = midpoint_offset;
                mesh_tf.rotation = rotation;
                mesh_tf.scale = Vec3::new(1.0, 1.0, length);
            }
        }
    }
}

/// System that watches for newly replicated explosion entities and spawns
/// a bright expanding sphere as a visual flash.
pub fn materialize_explosions(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    query: Query<Entity, Added<Explosion>>,
) {
    for entity in &query {
        let mesh = meshes.add(Sphere::new(3.0).mesh().uv(8, 8));
        let material = materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.6, 0.1, 0.9),
            emissive: LinearRgba::new(10.0, 5.0, 1.0, 1.0),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        });

        commands
            .entity(entity)
            .insert(Visibility::Visible)
            .with_child((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::IDENTITY,
            ));
    }
}

// ── Debug Visuals ───────────────────────────────────────────────────────

/// Toggle debug visuals on F3.
pub fn toggle_debug_visuals(keys: Res<ButtonInput<KeyCode>>, mut dbg_vis: ResMut<DebugVisuals>) {
    if keys.just_pressed(KeyCode::F3) {
        dbg_vis.0 = !dbg_vis.0;
        info!("Debug visuals: {}", if dbg_vis.0 { "ON" } else { "OFF" });
    }
}

/// Spawn seeker cone children on new missiles when debug visuals are enabled.
pub fn spawn_debug_seeker_cones(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    dbg_vis: Res<DebugVisuals>,
    query: Query<(Entity, &MissileVelocity), Added<Missile>>,
) {
    if !dbg_vis.0 {
        return;
    }

    for (entity, velocity) in &query {
        let cone_radius = SEEKER_HALF_ANGLE.tan() * SEEKER_MAX_RANGE;
        let seeker_mesh = meshes.add(Cone {
            radius: cone_radius,
            height: SEEKER_MAX_RANGE,
        });
        let seeker_material = materials.add(StandardMaterial {
            base_color: Color::srgba(0.0, 1.0, 0.5, 0.08),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            cull_mode: None,
            ..default()
        });

        let vel_dir = velocity.0.normalize_or_zero();
        let rotation = if vel_dir != Vec3::ZERO {
            Quat::from_rotation_arc(Vec3::NEG_Y, vel_dir)
        } else {
            Quat::IDENTITY
        };

        commands.entity(entity).with_child((
            SeekerConeVisual,
            Mesh3d(seeker_mesh),
            MeshMaterial3d(seeker_material),
            Transform {
                translation: vel_dir * (SEEKER_MAX_RANGE / 2.0),
                rotation,
                ..default()
            },
            Pickable::IGNORE,
        ));
    }
}

/// Update seeker cone orientation to match parent missile's velocity each frame.
pub fn update_debug_seeker_cones(
    dbg_vis: Res<DebugVisuals>,
    missile_query: Query<(&MissileVelocity, &Children), With<Missile>>,
    mut cone_query: Query<&mut Transform, With<SeekerConeVisual>>,
) {
    if !dbg_vis.0 {
        return;
    }

    for (vel, children) in &missile_query {
        let dir = vel.0.normalize_or_zero();
        if dir == Vec3::ZERO {
            continue;
        }
        let flip = Quat::from_rotation_arc(Vec3::NEG_Y, dir);
        for child in children.iter() {
            if let Ok(mut tf) = cone_query.get_mut(child) {
                tf.rotation = flip;
                tf.translation = dir * (SEEKER_MAX_RANGE / 2.0);
            }
        }
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

/// Cached mesh/material handles for squad highlight indicators.
#[derive(Resource)]
pub struct SquadIndicatorAssets {
    pub highlight_mesh: Handle<Mesh>,
    pub highlight_material: Handle<StandardMaterial>,
    pub line_mesh: Handle<Mesh>,
    pub line_material: Handle<StandardMaterial>,
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

    // Squad visual assets
    commands.insert_resource(SquadIndicatorAssets {
        // Dimmed gray torus for follower highlights
        highlight_mesh: meshes.add(Torus::new(12.0, 14.0)),
        highlight_material: materials.add(StandardMaterial {
            base_color: Color::srgba(0.5, 0.5, 0.5, 0.4),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        }),
        // Cyan capsule for follower-to-leader connection line
        line_mesh: meshes.add(Capsule3d::new(0.3, 1.0)),
        line_material: materials.add(StandardMaterial {
            base_color: Color::srgba(0.2, 0.9, 0.9, 0.5),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
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

// ── Squad Visual Indicators ──────────────────────────────────────────────

/// Marker for squad highlight torus entities (dimmed gray ring under followers).
#[derive(Component)]
pub struct SquadHighlightIndicator;

/// Marker for squad connection line entities (cyan line from follower to leader).
#[derive(Component)]
pub struct SquadConnectionLine;

/// System that renders dimmed gray torus rings under squad followers of the selected leader.
pub fn update_squad_highlight_indicators(
    mut commands: Commands,
    assets: Res<SquadIndicatorAssets>,
    highlight_query: Query<&Transform, With<SquadHighlight>>,
    indicator_query: Query<Entity, With<SquadHighlightIndicator>>,
) {
    // Despawn all existing indicators
    for entity in &indicator_query {
        commands.entity(entity).despawn();
    }

    // Spawn new indicators for highlighted followers (at ship height)
    for transform in &highlight_query {
        let pos = transform.translation;
        commands.spawn((
            SquadHighlightIndicator,
            Mesh3d(assets.highlight_mesh.clone()),
            MeshMaterial3d(assets.highlight_material.clone()),
            Transform::from_xyz(pos.x, 5.0, pos.z),
            Pickable::IGNORE,
        ));
    }
}

/// Helper: spawn a cyan connection line between two world XZ positions.
/// Line is drawn at ship height (Y=5) so it's visible from the strategic camera.
fn spawn_connection_line(
    commands: &mut Commands,
    assets: &SquadIndicatorAssets,
    from_pos: Vec3,
    to_pos: Vec3,
) {
    let from = Vec3::new(from_pos.x, 5.0, from_pos.z);
    let to = Vec3::new(to_pos.x, 5.0, to_pos.z);
    let diff = to - from;
    let length = diff.length();
    if length < 1.0 {
        return;
    }

    let mid = from + diff * 0.5;
    // Compute rotation to align Y-axis capsule along the XZ direction
    let dir_xz = Vec3::new(diff.x, 0.0, diff.z).normalize();
    // Rotate from Y-up to horizontal direction
    let rotation = Quat::from_rotation_arc(Vec3::Y, dir_xz);

    commands.spawn((
        SquadConnectionLine,
        Mesh3d(assets.line_mesh.clone()),
        MeshMaterial3d(assets.line_material.clone()),
        Transform::from_translation(mid)
            .with_rotation(rotation)
            .with_scale(Vec3::new(2.0, length, 2.0)),
        Pickable::IGNORE,
    ));
}

/// Marker for squad info text labels (e.g. "Following: 2" or "Squad: 3").
#[derive(Component)]
pub struct SquadInfoLabel;

/// System that draws cyan connection lines for squad relationships:
/// - Selected follower -> line to leader
/// - Selected leader -> lines from all highlighted followers to leader
/// Also shows squad info text near the selected ship.
pub fn update_squad_connection_lines(
    mut commands: Commands,
    assets: Res<SquadIndicatorAssets>,
    selected_query: Query<(Entity, &Transform), (With<crate::ship::Selected>, With<Ship>)>,
    secrets_query: Query<(&ShipSecretsOwner, Option<&crate::ship::SquadMember>, Option<&ShipNumber>), With<ShipSecrets>>,
    highlight_query: Query<(Entity, &Transform), With<SquadHighlight>>,
    ship_query: Query<&Transform, With<Ship>>,
    line_query: Query<Entity, With<SquadConnectionLine>>,
    label_query: Query<Entity, With<SquadInfoLabel>>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
) {
    // Despawn all existing lines and labels
    for entity in &line_query {
        commands.entity(entity).despawn();
    }
    for entity in &label_query {
        commands.entity(entity).despawn();
    }

    let Some((selected_entity, selected_tf)) = selected_query.iter().next() else {
        return;
    };

    // Check if selected ship is a squad follower
    let selected_squad = secrets_query
        .iter()
        .find(|(owner, _, _)| owner.0 == selected_entity)
        .and_then(|(_, sm, _)| sm.cloned());

    if let Some(squad) = &selected_squad {
        // Selected ship is a follower — draw line to leader
        if let Ok(leader_tf) = ship_query.get(squad.leader) {
            spawn_connection_line(&mut commands, &assets, selected_tf.translation, leader_tf.translation);

            // Show "Following: N" where N is the leader's ShipNumber
            let leader_number = secrets_query
                .iter()
                .find(|(owner, _, _)| owner.0 == squad.leader)
                .and_then(|(_, _, sn)| sn.map(|n| n.0))
                .unwrap_or(0);

            if leader_number > 0 {
                if let Ok((camera, camera_gt)) = camera_query.single() {
                    let label_pos = selected_tf.translation + Vec3::Y * 15.0;
                    if let Ok(screen_pos) = camera.world_to_viewport(camera_gt, label_pos) {
                        commands.spawn((
                            SquadInfoLabel,
                            Text::new(format!("Following: {}", leader_number)),
                            TextFont { font_size: 12.0, ..default() },
                            TextColor(Color::srgba(0.2, 0.9, 0.9, 0.8)),
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Px(screen_pos.x - 30.0),
                                top: Val::Px(screen_pos.y - 8.0),
                                ..default()
                            },
                            Pickable::IGNORE,
                        ));
                    }
                }
            }
        }
    }

    // If selected ship is a leader, draw lines from highlighted followers to leader
    let has_followers = highlight_query.iter().next().is_some();
    if has_followers {
        // Count followers for info label
        let follower_count = highlight_query.iter().count();

        for (_follower_entity, follower_tf) in &highlight_query {
            spawn_connection_line(&mut commands, &assets, follower_tf.translation, selected_tf.translation);
        }

        // Show follower count near leader
        if let Ok((camera, camera_gt)) = camera_query.single() {
            let label_pos = selected_tf.translation + Vec3::Y * 15.0;
            if let Ok(screen_pos) = camera.world_to_viewport(camera_gt, label_pos) {
                // Only show if we didn't already spawn a "Following" label
                if selected_squad.is_none() {
                    commands.spawn((
                        SquadInfoLabel,
                        Text::new(format!("Squad: {}", follower_count)),
                        TextFont { font_size: 12.0, ..default() },
                        TextColor(Color::srgba(0.2, 0.9, 0.9, 0.8)),
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(screen_pos.x - 20.0),
                            top: Val::Px(screen_pos.y - 8.0),
                            ..default()
                        },
                        Pickable::IGNORE,
                    ));
                }
            }
        }
    }
}

// ── LOS Circle (Move Mode) ─────────────────────────────────────────────

/// Marker for the LOS (vision range) circle shown in move mode.
#[derive(Component)]
pub struct LosCircle;

/// Cached assets for the LOS circle.
#[derive(Resource)]
pub struct LosCircleAssets {
    pub mesh: Handle<Mesh>,
    pub material: Handle<StandardMaterial>,
}

/// Startup: create LOS circle assets.
pub fn init_los_circle_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Unit torus — scaled to 400m at runtime
    commands.insert_resource(LosCircleAssets {
        mesh: meshes.add(Torus::new(0.98, 1.0)),
        material: materials.add(StandardMaterial {
            base_color: Color::srgba(0.2, 1.0, 0.3, 0.15),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        }),
    });
}

/// System: spawn/despawn and position the LOS circle based on move mode + selection.
pub fn update_los_circle(
    mut commands: Commands,
    move_mode: Res<MoveMode>,
    assets: Res<LosCircleAssets>,
    selected_query: Query<(&Transform, &ShipClass), (With<Selected>, With<Ship>)>,
    circle_query: Query<Entity, With<LosCircle>>,
) {
    let show = move_mode.0 && selected_query.iter().next().is_some();

    if !show {
        // Despawn all LOS circles
        for entity in &circle_query {
            commands.entity(entity).despawn();
        }
        return;
    }

    let Some((ship_tf, ship_class)) = selected_query.iter().next() else {
        return;
    };
    let vision_range = ship_class.profile().vision_range;

    if circle_query.iter().next().is_some() {
        // Already exists — we can't easily update transforms here without mut query conflicts,
        // so despawn and respawn (cheap per frame for a single entity)
        for entity in &circle_query {
            commands.entity(entity).despawn();
        }
    }

    commands.spawn((
        LosCircle,
        Mesh3d(assets.mesh.clone()),
        MeshMaterial3d(assets.material.clone()),
        Transform::from_xyz(ship_tf.translation.x, 0.5, ship_tf.translation.z)
            .with_scale(Vec3::splat(vision_range)),
        Pickable::IGNORE,
    ));
}

// ── Enemy Number Labels ─────────────────────────────────────────────────

/// Marker for enemy number label entities.
#[derive(Component)]
pub struct EnemyNumberLabel;

/// System that shows red number labels below visible enemy ships when EnemyNumbers is active.
pub fn update_enemy_number_labels(
    mut commands: Commands,
    enemy_numbers: Res<EnemyNumbers>,
    ship_query: Query<&Transform, With<Ship>>,
    label_query: Query<Entity, With<EnemyNumberLabel>>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
) {
    // Despawn all existing enemy number labels
    for entity in &label_query {
        commands.entity(entity).despawn();
    }

    if !enemy_numbers.active {
        return;
    }

    let Ok((camera, camera_gt)) = camera_query.single() else {
        return;
    };

    for (&ship_entity, &number) in &enemy_numbers.assignments {
        let Ok(transform) = ship_query.get(ship_entity) else {
            continue;
        };

        // Project world position to screen (below ship)
        let world_pos = transform.translation + Vec3::Y * -2.0;
        let Ok(screen_pos) = camera.world_to_viewport(camera_gt, world_pos) else {
            continue;
        };

        commands.spawn((
            EnemyNumberLabel,
            Text::new(format!("{}", number)),
            TextFont {
                font_size: 14.0,
                ..default()
            },
            TextColor(Color::WHITE),
            BackgroundColor(Color::srgba(0.7, 0.1, 0.1, 0.8)),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(screen_pos.x - 6.0),
                top: Val::Px(screen_pos.y),
                padding: UiRect::axes(Val::Px(4.0), Val::Px(1.0)),
                ..default()
            },
            Pickable::IGNORE,
        ));
    }
}
