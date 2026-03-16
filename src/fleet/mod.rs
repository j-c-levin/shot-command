pub mod lobby;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::ship::ShipClass;
use crate::weapon::{MountSize, WeaponType};

/// No-op plugin that wires the fleet module into the app.
/// Systems will be added in future tasks.
pub struct FleetPlugin;

impl Plugin for FleetPlugin {
    fn build(&self, _app: &mut App) {
        // No systems yet — lobby and fleet builder coming in later tasks.
    }
}

/// Total point budget each player has to build their fleet.
pub const FLEET_BUDGET: u16 = 1000;

/// Point cost of a ship hull (no weapons).
pub fn hull_cost(class: ShipClass) -> u16 {
    match class {
        ShipClass::Battleship => 450,
        ShipClass::Destroyer => 200,
        ShipClass::Scout => 140,
    }
}

/// Point cost of a single weapon.
pub fn weapon_cost(weapon: WeaponType) -> u16 {
    match weapon {
        WeaponType::HeavyCannon => 40,
        WeaponType::Cannon => 20,
        WeaponType::Railgun => 50,
        WeaponType::HeavyVLS => 45,
        WeaponType::LightVLS => 25,
        WeaponType::LaserPD => 30,
        WeaponType::CWIS => 15,
    }
}

/// A ship specification: hull class plus weapon loadout.
/// Each entry in `loadout` corresponds to a mount slot on the ship.
/// `None` means the slot is empty.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShipSpec {
    pub class: ShipClass,
    pub loadout: Vec<Option<WeaponType>>,
}

/// Total point cost of a single ship spec (hull + all weapons).
pub fn ship_spec_cost(spec: &ShipSpec) -> u16 {
    let weapons: u16 = spec
        .loadout
        .iter()
        .filter_map(|slot| slot.map(weapon_cost))
        .sum();
    hull_cost(spec.class) + weapons
}

/// Total point cost of an entire fleet.
pub fn fleet_cost(specs: &[ShipSpec]) -> u16 {
    specs.iter().map(ship_spec_cost).sum()
}

/// Errors that can occur when validating a fleet composition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FleetError {
    /// Fleet has no ships.
    EmptyFleet,
    /// Fleet exceeds point budget.
    OverBudget { cost: u16, budget: u16 },
    /// Ship loadout length doesn't match mount layout length.
    WrongSlotCount { ship_index: usize, expected: usize, got: usize },
    /// Weapon is too large for the mount slot.
    WeaponTooLarge { ship_index: usize, slot_index: usize, slot_size: MountSize, weapon_size: MountSize },
}

