//! Designs the yard builds without being asked: the player's cruiser and the enemy
//! classes. Each enemy is cut to its class's length and hull points, with its guns on
//! the modules beneath their present deck stations.
use crate::design::{Component::*, Design, Frame, Frame::*, Module, Protection};
use shipvivor_artillery::ArmorGrade;

/// An enemy design stretched to `length` metres, with `strength` hull points in each
/// module of the given frame, `gun` points in each main gun and a crew mending at
/// `repair` pace. Sinking one takes about half again its class's old hull points.
fn enemy(
    name: &str,
    modules: Vec<Module>,
    length: f32,
    (frame, strength): (Frame, f32),
    gun: f32,
    repair: f32,
) -> Design {
    let mut design = Design::new(name, modules).expect("a stock design");
    design.scale = length / design.length();
    design.toughness = strength / frame.spec().structure;
    design.fittings = gun / (12. * design.toughness);
    design.repair = repair;
    design
}

impl Protection {
    /// Breakwater as built: an inch of stock plate, which turns nothing that is fired
    /// at her, stock fittings, and an engine that fails once it has taken two thirds of
    /// its compartment's structure.
    pub const BREAKWATER: Self = Self {
        plating: 1.,
        thickness: 1.,
        engine: 2. / 3.,
        fittings: 1.,
    };
}

impl Design {
    pub const BREAKWATER_LENGTH: f32 = 30.;
    /// The yard splices midship frames between the original machinery space and after gun.
    pub const BREAKWATER_SPLICE: usize = 4;
    /// Pace of her crew's repairs and pumps against a stock crew's. A flooded module or a
    /// knocked-out gun stays down long enough to matter, so a ship that has been badly
    /// knocked about stays that way for a while.
    pub const BREAKWATER_REPAIR: f32 = 0.65;

    /// The player's cruiser: 30 metres and 100 hull points, a main gun fore and aft,
    /// the director and close guns amidships, and the torpedo racks over the engine.
    /// She stays afloat until four of her six modules are wrecked, and her crew mends
    /// at [`Design::BREAKWATER_REPAIR`] pace.
    pub fn breakwater() -> Self {
        let mut ship = Self::new(
            "Breakwater",
            vec![
                Module::new(Bow, []),
                Module::new(Straight, [MainGun]),
                Module::new(Sponsons, [Director, CloseGun, CloseGun]),
                Module::new(Sponsons, [TorpedoRack, TorpedoRack, Engine]),
                Module::new(Straight, [MainGun]),
                Module::new(Stern, []),
            ],
        )
        .expect("a stock design");
        ship.sinks_at = 2. / 3.;
        ship.repair = Self::BREAKWATER_REPAIR;
        ship.protect(Protection::BREAKWATER);
        ship.auxiliary_drive = true;
        ship.leaks = true;
        ship
    }

    /// A raider's boat: one module of 4 hull points with nobody aboard to mend it. Its
    /// engine fails at half structure, so one good hit leaves it adrift.
    pub fn patrol_boat() -> Self {
        enemy(
            "Patrol boat",
            vec![Module::new(Boat, [MainGun, Engine])],
            15.,
            (Boat, 4.),
            2.,
            0.,
        )
    }

    /// A torpedo boat: one 3-point hull of 12 metres with her engine in it and nobody
    /// aboard to mend it. Her one small gun goes out at the first hit on it. Her engine
    /// fails at half structure, so one main round sinks her or leaves her adrift.
    pub fn torpedo_boat() -> Self {
        enemy(
            "Torpedo boat",
            vec![Module::new(Boat, [MainGun, Engine])],
            12.,
            (Boat, 3.),
            1.,
            0.,
        )
    }

    /// A raider's destroyer: 22 metres, a gun fore and aft and the engine between them,
    /// with a small crew that mends slowly. Faster than a cruiser and lighter built.
    pub fn destroyer() -> Self {
        enemy(
            "Destroyer",
            vec![
                Module::new(Bow, []),
                Module::new(Straight, [MainGun]),
                Module::new(Straight, [Engine]),
                Module::new(Straight, [MainGun]),
                Module::new(Stern, []),
            ],
            22.,
            (Straight, 3.),
            2.,
            0.25,
        )
    }

