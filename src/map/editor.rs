use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;

use crate::camera::{
    camera_look_ground, compute_zoom, CameraLookAt, CameraSettings, GameCamera,
};
use crate::game::{GameState, Team};
use crate::map::data::{
    load_map_data, save_map_data, AsteroidDef, ControlPointDef, MapData, SpawnPoint,
};
use crate::map::{GroundPlane, MapBounds};

// ── Constants ────────────────────────────────────────────────────────────

const BG_DARK: Color = Color::srgba(0.08, 0.08, 0.12, 0.9);
const BG_PANEL: Color = Color::srgb(0.12, 0.12, 0.18);
const BG_BUTTON: Color = Color::srgb(0.2, 0.2, 0.35);
const BG_BUTTON_ACTIVE: Color = Color::srgb(0.25, 0.25, 0.45);
const BG_SAVE: Color = Color::srgb(0.15, 0.55, 0.2);
const TEXT_WHITE: Color = Color::WHITE;
const TEXT_GRAY: Color = Color::srgb(0.6, 0.6, 0.6);

const ASTEROID_COLOR: Color = Color::srgb(0.3, 0.25, 0.2);
const CONTROL_POINT_COLOR: Color = Color::srgba(1.0, 1.0, 0.2, 0.3);

// ── Resources ────────────────────────────────────────────────────────────

/// Optional resource: path to load a map file in the editor.
#[derive(Resource, Debug, Clone)]
pub struct EditorMapPath(pub String);

#[derive(Resource, Debug, Default)]
pub struct EditorFileName(pub Option<String>);

#[derive(Resource, Debug, Clone, Default)]
pub struct EditorMapData(pub MapData);

#[derive(Resource, Debug, Default)]
pub struct EditorState {
    pub tool: EditorTool,
    pub selected: Option<Entity>,
}

#[derive(Resource, Default)]
pub struct EditorDragState {
    pub dragging: bool,
    pub start_world: Vec2,
}

// ── EditorTool enum ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditorTool {
    #[default]
    Select,
    PlaceAsteroid,
    PlaceControlPoint,
    PlaceSpawn,
}

impl EditorTool {
    fn label(self) -> &'static str {
        match self {
            Self::Select => "Select (1)",
            Self::PlaceAsteroid => "Asteroid (2)",
            Self::PlaceControlPoint => "Ctrl Point (3)",
            Self::PlaceSpawn => "Spawn (4)",
        }
    }

    fn indicator_label(self) -> &'static str {
        match self {
            Self::Select => "SELECT",
            Self::PlaceAsteroid => "PLACE ASTEROID",
            Self::PlaceControlPoint => "PLACE CONTROL POINT",
            Self::PlaceSpawn => "PLACE SPAWN",
        }
    }
}

// ── Marker Components ────────────────────────────────────────────────────

#[derive(Component)]
pub struct EditorAsteroid;

#[derive(Component)]
pub struct EditorControlPoint;

#[derive(Component)]
pub struct EditorSpawn(pub u8);

#[derive(Component)]
pub struct EditorSelected;

#[derive(Component)]
pub struct EditorUiRoot;

#[derive(Component)]
pub struct ToolButton(pub EditorTool);

#[derive(Component)]
pub struct SaveButton;

#[derive(Component)]
pub struct LoadButton;

#[derive(Component)]
pub struct FileNameText;

#[derive(Component)]
pub struct EditorToolIndicator;

#[derive(Component)]
pub struct LoadPopupOverlay;

#[derive(Component)]
pub struct LoadFileOption(pub String);

#[derive(Component)]
pub struct PopupCancelButton;

#[derive(Component)]
pub struct SavePopupOverlay;

#[derive(Component)]
pub struct SaveNameInput;

#[derive(Component)]
pub struct SaveConfirmButton;

// ── Plugin ───────────────────────────────────────────────────────────────

pub struct MapEditorPlugin;

impl Plugin for MapEditorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EditorMapData>()
            .init_resource::<EditorFileName>()
            .init_resource::<EditorState>()
            .init_resource::<EditorDragState>()
            .add_systems(
                OnEnter(GameState::Editor),
                (setup_editor_scene, spawn_editor_ui),
            )
            .add_systems(
                Update,
                (
                    handle_editor_hotkeys,
                    handle_tool_button_clicks,
                    handle_editor_delete,
                    handle_editor_drag,
                    editor_camera_zoom_or_resize,
                    handle_save,
                    handle_save_input,
                    handle_save_confirm,
                    handle_load_request,
                    handle_load_file_click,
                    close_popup_on_escape,
                    handle_popup_cancel,
                )
                    .run_if(in_state(GameState::Editor)),
            )
            .add_systems(
                Update,
                (
                    update_tool_buttons,
                    update_tool_indicator,
                    update_file_name_text,
                    draw_editor_bounds_gizmos,
                    draw_editor_entity_gizmos,
                )
                    .run_if(in_state(GameState::Editor)),
            );
    }
}

// ── Scene Setup ──────────────────────────────────────────────────────────

