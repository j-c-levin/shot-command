pub mod fleet_builder;

use bevy::prelude::*;

use crate::game::GameState;
use fleet_builder::*;

pub struct FleetUiPlugin;

impl Plugin for FleetUiPlugin {
    fn build(&self, app: &mut App) {
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
                    update_status_text,
                    update_submit_button,
                    handle_add_ship_button,
                    handle_ship_entry_click,
                    handle_remove_ship_button,
                    handle_ship_picker_option,
                    handle_change_weapon_button,
                    handle_remove_weapon_button,
                    handle_weapon_picker_option,
                    handle_submit_button,
                    handle_popup_close,
                )
                    .run_if(in_state(GameState::FleetComposition)),
            );
    }
}
