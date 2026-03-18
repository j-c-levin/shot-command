pub mod fleet_builder;
pub mod fleet_status;

use bevy::prelude::*;

use crate::game::GameState;
use fleet_builder::*;
pub use fleet_status::FleetStatusPlugin;

pub struct FleetUiPlugin;

impl Plugin for FleetUiPlugin {
    fn build(&self, app: &mut App) {
        let fleet_builder_active = in_state(GameState::FleetComposition)
            .or(in_state(GameState::GameLobby));

        app.init_resource::<FleetBuilderState>()
            .add_systems(OnEnter(GameState::FleetComposition), spawn_fleet_ui)
            .add_systems(OnExit(GameState::FleetComposition), despawn_fleet_ui)
            .add_systems(
                Update,
                (
                    rebuild_fleet_list,
                    rebuild_ship_detail,
                    spawn_popup,
                    update_budget_text,
                    update_submit_button,
                    handle_add_ship_button,
                    handle_ship_entry_click,
                    handle_remove_ship_button,
                )
                    .run_if(fleet_builder_active.clone()),
            )
            .add_systems(
                Update,
                (
                    handle_clone_ship_button,
                    handle_ship_picker_option,
                    handle_change_weapon_button,
                    handle_remove_weapon_button,
                    handle_weapon_picker_option,
                    handle_submit_button,
                    handle_popup_close,
                    handle_save_fleet,
                    handle_save_input,
                    handle_save_confirm,
                    handle_load_fleet,
                    handle_load_fleet_option,
                    handle_delete_fleet,
                )
                    .run_if(fleet_builder_active.clone()),
            )
            .add_systems(
                Update,
                update_status_text
                    .run_if(in_state(GameState::FleetComposition)),
            );
    }
}
