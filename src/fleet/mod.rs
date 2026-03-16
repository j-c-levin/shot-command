use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::ship::ShipClass;
use crate::weapon::{MountSize, WeaponType};

/// Total point budget each player has to build their fleet.
pub const FLEET_BUDGET: u16 = 1000;

/// Point cost of a ship hull (no weapons).
pub fn hull_cost(class: ShipClass) -> u16 {
    match class {
        ShipClass::Battleship => 375,
        ShipClass::Destroyer => 150,
        ShipClass::Scout => 45,
    }
}

/// Point cost of a single weapon.
pub fn weapon_cost(weapon: WeaponType) -> u16 {
    match weapon {
        WeaponType::HeavyCannon => 30,
        WeaponType::Cannon => 15,
        WeaponType::Railgun => 40,
        WeaponType::HeavyVLS => 35,
        WeaponType::LightVLS => 20,
        WeaponType::LaserPD => 25,
        WeaponType::CWIS => 10,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hull_cost_battleship() {
        assert_eq!(hull_cost(ShipClass::Battleship), 375);
    }

    #[test]
    fn hull_cost_destroyer() {
        assert_eq!(hull_cost(ShipClass::Destroyer), 150);
    }

    #[test]
    fn hull_cost_scout() {
        assert_eq!(hull_cost(ShipClass::Scout), 45);
    }

    #[test]
    fn weapon_cost_heavy_cannon() {
        assert_eq!(weapon_cost(WeaponType::HeavyCannon), 30);
    }

    #[test]
    fn weapon_cost_railgun() {
        assert_eq!(weapon_cost(WeaponType::Railgun), 40);
    }

    #[test]
    fn weapon_cost_heavy_vls() {
        assert_eq!(weapon_cost(WeaponType::HeavyVLS), 35);
    }

    #[test]
    fn weapon_cost_cannon() {
        assert_eq!(weapon_cost(WeaponType::Cannon), 15);
    }

    #[test]
    fn weapon_cost_light_vls() {
        assert_eq!(weapon_cost(WeaponType::LightVLS), 20);
    }

    #[test]
    fn weapon_cost_laser_pd() {
        assert_eq!(weapon_cost(WeaponType::LaserPD), 25);
    }

    #[test]
    fn weapon_cost_cwis() {
        assert_eq!(weapon_cost(WeaponType::CWIS), 10);
    }

    #[test]
    fn ship_spec_cost_full_destroyer() {
        // Destroyer(150) + Railgun(40) + Cannon(15) + LaserPD(25) + CWIS(10) = 240
        let spec = ShipSpec {
            class: ShipClass::Destroyer,
            loadout: vec![
                Some(WeaponType::Railgun),
                Some(WeaponType::Cannon),
                Some(WeaponType::LaserPD),
                Some(WeaponType::CWIS),
            ],
        };
        assert_eq!(ship_spec_cost(&spec), 240);
    }

    #[test]
    fn ship_spec_cost_empty_slots() {
        // Scout(45) with no weapons
        let spec = ShipSpec {
            class: ShipClass::Scout,
            loadout: vec![None, None],
        };
        assert_eq!(ship_spec_cost(&spec), 45);
    }

    #[test]
    fn fleet_cost_multiple_ships() {
        // Battleship(375) + Scout(45) with Cannon(15) + CWIS(10) = 445
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
        assert_eq!(fleet_cost(&specs), 445);
    }
}