fn setup_editor_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    map_path: Option<Res<EditorMapPath>>,
    mut editor_data: ResMut<EditorMapData>,
    mut editor_file: ResMut<EditorFileName>,
) {
    // Load map from file if path provided
    if let Some(path_res) = map_path {
        let file_path = std::path::Path::new("assets/maps").join(&path_res.0);
        match load_map_data(&file_path) {
            Ok(data) => {
                info!("Editor: loaded map from {}", path_res.0);
                editor_data.0 = data;
                editor_file.0 = Some(path_res.0.clone());
            }
            Err(e) => {
                warn!("Editor: failed to load map '{}': {}", path_res.0, e);
            }
        }
    }

    info!(
        "Editor: scene setup — bounds {}x{}, {} asteroids, {} control points, {} spawns",
        editor_data.0.bounds.half_x * 2.0,
        editor_data.0.bounds.half_y * 2.0,
        editor_data.0.asteroids.len(),
        editor_data.0.control_points.len(),
        editor_data.0.spawns.len(),
    );

    // Insert MapBounds from loaded data
    let bounds = MapBounds {
        half_extents: Vec2::new(editor_data.0.bounds.half_x, editor_data.0.bounds.half_y),
    };
    commands.insert_resource(bounds.clone());

    // Spawn dark ground plane (3x bounds for generous clicking)
    let ground_size = bounds.size() * 3.0;
    commands.spawn((
        GroundPlane,
        Mesh3d(meshes.add(Plane3d::new(
            Vec3::Y,
            Vec2::new(ground_size.x / 2.0, ground_size.y / 2.0),
        ))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.02, 0.02, 0.05),
            perceptual_roughness: 1.0,
            ..default()
        })),
        Transform::from_xyz(0.0, 0.0, 0.0),
        Pickable::default(),
    ));

    // Register ground click observer
    commands.add_observer(handle_editor_ground_click);

    // Spawn visual entities from loaded map data
    let data = editor_data.0.clone();

    for asteroid_def in &data.asteroids {
        let pos = Vec2::new(asteroid_def.position.0, asteroid_def.position.1);
        spawn_editor_asteroid(&mut commands, &mut meshes, &mut materials, pos, asteroid_def.radius);
    }

    for cp_def in &data.control_points {
        let pos = Vec2::new(cp_def.position.0, cp_def.position.1);
        spawn_editor_control_point(&mut commands, &mut meshes, &mut materials, pos, cp_def.radius);
    }

    for spawn_def in &data.spawns {
        let pos = Vec2::new(spawn_def.position.0, spawn_def.position.1);
        spawn_editor_spawn_point(
            &mut commands,
            &mut meshes,
            &mut materials,
            pos,
            spawn_def.team,
        );
    }
}

// ── Entity Spawn Helpers ─────────────────────────────────────────────────

fn spawn_editor_asteroid(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    pos: Vec2,
    radius: f32,
) -> Entity {
    let entity = commands
        .spawn((
            EditorAsteroid,
            Transform::from_xyz(pos.x, 0.0, pos.y),
            Visibility::Visible,
            Pickable::default(),
        ))
        .with_child((
            Mesh3d(meshes.add(Sphere::new(radius))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: ASTEROID_COLOR,
                perceptual_roughness: 0.9,
                ..default()
            })),
        ))
        .observe(handle_editor_entity_click)
        .id();
    info!("Editor: placed asteroid at ({:.0}, {:.0}) radius={:.0}", pos.x, pos.y, radius);
    entity
}

fn spawn_editor_control_point(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    pos: Vec2,
    radius: f32,
) -> Entity {
    let entity = commands
        .spawn((
            EditorControlPoint,
            Transform::from_xyz(pos.x, 0.0, pos.y),
            Visibility::Visible,
            Pickable::default(),
        ))
        .with_child((
            Mesh3d(meshes.add(Cylinder::new(radius, 2.0))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: CONTROL_POINT_COLOR,
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                ..default()
            })),
        ))
        .observe(handle_editor_entity_click)
        .id();
    info!("Editor: placed control point at ({:.0}, {:.0}) radius={:.0}", pos.x, pos.y, radius);
    entity
}

fn spawn_editor_spawn_point(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    pos: Vec2,
    team: u8,
) -> Entity {
    let color = Team(team).color();
    let entity = commands
        .spawn((
            EditorSpawn(team),
            Transform::from_xyz(pos.x, 0.0, pos.y),
            Visibility::Visible,
            Pickable::default(),
        ))
        .with_child((
            Mesh3d(meshes.add(Cylinder::new(8.0, 2.0))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: color,
                emissive: LinearRgba::new(color.to_linear().red, color.to_linear().green, color.to_linear().blue, 1.0),
                ..default()
            })),
        ))
        .observe(handle_editor_entity_click)
        .id();
    info!("Editor: placed spawn point team {} at ({:.0}, {:.0})", team, pos.x, pos.y);
    entity
}

// ── Ground Click Observer ────────────────────────────────────────────────

