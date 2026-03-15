use bevy::{app::ScheduleRunnerPlugin, prelude::*};
use bevy_replicon::prelude::*;
use bevy_replicon_renet::RepliconRenetPlugins;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "nebulous-server")]
struct Cli {
    /// Address to bind the server to
    #[arg(long, default_value = "127.0.0.1:5000")]
    bind: String,
}

fn main() {
    let cli = Cli::parse();
    info!("Server bind address: {}", cli.bind);

    App::new()
        .add_plugins((
            MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(
                std::time::Duration::from_secs_f64(1.0 / 60.0),
            )),
            RepliconPlugins,
            RepliconRenetPlugins,
        ))
        .add_systems(Startup, || {
            info!("Server started (stub — exiting)");
        })
        .run();
}