    /// An unarmed merchant hull with its engine aft.
    pub fn freighter() -> Self {
        enemy(
            "Freighter",
            vec![
                Module::new(Bow, []),
                Module::new(Straight, []),
                Module::new(Straight, []),
                Module::new(Straight, []),
                Module::new(Straight, [Engine]),
                Module::new(Stern, []),
            ],
            30.1,
            (Straight, 4.),
            0.,
            0.,
        )
    }

    /// A coastal trader a quarter of a freighter's length: one 3-point hull with her
    /// engine in it and nobody aboard to mend it. Her engine fails at half structure, so
    /// one main round leaves her adrift and a secondary round more sinks her.
    pub fn coaster() -> Self {
        enemy(
            "Coaster",
            vec![Module::new(Boat, [Engine])],
            8.,
            (Boat, 3.),
            0.,
            0.,
        )
    }

    /// The enemy's cruiser: a gun fore and aft, with the engine under the after gun.
    /// Fire at her middle sinks her. Fire at her stern stops her first.
    pub fn cruiser() -> Self {
        enemy(
            "Cruiser",
            vec![
                Module::new(Bow, []),
                Module::new(Straight, [MainGun]),
                Module::new(Straight, []),
                Module::new(Straight, []),
                Module::new(Straight, [MainGun, Engine]),
                Module::new(Stern, []),
            ],
            30.1,
            (Straight, 5.),
            2.,
            0.5,
        )
    }

    /// A capital ship: a magazine under each end turret, and the director and both
    /// engines amidships, all inside a capital-grade citadel.
    pub fn battleship() -> Self {
        let mut ship = enemy(
            "Battleship",
            vec![
                Module::new(Bow, []),
                Module::new(Wide, [MainGun, Magazine]),
                Module::new(Wide, [MainGun]),
                Module::new(Wide, [Director, Engine]),
                Module::new(Wide, [Engine]),
                Module::new(Wide, [MainGun, Magazine]),
                Module::new(Stern, []),
            ],
            51.,
            (Wide, 21.),
            6.,
            0.5,
        );
        ship.citadel = Some(ArmorGrade::Capital);
        ship
    }

    /// The commodore's battleship: a battleship lengthened by a turret, with superfiring
    /// pairs fore and aft, a magazine under each end pair, the director over the after
    /// engine, and tougher turrets in the same capital-grade citadel. Her hull is twice a
    /// battleship's, so she outlasts one by about two thirds again under the same guns.
    pub fn commodore() -> Self {
        let mut ship = enemy(
            "Commodore",
            vec![
                Module::new(Bow, []),
                Module::new(Wide, [MainGun, Magazine]),
                Module::new(Wide, [MainGun]),
                Module::new(Wide, [Engine]),
                Module::new(Wide, [Director, Engine]),
                Module::new(Wide, [MainGun]),
                Module::new(Wide, [MainGun, Magazine]),
                Module::new(Stern, []),
            ],
            58.,
            (Wide, 36.),
            7.,
            0.6,
        );
        ship.citadel = Some(ArmorGrade::Capital);
        ship
    }

    /// The flagship: ten turrets of 7 points each, in pairs across the hull with one
    /// at each end, and a hull as stout as a battleship's behind them. Their reinforced
    /// plate stands six stock rounds, as the Regent's does. Her turrets screen her hull
    /// while most of them stand, and her capital belt holds what lighter guns throw at
    /// it, so she outlasts a battleship two to three times over.
    pub fn flagship() -> Self {
        let pair = || Module::new(Wide, [MainGun, MainGun]);
        enemy(
            "Flagship",
            vec![
                Module::new(Bow, []),
                Module::new(Wide, [MainGun]),
                pair(),
                pair(),
                pair(),
                Module::new(Wide, [Engine, Engine]),
                pair(),
                Module::new(Stern, [MainGun]),
            ],
            81.,
            (Wide, 24.),
            7.,
            0.,
        )
    }

    /// The Regent has five paired gun decks, two end turrets and a deep hull beneath them,
    /// the stoutest afloat: she outlasts a battleship about four times over.
    pub fn iron_regent() -> Self {
        let pair = || Module::new(Wide, [MainGun, MainGun]);
        enemy(
            "Iron Regent",
            vec![
                Module::new(Bow, []),
                Module::new(Wide, [MainGun]),
                pair(),
                pair(),
                pair(),
                Module::new(Wide, [Engine, Engine]),
                pair(),
                pair(),
                Module::new(Stern, [MainGun]),
            ],
            168.,
            (Wide, 40.),
            7.,
            0.,
        )
    }
}