fn handle_editor_ground_click(
    click: On<Pointer<Click>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut editor: ResMut<EditorState>,
    mut editor_data: ResMut<EditorMapData>,
    ground_query: Query<(), With<GroundPlane>>,
    spawn_entities: Query<(Entity, &EditorSpawn)>,
) {
    // Only respond to primary button
    if click.button != PointerButton::Primary {
        return;
    }

    // Make sure this is actually the ground plane
    if ground_query.get(click.event_target()).is_err() {
        return;
    }

    // Get hit position on the ground
    let Some(hit_pos) = click.hit.position else {
        return;
    };

    let raw_pos = Vec2::new(hit_pos.x, hit_pos.z);

    // Clamp placement to map bounds
    let world_pos = {
        let bounds = &editor_data.0.bounds;
        Vec2::new(
            raw_pos.x.clamp(-bounds.half_x, bounds.half_x),
            raw_pos.y.clamp(-bounds.half_y, bounds.half_y),
        )
    };

    match editor.tool {
        EditorTool::Select => {
            // Clicking ground in select mode = deselect
            if let Some(prev) = editor.selected.take() {
                commands.entity(prev).remove::<EditorSelected>();
            }
        }
        EditorTool::PlaceAsteroid => {
            let radius = 25.0;
            spawn_editor_asteroid(
                &mut commands,
                &mut meshes,
                &mut materials,
                world_pos,
                radius,
            );
            editor_data.0.asteroids.push(AsteroidDef {
                position: (world_pos.x, world_pos.y),
                radius,
            });
        }
        EditorTool::PlaceControlPoint => {
            let radius = 100.0;
            spawn_editor_control_point(
                &mut commands,
                &mut meshes,
                &mut materials,
                world_pos,
                radius,
            );
            editor_data.0.control_points.push(ControlPointDef {
                position: (world_pos.x, world_pos.y),
                radius,
            });
        }
        EditorTool::PlaceSpawn => {
            // Find the first team (0-3) that doesn't have a spawn yet
            let team = (0u8..4)
                .find(|&t| !editor_data.0.spawns.iter().any(|s| s.team == t))
                .unwrap_or_else(|| {
                    // All 4 exist — replace team 0 (oldest pattern)
                    for (entity, spawn) in &spawn_entities {
                        if spawn.0 == 0 {
                            commands.entity(entity).despawn();
                            break;
                        }
                    }
                    editor_data.0.spawns.retain(|s| s.team != 0);
                    0
                });

            spawn_editor_spawn_point(
                &mut commands,
                &mut meshes,
                &mut materials,
                world_pos,
                team,
            );
            editor_data.0.spawns.push(SpawnPoint {
                position: (world_pos.x, world_pos.y),
                team,
            });
        }
    }
}

// ── Entity Click Observer ────────────────────────────────────────────────

fn handle_editor_entity_click(
    click: On<Pointer<Press>>,
    mut commands: Commands,
    mut editor: ResMut<EditorState>,
    mut drag: ResMut<EditorDragState>,
    transforms: Query<&Transform>,
) {
    if click.button != PointerButton::Primary {
        return;
    }

    // Only select in Select tool
    if editor.tool != EditorTool::Select {
        return;
    }

    let entity = click.event_target();

    // Deselect previous
    if let Some(prev) = editor.selected.take() {
        commands.entity(prev).remove::<EditorSelected>();
    }

    // Select clicked entity
    commands.entity(entity).insert(EditorSelected);
    editor.selected = Some(entity);
    info!("Editor: selected entity {:?}", entity);

    // Start drag immediately so the user can click-and-drag in one motion
    if let Ok(tf) = transforms.get(entity) {
        drag.dragging = true;
        drag.start_world = Vec2::new(tf.translation.x, tf.translation.z);
    }
}

// ── Hotkeys ──────────────────────────────────────────────────────────────

fn handle_editor_hotkeys(
    keys: Res<ButtonInput<KeyCode>>,
    mut editor: ResMut<EditorState>,
) {
    let prev = editor.tool;
    if keys.just_pressed(KeyCode::Digit1) {
        editor.tool = EditorTool::Select;
    }
    if keys.just_pressed(KeyCode::Digit2) {
        editor.tool = EditorTool::PlaceAsteroid;
    }
    if keys.just_pressed(KeyCode::Digit3) {
        editor.tool = EditorTool::PlaceControlPoint;
    }
    if keys.just_pressed(KeyCode::Digit4) {
        editor.tool = EditorTool::PlaceSpawn;
    }
    if editor.tool != prev {
        info!("Editor: tool → {}", editor.tool.indicator_label());
    }
}

// ── Tool Button Clicks ──────────────────────────────────────────────────

fn handle_tool_button_clicks(
    mut editor: ResMut<EditorState>,
    buttons: Query<(&Interaction, &ToolButton), Changed<Interaction>>,
) {
    for (interaction, tool_btn) in &buttons {
        if *interaction == Interaction::Pressed {
            editor.tool = tool_btn.0;
        }
    }
}

// ── Delete ───────────────────────────────────────────────────────────────

fn handle_editor_delete(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut editor: ResMut<EditorState>,
    mut editor_data: ResMut<EditorMapData>,
    transforms: Query<&Transform>,
    asteroids: Query<(), With<EditorAsteroid>>,
    control_points: Query<(), With<EditorControlPoint>>,
    spawns: Query<&EditorSpawn>,
) {
    if !keys.just_pressed(KeyCode::Delete) && !keys.just_pressed(KeyCode::Backspace) {
        return;
    }

    let Some(entity) = editor.selected.take() else {
        return;
    };

    let Ok(tf) = transforms.get(entity) else {
        return;
    };

    let pos = Vec2::new(tf.translation.x, tf.translation.z);

    // Remove from MapData by position proximity
    if asteroids.get(entity).is_ok() {
        if let Some(idx) = editor_data
            .0
            .asteroids
            .iter()
            .position(|a| Vec2::new(a.position.0, a.position.1).distance(pos) < 1.0)
        {
            editor_data.0.asteroids.remove(idx);
        }
    } else if control_points.get(entity).is_ok() {
        if let Some(idx) = editor_data
            .0
            .control_points
            .iter()
            .position(|c| Vec2::new(c.position.0, c.position.1).distance(pos) < 1.0)
        {
            editor_data.0.control_points.remove(idx);
        }
    } else if let Ok(spawn) = spawns.get(entity) {
        if let Some(idx) = editor_data.0.spawns.iter().position(|s| {
            s.team == spawn.0 && Vec2::new(s.position.0, s.position.1).distance(pos) < 1.0
        }) {
            editor_data.0.spawns.remove(idx);
        }
    }

    info!("Editor: deleted entity {:?} at ({:.0}, {:.0})", entity, pos.x, pos.y);
    commands.entity(entity).despawn();
}

// ── Drag ─────────────────────────────────────────────────────────────────

