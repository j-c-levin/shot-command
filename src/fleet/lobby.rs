// Fleet lobby — pre-game fleet composition and readiness synchronization.
//
// The server stays in `WaitingForPlayers` state while lobby systems run.
// Once both players submit valid fleets and the countdown completes,
// the lobby transitions the server to `Playing`.

use std::collections::HashMap;

use bevy::prelude::*;
use bevy_replicon::prelude::*;
use bevy_replicon::shared::message::client_message::FromClient;

use crate::fleet::{validate_fleet, ShipSpec};
use crate::game::GameState;
use crate::net::commands::{
    CancelSubmission, FleetSubmission, GameStarted, LobbyState, LobbyStatus,
};
use crate::net::server::ClientTeams;

/// Tracks fleet submissions and lobby countdown on the server.
#[derive(Resource, Debug, Default)]
pub struct LobbyTracker {
    /// Mapping from client entity to their validated fleet specs.
    pub submissions: HashMap<Entity, Vec<ShipSpec>>,
    /// Countdown timer (seconds remaining). `Some` when both players have submitted.
    pub countdown: Option<f32>,
}

pub struct LobbyPlugin;

impl Plugin for LobbyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LobbyTracker>();
        app.add_observer(handle_fleet_submission);
        app.add_observer(handle_cancel_submission);
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
    client_teams: Res<ClientTeams>,
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
    info!(
        "Fleet submission accepted from {:?}. Total submissions: {}",
        client_entity,
        lobby.submissions.len()
    );

    if lobby.submissions.len() >= 2 {
        // Both players have submitted — start countdown
        lobby.countdown = Some(3.0);
        commands.server_trigger(ToClients {
            mode: SendMode::Broadcast,
            message: LobbyStatus {
                state: LobbyState::Countdown(3.0),
            },
        });
    } else {
        // Only this player submitted
        commands.server_trigger(ToClients {
            mode: SendMode::Direct(ClientId::Client(client_entity)),
            message: LobbyStatus {
                state: LobbyState::WaitingForOpponent,
            },
        });

        // Notify the other player (if connected) that their opponent has submitted
        for &other_entity in client_teams.map.keys() {
            if other_entity != client_entity {
                commands.server_trigger(ToClients {
                    mode: SendMode::Direct(ClientId::Client(other_entity)),
                    message: LobbyStatus {
                        state: LobbyState::OpponentComposing,
                    },
                });
            }
        }
    }
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

    info!("Fleet submission cancelled by {:?}", client_entity);

    // Tell the canceller they're back to composing
    commands.server_trigger(ToClients {
        mode: SendMode::Direct(ClientId::Client(client_entity)),
        message: LobbyStatus {
            state: LobbyState::Composing,
        },
    });

    // Notify any other player who has submitted that their opponent is still composing
    for (&other_entity, _) in &lobby.submissions {
        if other_entity != client_entity {
            commands.server_trigger(ToClients {
                mode: SendMode::Direct(ClientId::Client(other_entity)),
                message: LobbyStatus {
                    state: LobbyState::OpponentComposing,
                },
            });
        }
    }
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
        info!("Lobby countdown complete — starting game");
        commands.server_trigger(ToClients {
            mode: SendMode::Broadcast,
            message: GameStarted,
        });
        next_state.set(GameState::Playing);
    } else {
        let secs = *remaining;
        commands.server_trigger(ToClients {
            mode: SendMode::Broadcast,
            message: LobbyStatus {
                state: LobbyState::Countdown(secs),
            },
        });
    }
}
