use std::collections::{HashMap, HashSet};

use bevy::math::Vec2;
use bevy::prelude::*;
use bevy::time::Timer;
use bevy_replicon::prelude::*;
use rand::prelude::IndexedRandom;
use rand::Rng;

use crate::game::{Destroyed, DestroyTimer, GameState, Health, Team};
use crate::net::commands::GameResult;
use crate::ship::{EngineHealth, RepairCooldown, Ship, ShipSecrets, ShipSecretsOwner};
use crate::weapon::Mounts;

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

/// Offline cooldown by mount size: Small=10s, Medium=15s, Large=20s.
pub fn offline_cooldown_secs(size: crate::weapon::MountSize) -> f32 {
    match size {
        crate::weapon::MountSize::Small => 10.0,
        crate::weapon::MountSize::Medium => 15.0,
        crate::weapon::MountSize::Large => 20.0,
    }
}
/// Engine offline cooldown (engines are large systems).
pub const ENGINE_OFFLINE_COOLDOWN_SECS: f32 = 15.0;
pub const REPAIR_DELAY_SECS: f32 = 5.0;
pub const REPAIR_RATE_HP_PER_SEC: f32 = 20.0;

/// Apply directional damage to a ship's HP pools.
/// If `is_railgun` is true, damage bypasses normal zone routing and goes
/// 90% to a random component, 10% to hull (precision strike).
pub fn apply_damage_to_ship(
    impact_dir: Vec2,
    ship_forward: Vec2,
    raw_damage: u16,
    is_railgun: bool,
    health: &mut Health,
    engine_health: &mut EngineHealth,
    mounts: &mut Mounts,
    repair_cooldown: &mut RepairCooldown,
) {
    let (primary_target, primary_dmg, secondary_target, secondary_dmg) = if is_railgun {
        // Railgun: precision component strike, token hull damage
        let comp_dmg = raw_damage * 9 / 10;
        let hull_dmg = raw_damage - comp_dmg;
        (DamageTarget::Component, comp_dmg, DamageTarget::Hull, hull_dmg)
    } else {
        let zone = classify_hit_zone(impact_dir, ship_forward);
        route_damage(zone, raw_damage)
    };

    apply_to_target(primary_target, primary_dmg, health, engine_health, mounts);
    apply_to_target(secondary_target, secondary_dmg, health, engine_health, mounts);

    // Reset repair cooldown on any hit
    repair_cooldown.0 = REPAIR_DELAY_SECS;
}

fn apply_to_target(
    target: DamageTarget,
    damage: u16,
    health: &mut Health,
    engine_health: &mut EngineHealth,
    mounts: &mut Mounts,
) {
    if damage == 0 {
        return;
    }
    match target {
        DamageTarget::Hull => {
            health.hp = health.hp.saturating_sub(damage);
        }
        DamageTarget::Engines => {
            if engine_health.hp > 0 {
                engine_health.hp = engine_health.hp.saturating_sub(damage);
                if engine_health.hp == 0 {
                    engine_health.offline_timer = ENGINE_OFFLINE_COOLDOWN_SECS;
                }
            } else {
                // Engines already down — spill to hull
                health.hp = health.hp.saturating_sub(damage);
            }
        }
        DamageTarget::Component => {
            let mut rng = rand::rng();
            let candidates: Vec<usize> = mounts
                .0
                .iter()
                .enumerate()
                .filter(|(_, m)| m.weapon.is_some() && m.max_hp > 0 && m.offline_timer <= 0.0)
                .map(|(i, _)| i)
                .collect();
            if let Some(&idx) = candidates.choose(&mut rng) {
                let mount = &mut mounts.0[idx];
                mount.hp = mount.hp.saturating_sub(damage);
                if mount.hp == 0 {
                    mount.offline_timer = offline_cooldown_secs(mount.size);
                }
            } else {
                health.hp = health.hp.saturating_sub(damage);
            }
        }
        DamageTarget::HullOrEngines => {
            let mut rng = rand::rng();
            if rng.random_bool(0.5) {
                health.hp = health.hp.saturating_sub(damage);
            } else if engine_health.hp > 0 {
                engine_health.hp = engine_health.hp.saturating_sub(damage);
                if engine_health.hp == 0 {
                    engine_health.offline_timer = ENGINE_OFFLINE_COOLDOWN_SECS;
                }
            } else {
                health.hp = health.hp.saturating_sub(damage);
            }
        }
    }
}