fn handle_editor_drag(
    mouse_button: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    mut drag: ResMut<EditorDragState>,
    editor: Res<EditorState>,
    mut editor_data: ResMut<EditorMapData>,
    mut transforms: Query<&mut Transform>,
    cam_query: Query<(&Camera, &GlobalTransform), With<GameCamera>>,
    bounds: Option<Res<MapBounds>>,
    asteroids: Query<(), With<EditorAsteroid>>,
    control_points: Query<(), With<EditorControlPoint>>,
    spawns: Query<&EditorSpawn>,
) {
    // Only drag in Select tool with a selected entity
    if editor.tool != EditorTool::Select {
        return;
    }

    let Some(entity) = editor.selected else {
        drag.dragging = false;
        return;
    };

    let Ok(window) = windows.single() else {
        return;
    };
    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };
    let Ok((camera, global_tf)) = cam_query.single() else {
        return;
    };

    // Raycast cursor to Y=0 plane
    let world_pos = match camera.viewport_to_world(global_tf, cursor_pos) {
        Ok(ray) => {
            let dir = ray.direction.as_vec3();
            if dir.y.abs() < 0.001 {
                return;
            }
            let t = -ray.origin.y / dir.y;
            if t < 0.0 {
                return;
            }
            Vec2::new(ray.origin.x + dir.x * t, ray.origin.z + dir.z * t)
        }
        Err(_) => return,
    };

    // Drag start is handled by handle_editor_entity_click observer.
    // Here we only handle movement and release.

    if mouse_button.just_released(MouseButton::Left) {
        if drag.dragging {
            drag.dragging = false;
            // Sync final position to MapData
            if let Ok(tf) = transforms.get(entity) {
                let new_pos = Vec2::new(tf.translation.x, tf.translation.z);
                sync_entity_position_to_data(
                    entity,
                    drag.start_world,
                    new_pos,
                    &mut editor_data,
                    &asteroids,
                    &control_points,
                    &spawns,
                );
                drag.start_world = new_pos;
            }
        }
        return;
    }

    if !drag.dragging {
        return;
    }

    // Move entity (clamped to map bounds)
    let clamped = if let Some(ref bounds) = bounds {
        bounds.clamp(world_pos)
    } else {
        world_pos
    };
    if let Ok(mut tf) = transforms.get_mut(entity) {
        tf.translation.x = clamped.x;
        tf.translation.z = clamped.y;
    }
}

fn sync_entity_position_to_data(
    entity: Entity,
    old_pos: Vec2,
    new_pos: Vec2,
    editor_data: &mut ResMut<EditorMapData>,
    asteroids: &Query<(), With<EditorAsteroid>>,
    control_points: &Query<(), With<EditorControlPoint>>,
    spawns: &Query<&EditorSpawn>,
) {
    if asteroids.get(entity).is_ok() {
        if let Some(a) = editor_data
            .0
            .asteroids
            .iter_mut()
            .find(|a| Vec2::new(a.position.0, a.position.1).distance(old_pos) < 1.0)
        {
            a.position = (new_pos.x, new_pos.y);
        }
    } else if control_points.get(entity).is_ok() {
        if let Some(c) = editor_data
            .0
            .control_points
            .iter_mut()
            .find(|c| Vec2::new(c.position.0, c.position.1).distance(old_pos) < 1.0)
        {
            c.position = (new_pos.x, new_pos.y);
        }
    } else if let Ok(spawn) = spawns.get(entity) {
        if let Some(s) = editor_data.0.spawns.iter_mut().find(|s| {
            s.team == spawn.0 && Vec2::new(s.position.0, s.position.1).distance(old_pos) < 1.0
        }) {
            s.position = (new_pos.x, new_pos.y);
        }
    }
}

// ── Camera Zoom / Asteroid Resize ────────────────────────────────────────

fn editor_camera_zoom_or_resize(
    scroll: Res<AccumulatedMouseScroll>,
    settings: Res<CameraSettings>,
    editor: Res<EditorState>,
    mut editor_data: ResMut<EditorMapData>,
    mut look_at: ResMut<CameraLookAt>,
    mut cam_query: Query<&mut Transform, With<GameCamera>>,
    transforms: Query<&Transform, Without<GameCamera>>,
    asteroids: Query<(), With<EditorAsteroid>>,
    children_query: Query<&Children>,
    mut meshes: ResMut<Assets<Mesh>>,
    mesh_handles: Query<&Mesh3d>,
) {
    if scroll.delta.y.abs() < 0.001 {
        return;
    }

    // If an asteroid is selected, resize it instead of zooming
    if let Some(entity) = editor.selected {
        if asteroids.get(entity).is_ok() {
            if let Ok(tf) = transforms.get(entity) {
                let entity_pos = Vec2::new(tf.translation.x, tf.translation.z);

                // Find matching asteroid in data
                if let Some(asteroid) = editor_data
                    .0
                    .asteroids
                    .iter_mut()
                    .find(|a| {
                        Vec2::new(a.position.0, a.position.1).distance(entity_pos) < 1.0
                    })
                {
                    // Resize: scroll up = bigger, scroll down = smaller
                    let delta = scroll.delta.y * 2.0;
                    asteroid.radius = (asteroid.radius + delta).clamp(5.0, 100.0);
                    let new_radius = asteroid.radius;

                    // Update the mesh asset in-place
                    if let Ok(children) = children_query.get(entity) {
                        for child in children.iter() {
                            if let Ok(mesh_handle) = mesh_handles.get(child) {
                                if let Some(mesh) = meshes.get_mut(&mesh_handle.0) {
                                    *mesh = Sphere::new(new_radius).into();
                                }
                            }
                        }
                    }
                }
            }
            return;
        }
    }

    // Regular camera zoom (replicate camera_zoom logic)
    let Ok(mut transform) = cam_query.single_mut() else {
        return;
    };

    let cam_forward = transform.forward().as_vec3();
    let anchor = look_at.0;

    let Some((new_pos, _)) = compute_zoom(
        transform.translation,
        anchor,
        scroll.delta.y,
        settings.min_zoom,
        settings.max_zoom,
    ) else {
        return;
    };

    transform.translation = new_pos;
    let actual_look = camera_look_ground(new_pos, cam_forward);
    look_at.0 = actual_look;
    transform.look_at(actual_look, Vec3::Y);
}

