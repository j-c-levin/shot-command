// Fleet lobby — pre-game fleet composition and readiness synchronization.
//
// The server stays in `WaitingForPlayers` state while lobby systems run.
// The game creator sends a `LaunchCommand` to start the countdown once
// every team has at least one submitted fleet.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use bevy_replicon::prelude::*;
use bevy_replicon::shared::message::client_message::FromClient;

use crate::fleet::{validate_fleet, ShipSpec};
use crate::game::{GameConfig, GameState, Team};
use crate::net::commands::{
    CancelSubmission, FleetSubmission, GameStarted, LaunchCommand, LobbyState, LobbyStatus,
};
use crate::net::server::ClientTeams;

/// Tracks fleet submissions and lobby countdown on the server.
#[derive(Resource, Debug, Default)]
pub struct LobbyTracker {
    /// Mapping from client entity to their validated fleet specs.
    pub submissions: HashMap<Entity, Vec<ShipSpec>>,
    /// Countdown timer (seconds remaining). `Some` when creator has launched.
    pub countdown: Option<f32>,
    /// Last broadcast second (to avoid broadcasting every frame). -1 means no broadcast yet.
    pub last_broadcast_secs: i32,
}

pub struct LobbyPlugin;

impl Plugin for LobbyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LobbyTracker>();
        app.add_observer(handle_fleet_submission);
        app.add_observer(handle_cancel_submission);
        app.add_observer(handle_launch_command);
        app.add_systems(
            Update,
            tick_lobby_countdown.run_if(in_state(GameState::WaitingForPlayers)),
        );
    }
}

/// Observer: handle `FleetSubmission` from clients.
fn handle_fleet_submission(
    trigger: On<FromClient<FleetSubmission>>,
    mut commands: Commands,
    mut lobby: ResMut<LobbyTracker>,
    config: Res<GameConfig>,
) {
    let from = trigger.event();
    let submission = &from.message;

    let client_entity = match from.client_id {
        ClientId::Client(e) => e,
        ClientId::Server => return,
    };

    // Validate the fleet
    if let Err(err) = validate_fleet(&submission.ships) {
        info!(
            "Fleet submission from {:?} rejected: {:?}",
            client_entity, err
        );
        commands.server_trigger(ToClients {
            mode: SendMode::Direct(ClientId::Client(client_entity)),
            message: LobbyStatus {
                state: LobbyState::Rejected(format!("{:?}", err)),
            },
        });
        return;
    }

    // Store the valid submission
    lobby.submissions.insert(client_entity, submission.ships.clone());
    let count = lobby.submissions.len();
    info!(
        "Fleet submission accepted from {:?}. Total submissions: {}/{}",
        client_entity,
        count,
        config.max_players()
    );

    // Tell the submitter they're waiting
    commands.server_trigger(ToClients {
        mode: SendMode::Direct(ClientId::Client(client_entity)),
        message: LobbyStatus {
            state: LobbyState::WaitingForOpponent,
        },
    });

    // Broadcast current submission count to all clients
    commands.server_trigger(ToClients {
        mode: SendMode::Broadcast,
        message: LobbyStatus {
            state: LobbyState::SubmissionCount(count as u32),
        },
    });
}

/// Observer: handle `CancelSubmission` from clients.
fn handle_cancel_submission(
    trigger: On<FromClient<CancelSubmission>>,
    mut commands: Commands,
    mut lobby: ResMut<LobbyTracker>,
) {
    let from = trigger.event();

    let client_entity = match from.client_id {
        ClientId::Client(e) => e,
        ClientId::Server => return,
    };

    lobby.submissions.remove(&client_entity);
    lobby.countdown = None;
    lobby.last_broadcast_secs = -1;

    info!("Fleet submission cancelled by {:?}", client_entity);

    // Tell the canceller they're back to composing
    commands.server_trigger(ToClients {
        mode: SendMode::Direct(ClientId::Client(client_entity)),
        message: LobbyStatus {
            state: LobbyState::Composing,
        },
    });

    // Broadcast updated submission count to all clients
    let count = lobby.submissions.len() as u32;
    commands.server_trigger(ToClients {
        mode: SendMode::Broadcast,
        message: LobbyStatus {
            state: LobbyState::SubmissionCount(count),
        },
    });
}

/// Observer: handle `LaunchCommand` from the game creator.
pub fn handle_launch_command(
    trigger: On<FromClient<LaunchCommand>>,
    mut commands: Commands,
    mut lobby: ResMut<LobbyTracker>,
    client_teams: Res<ClientTeams>,
    config: Res<GameConfig>,
) {
    let from = trigger.event();
    let client_entity = match from.client_id {
        ClientId::Client(e) => e,
        ClientId::Server => return,
    };

    // Creator is Team(0) slot 0
    let is_creator = client_teams
        .map
        .get(&client_entity)
        .map(|s| s.team == Team(0) && s.slot == 0)
        .unwrap_or(false);

    if !is_creator {
        warn!(
            "LaunchCommand from non-creator {:?}, ignoring",
            client_entity
        );
        commands.server_trigger(ToClients {
            mode: SendMode::Direct(ClientId::Client(client_entity)),
            message: LobbyStatus {
                state: LobbyState::Rejected("Only the game creator can launch".to_string()),
            },
        });
        return;
    }

    // Check: every team 0..config.team_count has at least 1 submission
    let mut teams_with_submissions: HashSet<u8> = HashSet::new();
    for &sub_entity in lobby.submissions.keys() {
        if let Some(slot) = client_teams.map.get(&sub_entity) {
            teams_with_submissions.insert(slot.team.0);
        }
    }

    let missing_teams: Vec<u8> = (0..config.team_count)
        .filter(|t| !teams_with_submissions.contains(t))
        .collect();

    if !missing_teams.is_empty() {
        let msg = format!("Teams without submissions: {:?}", missing_teams);
        info!("Launch rejected: {}", msg);
        commands.server_trigger(ToClients {
            mode: SendMode::Direct(ClientId::Client(client_entity)),
            message: LobbyStatus {
                state: LobbyState::Rejected(msg),
            },
        });
        return;
    }

    // All teams have at least 1 submission — start countdown
    lobby.countdown = Some(3.0);
    lobby.last_broadcast_secs = -1;
    info!("Game creator launched — starting 3s countdown");
    commands.server_trigger(ToClients {
        mode: SendMode::Broadcast,
        message: LobbyStatus {
            state: LobbyState::Countdown(3.0),
        },
    });
}

/// Tick the lobby countdown. When it reaches zero, broadcast `GameStarted`
/// and transition the server to `Playing`.
fn tick_lobby_countdown(
    mut commands: Commands,
    time: Res<Time>,
    mut lobby: ResMut<LobbyTracker>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    let Some(remaining) = lobby.countdown.as_mut() else {
        return;
    };

    *remaining -= time.delta_secs();

    if *remaining <= 0.0 {
        lobby.countdown = None;
        lobby.last_broadcast_secs = -1;
        info!("Lobby countdown complete — starting game");
        commands.server_trigger(ToClients {
            mode: SendMode::Broadcast,
            message: GameStarted,
        });
        next_state.set(GameState::Playing);
    } else {
        let secs = *remaining;
        let display_secs = secs.ceil() as i32;
        if display_secs != lobby.last_broadcast_secs {
            lobby.last_broadcast_secs = display_secs;
            commands.server_trigger(ToClients {
                mode: SendMode::Broadcast,
                message: LobbyStatus {
                    state: LobbyState::Countdown(secs),
                },
            });
        }
    }
}