fn tick_repair(
    time: Res<Time>,
    mut query: Query<
        (&mut EngineHealth, &mut Mounts, &mut RepairCooldown),
        (With<Ship>, Without<Destroyed>),
    >,
) {
    let dt = time.delta_secs();
    for (mut engine_health, mut mounts, mut repair_cooldown) in &mut query {
        // Tick repair cooldown
        if repair_cooldown.0 > 0.0 {
            repair_cooldown.0 = (repair_cooldown.0 - dt).max(0.0);
        }
        let repair_active = repair_cooldown.0 <= 0.0;

        // --- Engine health ---
        if engine_health.hp == 0 && engine_health.offline_timer > 0.0 {
            engine_health.offline_timer = (engine_health.offline_timer - dt).max(0.0);
            if engine_health.offline_timer <= 0.0 {
                engine_health.hp = engine_health.floor();
            }
        } else if repair_active && engine_health.hp > 0 {
            let floor = engine_health.floor();
            if engine_health.hp < floor {
                let healed = (REPAIR_RATE_HP_PER_SEC * dt) as u16;
                engine_health.hp = (engine_health.hp + healed).min(floor);
            }
        }

        // --- Mount component health ---
        for mount in mounts.0.iter_mut() {
            if mount.max_hp == 0 {
                continue;
            }
            let floor = (mount.max_hp / 10).max(1);

            if mount.hp == 0 && mount.offline_timer > 0.0 {
                mount.offline_timer = (mount.offline_timer - dt).max(0.0);
                if mount.offline_timer <= 0.0 {
                    mount.hp = floor;
                }
            } else if repair_active && mount.hp > 0 && mount.hp < floor {
                let healed = (REPAIR_RATE_HP_PER_SEC * dt) as u16;
                mount.hp = (mount.hp + healed).min(floor);
            }
        }
    }
}

pub struct DamagePlugin;

impl Plugin for DamagePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (tick_repair, mark_destroyed, despawn_destroyed, check_win_condition)
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

    #[test]
    fn engine_offline_timer_ticks_to_restore() {
        use crate::ship::EngineHealth;
        let mut eh = EngineHealth::new(300);
        eh.hp = 0;
        eh.offline_timer = 0.05;

        let dt = 0.1_f32;
        eh.offline_timer = (eh.offline_timer - dt).max(0.0);
        if eh.offline_timer <= 0.0 {
            eh.hp = eh.floor();
        }

        assert_eq!(eh.hp, 30);
        assert_eq!(eh.offline_timer, 0.0);
    }

    #[test]
    fn mount_restores_to_floor_after_offline() {
        use crate::weapon::{Mount, MountSize, WeaponState, WeaponType};
        let profile = WeaponType::Cannon.profile();
        let mut mount = Mount {
            size: MountSize::Medium,
            offset: bevy::math::Vec2::ZERO,
            weapon: Some(WeaponState {
                weapon_type: WeaponType::Cannon,
                ammo: 0,
                cooldown: 0.0,
                pd_retarget_cooldown: 0.0,
                tubes_loaded: profile.tubes,
                tube_reload_timer: 0.0,
                fire_delay: 0.0,
            }),
            hp: 0,
            max_hp: 100,
            offline_timer: 0.05,
        };

        let dt = 0.1_f32;
        mount.offline_timer = (mount.offline_timer - dt).max(0.0);
        if mount.offline_timer <= 0.0 {
            let floor = (mount.max_hp / 10).max(1);
            mount.hp = floor;
        }

        assert_eq!(mount.hp, 10);
    }

    #[test]
    fn repair_heals_toward_floor_not_above() {
        use crate::ship::EngineHealth;
        let mut eh = EngineHealth::new(300);
        eh.hp = 25;
        let floor = eh.floor();

        let healed = (REPAIR_RATE_HP_PER_SEC * 1.0) as u16;
        eh.hp = (eh.hp + healed).min(floor);

        assert!(eh.hp <= floor);
        assert_eq!(eh.hp, floor);
    }
}