/// Validate an entire fleet against the budget and mount constraints.
/// Returns Ok(()) if the fleet is valid, or the first error found.
pub fn validate_fleet(specs: &[ShipSpec]) -> Result<(), FleetError> {
    if specs.is_empty() {
        return Err(FleetError::EmptyFleet);
    }

    let cost = fleet_cost(specs);
    if cost > FLEET_BUDGET {
        return Err(FleetError::OverBudget { cost, budget: FLEET_BUDGET });
    }

    for (ship_idx, spec) in specs.iter().enumerate() {
        let layout = spec.class.mount_layout();
        if spec.loadout.len() != layout.len() {
            return Err(FleetError::WrongSlotCount {
                ship_index: ship_idx,
                expected: layout.len(),
                got: spec.loadout.len(),
            });
        }

        for (slot_idx, (weapon_opt, (slot_size, _))) in spec.loadout.iter().zip(layout.iter()).enumerate() {
            if let Some(weapon) = weapon_opt {
                let weapon_size = weapon.mount_size();
                if !slot_size.fits(weapon_size) {
                    return Err(FleetError::WeaponTooLarge {
                        ship_index: ship_idx,
                        slot_index: slot_idx,
                        slot_size: *slot_size,
                        weapon_size,
                    });
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hull_cost_battleship() {
        assert_eq!(hull_cost(ShipClass::Battleship), 450);
    }

    #[test]
    fn hull_cost_destroyer() {
        assert_eq!(hull_cost(ShipClass::Destroyer), 200);
    }

    #[test]
    fn hull_cost_scout() {
        assert_eq!(hull_cost(ShipClass::Scout), 140);
    }

    #[test]
    fn weapon_cost_heavy_cannon() {
        assert_eq!(weapon_cost(WeaponType::HeavyCannon), 40);
    }

    #[test]
    fn weapon_cost_railgun() {
        assert_eq!(weapon_cost(WeaponType::Railgun), 50);
    }

    #[test]
    fn weapon_cost_heavy_vls() {
        assert_eq!(weapon_cost(WeaponType::HeavyVLS), 45);
    }

    #[test]
    fn weapon_cost_cannon() {
        assert_eq!(weapon_cost(WeaponType::Cannon), 20);
    }

    #[test]
    fn weapon_cost_light_vls() {
        assert_eq!(weapon_cost(WeaponType::LightVLS), 25);
    }

    #[test]
    fn weapon_cost_laser_pd() {
        assert_eq!(weapon_cost(WeaponType::LaserPD), 30);
    }

    #[test]
    fn weapon_cost_cwis() {
        assert_eq!(weapon_cost(WeaponType::CWIS), 15);
    }

    #[test]
    fn ship_spec_cost_full_destroyer() {
        // Destroyer(200) + Railgun(50) + Cannon(20) + LaserPD(30) + CWIS(15) = 315
        let spec = ShipSpec {
            class: ShipClass::Destroyer,
            loadout: vec![
                Some(WeaponType::Railgun),
                Some(WeaponType::Cannon),
                Some(WeaponType::LaserPD),
                Some(WeaponType::CWIS),
            ],
        };
        assert_eq!(ship_spec_cost(&spec), 315);
    }

    #[test]
    fn ship_spec_cost_empty_slots() {
        // Scout(140) with no weapons
        let spec = ShipSpec {
            class: ShipClass::Scout,
            loadout: vec![None, None],
        };
        assert_eq!(ship_spec_cost(&spec), 140);
    }

    #[test]
    fn fleet_cost_multiple_ships() {
        // Battleship(450) + Scout(140) with Cannon(20) + CWIS(15) = 625
        let specs = vec![
            ShipSpec {
                class: ShipClass::Battleship,
                loadout: vec![None, None, None, None, None, None],
            },
            ShipSpec {
                class: ShipClass::Scout,
                loadout: vec![Some(WeaponType::Cannon), Some(WeaponType::CWIS)],
            },
        ];
        assert_eq!(fleet_cost(&specs), 625);
    }

    #[test]
    fn validate_valid_fleet() {
        let specs = vec![
            ShipSpec {
                class: ShipClass::Destroyer,
                loadout: vec![
                    Some(WeaponType::Railgun),
                    Some(WeaponType::Cannon),
                    Some(WeaponType::LaserPD),
                    Some(WeaponType::CWIS),
                ],
            },
            ShipSpec {
                class: ShipClass::Scout,
                loadout: vec![Some(WeaponType::Cannon), Some(WeaponType::CWIS)],
            },
        ];
        assert!(validate_fleet(&specs).is_ok());
    }

    #[test]
    fn validate_over_budget() {
        // 3 battleships = 3 * 450 = 1350 > 1000
        let specs = vec![
            ShipSpec { class: ShipClass::Battleship, loadout: vec![None, None, None, None, None, None] },
            ShipSpec { class: ShipClass::Battleship, loadout: vec![None, None, None, None, None, None] },
            ShipSpec { class: ShipClass::Battleship, loadout: vec![None, None, None, None, None, None] },
        ];
        assert_eq!(
            validate_fleet(&specs),
            Err(FleetError::OverBudget { cost: 1350, budget: 1000 })
        );
    }

    #[test]
    fn validate_wrong_slot_count() {
        // Scout has 2 slots, giving 3
        let specs = vec![ShipSpec {
            class: ShipClass::Scout,
            loadout: vec![None, None, None],
        }];
        assert_eq!(
            validate_fleet(&specs),
            Err(FleetError::WrongSlotCount { ship_index: 0, expected: 2, got: 3 })
        );
    }

    #[test]
    fn validate_weapon_too_large() {
        // Scout: slot 0 is Medium, HeavyCannon requires Large
        let specs = vec![ShipSpec {
            class: ShipClass::Scout,
            loadout: vec![Some(WeaponType::HeavyCannon), Some(WeaponType::CWIS)],
        }];
        assert_eq!(
            validate_fleet(&specs),
            Err(FleetError::WeaponTooLarge {
                ship_index: 0,
                slot_index: 0,
                slot_size: MountSize::Medium,
                weapon_size: MountSize::Large,
            })
        );
    }

    #[test]
    fn validate_empty_fleet() {
        assert_eq!(validate_fleet(&[]), Err(FleetError::EmptyFleet));
    }

    #[test]
    fn validate_downsized_weapon_ok() {
        // Putting a Small weapon (CWIS) in a Large slot is fine
        let specs = vec![ShipSpec {
            class: ShipClass::Scout,
            loadout: vec![Some(WeaponType::CWIS), Some(WeaponType::CWIS)],
        }];
        assert!(validate_fleet(&specs).is_ok());
    }
}