// ── Save ─────────────────────────────────────────────────────────────────

/// Performs the actual save to disk.
fn do_save(editor_data: &MapData, filename: &str, editor_file: &mut ResMut<EditorFileName>) {
    let maps_dir = std::path::Path::new("assets/maps");
    if let Err(e) = std::fs::create_dir_all(maps_dir) {
        warn!("Failed to create assets/maps: {}", e);
        return;
    }

    let filename = if filename.ends_with(".ron") {
        filename.to_string()
    } else {
        format!("{filename}.ron")
    };

    let file_path = maps_dir.join(&filename);
    match save_map_data(editor_data, &file_path) {
        Ok(()) => {
            info!("Editor: saved map to {}", file_path.display());
            editor_file.0 = Some(filename);
        }
        Err(e) => {
            warn!("Editor: failed to save map: {}", e);
        }
    }
}

/// Resource: holds the name being typed in the save popup.
#[derive(Resource, Default)]
struct SaveInputBuffer(String);

fn handle_save(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    editor_data: Res<EditorMapData>,
    mut editor_file: ResMut<EditorFileName>,
    save_buttons: Query<&Interaction, (With<SaveButton>, Changed<Interaction>)>,
    existing_popups: Query<(), Or<(With<SavePopupOverlay>, With<LoadPopupOverlay>)>>,
) {
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight)
        || keys.pressed(KeyCode::SuperLeft) || keys.pressed(KeyCode::SuperRight);
    let hotkey = ctrl && keys.just_pressed(KeyCode::KeyS);

    let button_pressed = save_buttons
        .iter()
        .any(|i| *i == Interaction::Pressed);

    if !hotkey && !button_pressed {
        return;
    }

    // If file already has a name, save directly
    if let Some(name) = editor_file.0.clone() {
        do_save(&editor_data.0, &name, &mut editor_file);
        return;
    }

    // No name yet — show save popup
    if !existing_popups.is_empty() {
        return;
    }

    commands.insert_resource(SaveInputBuffer("my_map".to_string()));

    commands
        .spawn((
            SavePopupOverlay,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                position_type: PositionType::Absolute,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
            GlobalZIndex(10),
        ))
        .with_children(|overlay| {
            overlay
                .spawn((
                    Node {
                        width: Val::Px(400.0),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(20.0)),
                        row_gap: Val::Px(12.0),
                        ..default()
                    },
                    BackgroundColor(BG_PANEL),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        Text::new("SAVE MAP"),
                        TextFont { font_size: 22.0, ..default() },
                        TextColor(TEXT_WHITE),
                    ));

                    panel.spawn((
                        Text::new("Type a name, then press Enter or click Save:"),
                        TextFont { font_size: 14.0, ..default() },
                        TextColor(TEXT_GRAY),
                    ));

                    // Display current input
                    panel.spawn((
                        SaveNameInput,
                        Text::new("my_map.ron"),
                        TextFont { font_size: 18.0, ..default() },
                        TextColor(Color::srgb(0.4, 1.0, 0.4)),
                        Node {
                            padding: UiRect::all(Val::Px(8.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.1, 0.1, 0.15)),
                    ));

                    // Button row
                    panel
                        .spawn(Node {
                            width: Val::Percent(100.0),
                            justify_content: JustifyContent::SpaceBetween,
                            column_gap: Val::Px(10.0),
                            ..default()
                        })
                        .with_children(|row| {
                            row.spawn((
                                SaveConfirmButton,
                                Button,
                                Node {
                                    flex_grow: 1.0,
                                    padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                                    justify_content: JustifyContent::Center,
                                    ..default()
                                },
                                BackgroundColor(BG_SAVE),
                            ))
                            .with_child((
                                Text::new("Save"),
                                TextFont { font_size: 16.0, ..default() },
                                TextColor(TEXT_WHITE),
                            ));

                            row.spawn((
                                PopupCancelButton,
                                Button,
                                Node {
                                    flex_grow: 1.0,
                                    padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                                    justify_content: JustifyContent::Center,
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.5, 0.2, 0.2)),
                            ))
                            .with_child((
                                Text::new("Cancel"),
                                TextFont { font_size: 16.0, ..default() },
                                TextColor(TEXT_WHITE),
                            ));
                        });
                });
        });
}

