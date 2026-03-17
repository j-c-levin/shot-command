use bevy::{asset::AssetMetaCheck, prelude::*};
use bevy_replicon::prelude::*;
use bevy_replicon_renet::RepliconRenetPlugins;
use clap::Parser;

use nebulous_shot_command::camera::CameraPlugin;
use nebulous_shot_command::control_point::ControlPointClientPlugin;
use nebulous_shot_command::fleet::{FleetPlugin, ShipSpec};
use nebulous_shot_command::fog::FogClientPlugin;
use nebulous_shot_command::game::{GamePlugin, GameState};
use nebulous_shot_command::input::InputPlugin;
use nebulous_shot_command::net::client::{ClientConnectAddress, ClientNetPlugin};
use nebulous_shot_command::net::commands::FleetSubmission;
use nebulous_shot_command::net::SharedReplicationPlugin;
use nebulous_shot_command::net::LocalTeam;
use nebulous_shot_command::radar::RadarClientPlugin;
use nebulous_shot_command::ship::{ShipClass, ShipVisualsPlugin};
use nebulous_shot_command::ui::FleetUiPlugin;
use nebulous_shot_command::weapon::WeaponType;

#[derive(Parser, Debug)]
#[command(name = "nebulous-client")]
struct Cli {
    /// Server address to connect to
    #[arg(long, default_value = "127.0.0.1:5000")]
    connect: String,

    /// Auto-submit a preset fleet (1 or 2) to skip the fleet builder
    #[arg(long)]
    fleet: Option<u8>,
}

/// Resource: if set, auto-submit this fleet on entering FleetComposition.
#[derive(Resource)]
struct AutoFleet(Vec<ShipSpec>);

fn preset_fleet(id: u8) -> Vec<ShipSpec> {
    match id {
        1 => vec![
            // Battleship: Railgun, HeavyVLS, SearchRadar, Cannon, CWIS, CWIS
            ShipSpec {
                class: ShipClass::Battleship,
                loadout: vec![
                    Some(WeaponType::Railgun),
                    Some(WeaponType::HeavyVLS),
                    Some(WeaponType::SearchRadar),
                    Some(WeaponType::Cannon),
                    Some(WeaponType::CWIS),
                    Some(WeaponType::CWIS),
                ],
            },
        ],
        2 => vec![
            // Scout: NavRadar, CWIS
            ShipSpec {
                class: ShipClass::Scout,
                loadout: vec![
                    Some(WeaponType::NavRadar),
                    Some(WeaponType::CWIS),
                ],
            },
        ],
        _ => {
            eprintln!("Unknown fleet preset {id}, using default loadouts");
            vec![
                ShipSpec {
                    class: ShipClass::Battleship,
                    loadout: ShipClass::Battleship.default_loadout(),
                },
                ShipSpec {
                    class: ShipClass::Destroyer,
                    loadout: ShipClass::Destroyer.default_loadout(),
                },
                ShipSpec {
                    class: ShipClass::Scout,
                    loadout: ShipClass::Scout.default_loadout(),
                },
            ]
        }
    }
}

fn main() {
    let cli = Cli::parse();

    let mut app = App::new();
    app.add_plugins((
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Nebulous Shot Command".to_string(),
                        fit_canvas_to_parent: true,
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin {
                    meta_check: AssetMetaCheck::Never,
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
            MeshPickingPlugin,
            RepliconPlugins,
            RepliconRenetPlugins,
            SharedReplicationPlugin,
            GamePlugin,
            FleetPlugin,
            CameraPlugin,
            ShipVisualsPlugin,
            FogClientPlugin,
            InputPlugin,
            FleetUiPlugin,
            RadarClientPlugin,
            ControlPointClientPlugin,
            ClientNetPlugin,
        ));
    app.insert_resource(ClientConnectAddress(cli.connect));

    if let Some(fleet_id) = cli.fleet {
        app.insert_resource(AutoFleet(preset_fleet(fleet_id)));
        app.add_systems(OnEnter(GameState::FleetComposition), auto_submit_fleet);
    }

    app.init_resource::<LocalTeam>()
        .add_systems(Startup, set_connecting)
        .run();
}

/// Transition from the default Setup state to Connecting on startup.
fn set_connecting(mut next_state: ResMut<NextState<GameState>>) {
    next_state.set(GameState::Connecting);
}

/// Auto-submit the preset fleet when entering FleetComposition.
fn auto_submit_fleet(
    mut commands: Commands,
    auto_fleet: Res<AutoFleet>,
) {
    use bevy_replicon::shared::message::client_event::ClientTriggerExt;
    info!("Auto-submitting preset fleet ({} ships)", auto_fleet.0.len());
    commands.client_trigger(FleetSubmission {
        ships: auto_fleet.0.clone(),
    });
}
