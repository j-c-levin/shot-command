use bevy::{asset::AssetMetaCheck, prelude::*};
use bevy_replicon::prelude::*;
use bevy_replicon_renet::RepliconRenetPlugins;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "nebulous-client")]
struct Cli {
    /// Server address to connect to
    #[arg(long, default_value = "127.0.0.1:5000")]
    connect: String,
}

fn main() {
    let cli = Cli::parse();
    info!("Client connecting to: {}", cli.connect);

    App::new()
        .add_plugins((
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
        ))
        .add_systems(Startup, || {
            info!("Client started (stub)");
        })
        .run();
}