/// Handle keyboard input for save name using ButtonInput<KeyCode>.
fn handle_save_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut buffer: Option<ResMut<SaveInputBuffer>>,
    mut text_query: Query<&mut Text, With<SaveNameInput>>,
) {
    let Some(ref mut buffer) = buffer else {
        return;
    };

    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);

    if keys.just_pressed(KeyCode::Backspace) {
        buffer.0.pop();
    }

    // Map key presses to characters (filename-safe only)
    let key_map: &[(KeyCode, char)] = &[
        (KeyCode::KeyA, 'a'), (KeyCode::KeyB, 'b'), (KeyCode::KeyC, 'c'),
        (KeyCode::KeyD, 'd'), (KeyCode::KeyE, 'e'), (KeyCode::KeyF, 'f'),
        (KeyCode::KeyG, 'g'), (KeyCode::KeyH, 'h'), (KeyCode::KeyI, 'i'),
        (KeyCode::KeyJ, 'j'), (KeyCode::KeyK, 'k'), (KeyCode::KeyL, 'l'),
        (KeyCode::KeyM, 'm'), (KeyCode::KeyN, 'n'), (KeyCode::KeyO, 'o'),
        (KeyCode::KeyP, 'p'), (KeyCode::KeyQ, 'q'), (KeyCode::KeyR, 'r'),
        (KeyCode::KeyS, 's'), (KeyCode::KeyT, 't'), (KeyCode::KeyU, 'u'),
        (KeyCode::KeyV, 'v'), (KeyCode::KeyW, 'w'), (KeyCode::KeyX, 'x'),
        (KeyCode::KeyY, 'y'), (KeyCode::KeyZ, 'z'),
        (KeyCode::Digit0, '0'), (KeyCode::Digit1, '1'), (KeyCode::Digit2, '2'),
        (KeyCode::Digit3, '3'), (KeyCode::Digit4, '4'), (KeyCode::Digit5, '5'),
        (KeyCode::Digit6, '6'), (KeyCode::Digit7, '7'), (KeyCode::Digit8, '8'),
        (KeyCode::Digit9, '9'),
        (KeyCode::Minus, '-'), (KeyCode::Period, '.'),
    ];

    // Underscore: shift + minus
    if shift && keys.just_pressed(KeyCode::Minus) {
        buffer.0.push('_');
    } else {
        for &(code, ch) in key_map {
            if keys.just_pressed(code) {
                if shift && ch.is_alphabetic() {
                    buffer.0.push(ch.to_ascii_uppercase());
                } else {
                    buffer.0.push(ch);
                }
            }
        }
    }

    // Update display
    for mut text in &mut text_query {
        let display = if buffer.0.is_empty() {
            "_.ron".to_string()
        } else if buffer.0.ends_with(".ron") {
            buffer.0.clone()
        } else {
            format!("{}.ron", buffer.0)
        };
        **text = display;
    }
}

/// Handle save confirm (button click or Enter key).
fn handle_save_confirm(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    editor_data: Res<EditorMapData>,
    mut editor_file: ResMut<EditorFileName>,
    confirm_buttons: Query<&Interaction, (With<SaveConfirmButton>, Changed<Interaction>)>,
    buffer: Option<Res<SaveInputBuffer>>,
    popup_query: Query<Entity, With<SavePopupOverlay>>,
) {
    if popup_query.is_empty() {
        return;
    }

    let button_pressed = confirm_buttons.iter().any(|i| *i == Interaction::Pressed);
    let enter_pressed = keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter);

    if !button_pressed && !enter_pressed {
        return;
    }

    let filename = buffer
        .as_ref()
        .map(|b| b.0.trim().to_string())
        .unwrap_or_else(|| "untitled".to_string());

    if filename.is_empty() {
        warn!("Editor: filename cannot be empty");
        return;
    }

    do_save(&editor_data.0, &filename, &mut editor_file);

    // Close popup
    for entity in &popup_query {
        commands.entity(entity).despawn();
    }
    commands.remove_resource::<SaveInputBuffer>();
}

// ── Load Request (opens popup) ───────────────────────────────────────────

fn handle_load_request(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    load_buttons: Query<&Interaction, (With<LoadButton>, Changed<Interaction>)>,
    existing_popups: Query<(), With<LoadPopupOverlay>>,
) {
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight)
        || keys.pressed(KeyCode::SuperLeft) || keys.pressed(KeyCode::SuperRight);
    let hotkey = ctrl && keys.just_pressed(KeyCode::KeyO);

    let button_pressed = load_buttons
        .iter()
        .any(|i| *i == Interaction::Pressed);

    if !hotkey && !button_pressed {
        return;
    }

    // Don't open if already open
    if !existing_popups.is_empty() {
        return;
    }

    // List .ron files in assets/maps/
    let maps_dir = std::path::Path::new("assets/maps");
    let mut files: Vec<String> = Vec::new();
    if maps_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(maps_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("ron") {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        files.push(name.to_string());
                    }
                }
            }
        }
    }
    files.sort();

    // Spawn popup overlay
    commands
        .spawn((
            LoadPopupOverlay,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                position_type: PositionType::Absolute,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
            GlobalZIndex(10),
        ))
        .with_children(|overlay| {
            // Inner panel
            overlay
                .spawn((
                    Node {
                        width: Val::Px(400.0),
                        max_height: Val::Percent(80.0),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(20.0)),
                        row_gap: Val::Px(8.0),
                        overflow: Overflow::scroll_y(),
                        ..default()
                    },
                    BackgroundColor(BG_PANEL),
                ))
                .with_children(|panel| {
                    // Title
                    panel.spawn((
                        Text::new("LOAD MAP"),
                        TextFont {
                            font_size: 22.0,
                            ..default()
                        },
                        TextColor(TEXT_WHITE),
                    ));

                    if files.is_empty() {
                        panel.spawn((
                            Text::new("No .ron files found in assets/maps/"),
                            TextFont {
                                font_size: 16.0,
                                ..default()
                            },
                            TextColor(TEXT_GRAY),
                        ));
                    } else {
                        for file in &files {
                            panel
                                .spawn((
                                    LoadFileOption(file.clone()),
                                    Button,
                                    Node {
                                        width: Val::Percent(100.0),
                                        padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                                        justify_content: JustifyContent::FlexStart,
                                        align_items: AlignItems::Center,
                                        ..default()
                                    },
                                    BackgroundColor(BG_BUTTON),
                                ))
                                .with_child((
                                    Text::new(file.clone()),
                                    TextFont {
                                        font_size: 16.0,
                                        ..default()
                                    },
                                    TextColor(TEXT_WHITE),
                                ));
                        }
                    }

                    // Cancel button
                    panel
                        .spawn((
                            PopupCancelButton,
                            Button,
                            Node {
                                width: Val::Percent(100.0),
                                padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                margin: UiRect::top(Val::Px(10.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.5, 0.2, 0.2)),
                        ))
                        .with_child((
                            Text::new("Cancel"),
                            TextFont {
                                font_size: 16.0,
                                ..default()
                            },
                            TextColor(TEXT_WHITE),
                        ));
                });
        });
}

