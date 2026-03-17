use std::collections::{HashMap, HashSet};

use bevy::math::Vec2;
use bevy::prelude::*;
use bevy::time::Timer;
use bevy_replicon::prelude::*;

use crate::game::{Destroyed, DestroyTimer, GameState, Health, Team};
use crate::net::commands::GameResult;
use crate::ship::{Ship, ShipSecrets, ShipSecretsOwner};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitZone {
    Front,     // ±45° from nose. Primary: Hull.
    Rear,      // ±45° from tail (135–180°). Primary: Engines.
    Broadside, // 45–135° from nose. Primary: Component.
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageTarget {
    Hull,
    Engines,
    Component,     // randomly chosen mounted component
    HullOrEngines, // randomly pick hull or engines
}

/// Classify hit zone from incoming direction and ship forward.
/// impact_dir: normalized direction projectile was traveling.
/// ship_forward: normalized facing direction (XZ plane).
pub fn classify_hit_zone(impact_dir: Vec2, ship_forward: Vec2) -> HitZone {
    let from_attacker = -impact_dir.normalize_or_zero();
    let fwd = ship_forward.normalize_or_zero();
    let cos_angle = from_attacker.dot(fwd).clamp(-1.0, 1.0);
    let angle = cos_angle.acos(); // 0 = nose-on, PI = tail-on

    const FRONT_MAX: f32 = std::f32::consts::FRAC_PI_4; // 45°
    const REAR_MIN: f32 = 3.0 * std::f32::consts::FRAC_PI_4; // 135°

    if angle < FRONT_MAX {
        HitZone::Front
    } else if angle > REAR_MIN {
        HitZone::Rear
    } else {
        HitZone::Broadside
    }
}

/// Return (primary_target, primary_damage, secondary_target, secondary_damage).
/// 70% to primary, 30% to secondary. Total always equals raw_damage.
pub fn route_damage(zone: HitZone, raw_damage: u16) -> (DamageTarget, u16, DamageTarget, u16) {
    let primary_dmg = raw_damage * 7 / 10;
    let secondary_dmg = raw_damage - primary_dmg;

    match zone {
        HitZone::Front => (
            DamageTarget::Hull,
            primary_dmg,
            DamageTarget::Component,
            secondary_dmg,
        ),
        HitZone::Rear => (
            DamageTarget::Engines,
            primary_dmg,
            DamageTarget::Component,
            secondary_dmg,
        ),
        HitZone::Broadside => (
            DamageTarget::Component,
            primary_dmg,
            DamageTarget::HullOrEngines,
            secondary_dmg,
        ),
    }
}

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
    let mut alive_counts: HashMap<u8, u32> = HashMap::new();
    let mut teams_seen: HashSet<u8> = HashSet::new();

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

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::math::Vec2;

    #[test]
    fn front_hit_nose_on() {
        let ship_forward = Vec2::new(0.0, 1.0);
        let impact_dir = Vec2::new(0.0, -1.0);
        assert_eq!(classify_hit_zone(impact_dir, ship_forward), HitZone::Front);
    }

    #[test]
    fn rear_hit_tail_on() {
        let ship_forward = Vec2::new(0.0, 1.0);
        let impact_dir = Vec2::new(0.0, 1.0);
        assert_eq!(classify_hit_zone(impact_dir, ship_forward), HitZone::Rear);
    }

    #[test]
    fn broadside_hit_right() {
        let ship_forward = Vec2::new(0.0, 1.0);
        let impact_dir = Vec2::new(-1.0, 0.0);
        assert_eq!(
            classify_hit_zone(impact_dir, ship_forward),
            HitZone::Broadside
        );
    }

    #[test]
    fn broadside_hit_left() {
        let ship_forward = Vec2::new(0.0, 1.0);
        let impact_dir = Vec2::new(1.0, 0.0);
        assert_eq!(
            classify_hit_zone(impact_dir, ship_forward),
            HitZone::Broadside
        );
    }

    #[test]
    fn front_boundary_44_degrees() {
        let angle_rad = 44_f32.to_radians();
        let ship_forward = Vec2::new(0.0, 1.0);
        let from_attacker = Vec2::new(angle_rad.sin(), angle_rad.cos());
        let impact_dir = -from_attacker;
        assert_eq!(classify_hit_zone(impact_dir, ship_forward), HitZone::Front);
    }

    #[test]
    fn rear_boundary_136_degrees() {
        let angle_rad = 136_f32.to_radians();
        let ship_forward = Vec2::new(0.0, 1.0);
        let from_attacker = Vec2::new(angle_rad.sin(), angle_rad.cos());
        let impact_dir = -from_attacker;
        assert_eq!(classify_hit_zone(impact_dir, ship_forward), HitZone::Rear);
    }

    #[test]
    fn front_damage_split_70_30() {
        let (primary, primary_dmg, _secondary, secondary_dmg) =
            route_damage(HitZone::Front, 100);
        assert_eq!(primary, DamageTarget::Hull);
        assert_eq!(primary_dmg, 70);
        assert_eq!(secondary_dmg, 30);
    }

    #[test]
    fn rear_damage_split_70_30() {
        let (primary, primary_dmg, _secondary, secondary_dmg) =
            route_damage(HitZone::Rear, 100);
        assert_eq!(primary, DamageTarget::Engines);
        assert_eq!(primary_dmg, 70);
        assert_eq!(secondary_dmg, 30);
    }

    #[test]
    fn broadside_damage_split_70_30() {
        let (primary, primary_dmg, _secondary, secondary_dmg) =
            route_damage(HitZone::Broadside, 100);
        assert_eq!(primary, DamageTarget::Component);
        assert_eq!(primary_dmg, 70);
        assert_eq!(secondary_dmg, 30);
    }

    #[test]
    fn damage_split_totals_match_raw() {
        for raw in [1u16, 2, 3, 10, 15, 99, 100] {
            let (_, p, _, s) = route_damage(HitZone::Front, raw);
            assert_eq!(p + s, raw, "total must equal raw for damage={raw}");
        }
    }
}
