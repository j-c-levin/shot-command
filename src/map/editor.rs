use bevy::prelude::*;

/// Optional resource: path to load a map file in the editor.
#[derive(Resource, Debug, Clone)]
pub struct EditorMapPath(pub String);

pub struct MapEditorPlugin;

impl Plugin for MapEditorPlugin {
    fn build(&self, _app: &mut App) {
        // Will be filled in subsequent tasks
    }
}