// ── Load File Click ──────────────────────────────────────────────────────

fn handle_load_file_click(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut editor: ResMut<EditorState>,
    mut editor_data: ResMut<EditorMapData>,
    mut editor_file: ResMut<EditorFileName>,
    file_buttons: Query<(&Interaction, &LoadFileOption), Changed<Interaction>>,
    popup_query: Query<Entity, With<LoadPopupOverlay>>,
    editor_entities: Query<
        Entity,
        Or<(
            With<EditorAsteroid>,
            With<EditorControlPoint>,
            With<EditorSpawn>,
        )>,
    >,
) {
    let mut chosen_file: Option<String> = None;
    for (interaction, file_opt) in &file_buttons {
        if *interaction == Interaction::Pressed {
            chosen_file = Some(file_opt.0.clone());
            break;
        }
    }

    let Some(filename) = chosen_file else {
        return;
    };

    let file_path = std::path::Path::new("assets/maps").join(&filename);
    match load_map_data(&file_path) {
        Ok(data) => {
            info!("Editor: loaded map from {}", filename);

            // Despawn all existing editor entities
            for entity in &editor_entities {
                commands.entity(entity).despawn();
            }

            // Clear selection
            editor.selected = None;

            // Update data and filename
            editor_data.0 = data.clone();
            editor_file.0 = Some(filename);

            // Update bounds
            commands.insert_resource(MapBounds {
                half_extents: Vec2::new(data.bounds.half_x, data.bounds.half_y),
            });

            // Respawn entities from new data
            for asteroid_def in &data.asteroids {
                let pos = Vec2::new(asteroid_def.position.0, asteroid_def.position.1);
                spawn_editor_asteroid(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    pos,
                    asteroid_def.radius,
                );
            }

            for cp_def in &data.control_points {
                let pos = Vec2::new(cp_def.position.0, cp_def.position.1);
                spawn_editor_control_point(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    pos,
                    cp_def.radius,
                );
            }

            for spawn_def in &data.spawns {
                let pos = Vec2::new(spawn_def.position.0, spawn_def.position.1);
                spawn_editor_spawn_point(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    pos,
                    spawn_def.team,
                );
            }
        }
        Err(e) => {
            warn!("Editor: failed to load '{}': {}", filename, e);
        }
    }

    // Close popup
    for entity in &popup_query {
        commands.entity(entity).despawn();
    }
}

// ── Close Popup on Escape ────────────────────────────────────────────────

fn close_popup_on_escape(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    load_popups: Query<Entity, With<LoadPopupOverlay>>,
    save_popups: Query<Entity, With<SavePopupOverlay>>,
) {
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    for entity in load_popups.iter().chain(save_popups.iter()) {
        commands.entity(entity).despawn();
    }
    commands.remove_resource::<SaveInputBuffer>();
}

fn handle_popup_cancel(
    mut commands: Commands,
    cancel_buttons: Query<&Interaction, (With<PopupCancelButton>, Changed<Interaction>)>,
    load_popups: Query<Entity, With<LoadPopupOverlay>>,
    save_popups: Query<Entity, With<SavePopupOverlay>>,
) {
    let any_pressed = cancel_buttons.iter().any(|i| *i == Interaction::Pressed);
    if !any_pressed {
        return;
    }
    for entity in load_popups.iter().chain(save_popups.iter()) {
        commands.entity(entity).despawn();
    }
    commands.remove_resource::<SaveInputBuffer>();
}

// ── UI Spawn ─────────────────────────────────────────────────────────────

