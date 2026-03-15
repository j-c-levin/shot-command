use bevy::prelude::*;
use bevy::time::Timer;

use crate::game::{Destroyed, DestroyTimer, GameState, Health, Team};
use crate::ship::{Ship, ShipSecrets, ShipSecretsOwner};

pub struct DamagePlugin;

impl Plugin for DamagePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (mark_destroyed, despawn_destroyed, check_win_condition)
                .chain()
                .run_if(in_state(GameState::Playing)),
        );
    }
}

/// Mark ships with 0 HP as destroyed, inserting a delay timer before despawn.
fn mark_destroyed(
    mut commands: Commands,
    query: Query<(Entity, &Health, &Team, &Ship), Without<Destroyed>>,
) {
    for (entity, health, team, _ship) in &query {
        if health.hp == 0 {
            commands.entity(entity).insert((
                Destroyed,
                DestroyTimer(Timer::from_seconds(1.0, TimerMode::Once)),
            ));
            info!("Ship {:?} (Team {}) destroyed!", entity, team.0);
        }
    }
}

/// After the destroy timer elapses, despawn the ship and its ShipSecrets entity.
fn despawn_destroyed(
    mut commands: Commands,
    time: Res<Time>,
    mut ship_query: Query<(Entity, &mut DestroyTimer), With<Destroyed>>,
    secrets_query: Query<(Entity, &ShipSecretsOwner), With<ShipSecrets>>,
) {
    for (ship_entity, mut timer) in &mut ship_query {
        timer.0.tick(time.delta());
        if timer.0.is_finished() {
            // Find and despawn the associated ShipSecrets entity
            for (secrets_entity, owner) in &secrets_query {
                if owner.0 == ship_entity {
                    commands.entity(secrets_entity).despawn();
                    break;
                }
            }
            commands.entity(ship_entity).despawn();
            info!("Despawned destroyed ship {:?}", ship_entity);
        }
    }
}

/// Check if all ships of a team are destroyed. If so, the other team wins.
fn check_win_condition(
    mut commands: Commands,
    mut next_state: ResMut<NextState<GameState>>,
    ships: Query<(&Team, Option<&Destroyed>), With<Ship>>,
) {
    use std::collections::HashMap;

    use bevy_replicon::prelude::*;

    use crate::net::commands::GameResult;

    let mut alive_counts: HashMap<u8, u32> = HashMap::new();
    let mut teams_seen: std::collections::HashSet<u8> = std::collections::HashSet::new();

    for (team, destroyed) in &ships {
        teams_seen.insert(team.0);
        if destroyed.is_none() {
            *alive_counts.entry(team.0).or_insert(0) += 1;
        }
    }

    // Only check once both teams have been spawned (at least seen)
    if teams_seen.len() < 2 {
        return;
    }

    let team0_alive = alive_counts.get(&0).copied().unwrap_or(0);
    let team1_alive = alive_counts.get(&1).copied().unwrap_or(0);

    let winner = if team0_alive == 0 {
        Some(Team(1))
    } else if team1_alive == 0 {
        Some(Team(0))
    } else {
        None
    };

    if let Some(winning_team) = winner {
        info!("Team {} wins! All enemy ships destroyed.", winning_team.0);
        commands.server_trigger(ToClients {
            mode: SendMode::Broadcast,
            message: GameResult { winning_team },
        });
        next_state.set(GameState::GameOver);
    }
}