fn spawn_editor_ui(mut commands: Commands) {
    commands
        .spawn((
            EditorUiRoot,
            Node {
                width: Val::Px(200.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.0)),
                row_gap: Val::Px(6.0),
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                ..default()
            },
            BackgroundColor(BG_DARK),
            Pickable::IGNORE,
            GlobalZIndex(5),
        ))
        .with_children(|panel| {
            // Title: ENTITIES
            panel.spawn((
                Text::new("ENTITIES"),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(TEXT_WHITE),
            ));

            // Tool buttons
            let tools = [
                EditorTool::Select,
                EditorTool::PlaceAsteroid,
                EditorTool::PlaceControlPoint,
                EditorTool::PlaceSpawn,
            ];

            for tool in tools {
                panel
                    .spawn((
                        ToolButton(tool),
                        Button,
                        Node {
                            width: Val::Percent(100.0),
                            padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(BG_BUTTON),
                    ))
                    .with_child((
                        Text::new(tool.label()),
                        TextFont {
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(TEXT_WHITE),
                    ));
            }

            // Spacer
            panel.spawn(Node {
                height: Val::Px(20.0),
                ..default()
            });

            // Title: FILE
            panel.spawn((
                Text::new("FILE"),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(TEXT_WHITE),
            ));

            // File name text
            panel.spawn((
                FileNameText,
                Text::new("Untitled"),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(TEXT_GRAY),
            ));

            // Save button
            panel
                .spawn((
                    SaveButton,
                    Button,
                    Node {
                        width: Val::Percent(100.0),
                        padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(BG_SAVE),
                ))
                .with_child((
                    Text::new("Save (Ctrl+S)"),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(TEXT_WHITE),
                ));

            // Load button
            panel
                .spawn((
                    LoadButton,
                    Button,
                    Node {
                        width: Val::Percent(100.0),
                        padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(BG_BUTTON),
                ))
                .with_child((
                    Text::new("Load (Ctrl+O)"),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(TEXT_WHITE),
                ));
        });

    // Bottom-left tool indicator
    commands.spawn((
        EditorToolIndicator,
        Text::new("SELECT"),
        TextFont {
            font_size: 16.0,
            ..default()
        },
        TextColor(TEXT_WHITE),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(220.0),
            bottom: Val::Px(8.0),
            ..default()
        },
        Pickable::IGNORE,
        GlobalZIndex(5),
    ));
}

// ── UI Updates ───────────────────────────────────────────────────────────

fn update_tool_buttons(
    editor: Res<EditorState>,
    mut buttons: Query<(&ToolButton, &mut BackgroundColor)>,
) {
    if !editor.is_changed() {
        return;
    }
    for (tool_btn, mut bg) in &mut buttons {
        if tool_btn.0 == editor.tool {
            *bg = BackgroundColor(BG_BUTTON_ACTIVE);
        } else {
            *bg = BackgroundColor(BG_BUTTON);
        }
    }
}

fn update_tool_indicator(
    editor: Res<EditorState>,
    mut indicators: Query<&mut Text, With<EditorToolIndicator>>,
) {
    if !editor.is_changed() {
        return;
    }
    for mut text in &mut indicators {
        **text = editor.tool.indicator_label().to_string();
    }
}

fn update_file_name_text(
    editor_file: Res<EditorFileName>,
    mut texts: Query<(&mut Text, &mut TextColor), With<FileNameText>>,
) {
    if !editor_file.is_changed() {
        return;
    }
    for (mut text, mut color) in &mut texts {
        match &editor_file.0 {
            Some(name) => {
                **text = name.clone();
                *color = TextColor(TEXT_WHITE);
            }
            None => {
                **text = "Untitled".to_string();
                *color = TextColor(TEXT_GRAY);
            }
        }
    }
}

// ── Gizmos ───────────────────────────────────────────────────────────────

fn draw_editor_bounds_gizmos(mut gizmos: Gizmos, bounds: Option<Res<MapBounds>>) {
    let Some(bounds) = bounds else {
        return;
    };

    let hx = bounds.half_extents.x;
    let hy = bounds.half_extents.y;
    let y = 0.5;
    let color = Color::srgb(0.0, 0.8, 0.8); // Cyan

    // Draw rectangle at Y=0.5
    let corners = [
        Vec3::new(-hx, y, -hy),
        Vec3::new(hx, y, -hy),
        Vec3::new(hx, y, hy),
        Vec3::new(-hx, y, hy),
    ];

    for i in 0..4 {
        gizmos.line(corners[i], corners[(i + 1) % 4], color);
    }
}

fn draw_editor_entity_gizmos(
    mut gizmos: Gizmos,
    editor: Res<EditorState>,
    control_points: Query<&Transform, With<EditorControlPoint>>,
    spawns: Query<(&Transform, &EditorSpawn)>,
    asteroids: Query<&Transform, With<EditorAsteroid>>,
    selected_transforms: Query<&Transform>,
    editor_data: Res<EditorMapData>,
) {
    // Control point radius circles
    for (i, tf) in control_points.iter().enumerate() {
        let radius = editor_data
            .0
            .control_points
            .get(i)
            .map(|cp| cp.radius)
            .unwrap_or(100.0);

        gizmos.circle(
            Isometry3d::new(
                Vec3::new(tf.translation.x, 0.5, tf.translation.z),
                Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            ),
            radius,
            Color::srgb(1.0, 1.0, 0.2),
        );
    }

    // Spawn point team indicators
    for (tf, spawn) in &spawns {
        let color = Team(spawn.0).color();
        gizmos.circle(
            Isometry3d::new(
                Vec3::new(tf.translation.x, 0.5, tf.translation.z),
                Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            ),
            15.0,
            color,
        );
    }

    // Selected entity highlight — size adapts to entity type
    if let Some(entity) = editor.selected {
        if let Ok(tf) = selected_transforms.get(entity) {
            // Determine selection ring radius based on entity type
            let sel_radius = if asteroids.get(entity).is_ok() {
                // Find matching asteroid radius in data
                let pos = Vec2::new(tf.translation.x, tf.translation.z);
                let asteroid_radius = editor_data
                    .0
                    .asteroids
                    .iter()
                    .find(|a| Vec2::new(a.position.0, a.position.1).distance(pos) < 1.0)
                    .map(|a| a.radius)
                    .unwrap_or(25.0);
                (asteroid_radius + 5.0).max(15.0)
            } else if control_points.get(entity).is_ok() {
                let pos = Vec2::new(tf.translation.x, tf.translation.z);
                let cp_radius = editor_data
                    .0
                    .control_points
                    .iter()
                    .find(|c| Vec2::new(c.position.0, c.position.1).distance(pos) < 1.0)
                    .map(|c| c.radius)
                    .unwrap_or(100.0);
                (cp_radius + 10.0).max(15.0)
            } else {
                // Spawn point or unknown — fixed size
                15.0
            };

            gizmos.circle(
                Isometry3d::new(
                    Vec3::new(tf.translation.x, 1.0, tf.translation.z),
                    Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                ),
                sel_radius,
                Color::srgb(0.0, 1.0, 0.0),
            );
        }
    }
}
