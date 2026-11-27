//! Game rules for one shell contact with one armor panel.
//!
//! Collision supplies the panel-local cell a round strikes. Every round has a
//! penetration in inches, and every plate a thickness. The result says what became of
//! the round, what share of its damage got behind the plate, and whether it flies on.
//! Each ship owns its panels and the wear on them.

mod shell;
pub use shell::MainShell;
mod flight;
pub use flight::ShellArc;

/// The player's main turret fitting. Its muzzle offsets are in unscaled model units.
/// The player's turrets are built single and take more barrels one at a time in the
/// yard. Enemy mounts are twins.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TurretLayout {
    #[default]
    Single,
    Twin,
    Triple,
    Quad,
}
impl TurretLayout {
    pub const ALL: [Self; 4] = [Self::Single, Self::Twin, Self::Triple, Self::Quad];
    pub const fn count(self) -> usize {
        match self {
            Self::Single => 1,
            Self::Twin => 2,
            Self::Triple => 3,
            Self::Quad => 4,
        }
    }
    pub fn barrel_x(self, tube: usize) -> f32 {
        let spacing = match self {
            Self::Single => 0.,
            Self::Twin => 0.88,
            Self::Triple => 0.70,
            Self::Quad => 0.60,
        };
        (tube as f32 - (self.count() - 1) as f32 * 0.5) * spacing
    }
    /// One barrel more, if the gunhouse can take it.
    pub fn more(self) -> Option<Self> {
        Self::ALL.get(self.count()).copied()
    }
    /// One barrel fewer, if it has more than one.
    pub fn fewer(self) -> Option<Self> {
        self.count().checked_sub(2).map(|i| Self::ALL[i])
    }
}

/// Degrees per second a stock four-inch gunhouse trains.
pub const STOCK_TRAVERSE: f32 = 18.;
/// Ranks of training gear a gunhouse can take.
pub const MOST_TRAINING_GEAR: u16 = 8;

/// How fast a main gunhouse trains and elevates, as a multiple of a stock four-inch
/// mount. A larger bore is a heavier gunhouse and moves more slowly, and each rank of
/// training gear adds a quarter, so four ranks bring a sixteen-inch mount back to stock
/// and eight ranks it trains half again as fast.
pub fn training_rate(calibre_inches: f32, gear: u16) -> f32 {
    (4. / calibre_inches.max(1.)).sqrt() * (1. + 0.25 * f32::from(gear.min(MOST_TRAINING_GEAR)))
}

/// The crew-worked close-range battery as fitted.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Secondaries {
    /// Guns on each side: the stock pair on the bow and the beam, then more along the
    /// deck edge, as many as the hull has room for.
    pub per_side: usize,
    /// Damage a hit does.
    pub damage: u16,
    /// Seconds between rounds from one gun.
    pub reload: f32,
    /// Metres out to which the crew engages.
    pub range: f32,
    /// Barrels on each gun. A twin mount fires both together.
    pub barrels: usize,
    /// Inches across the bore of the gunhouse each gun is rebuilt as, like the main
    /// battery's first turrets: four or six. `None` is the shielded mount she is built
    /// with.
    pub gunhouse: Option<u16>,
}
impl Secondaries {
    /// A gun each side amidships and another on the bow.
    pub const STOCK: Self = Self {
        per_side: 2,
        damage: 1,
        reload: 0.82,
        range: 160.,
        barrels: 1,
        gunhouse: None,
    };
    /// Inches across the round a shielded mount fires. A gunhouse fires its own bore's.
    pub const MOUNT_BORE: f32 = 0.07 / 0.0254;
    /// Inches across the round each gun fires.
    pub fn bore(&self) -> f32 {
        self.gunhouse.map_or(Self::MOUNT_BORE, f32::from)
    }
    /// No hull has room for more guns a side than this. Her deck room grows with her
    /// length, two a side on each open section: see `Design::wing_stations`.
    pub const MOST_PER_SIDE: usize = 24;
}

/// The light anti-aircraft battery as fitted. Each mount fires a time-fused round that
/// bursts where it leads an aircraft to be.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AntiAir {
    /// Mounts aboard, in port and starboard pairs.
    pub mounts: usize,
    /// Damage a burst does to an aircraft caught in it.
    pub damage: u16,
    /// Seconds between rounds from one mount.
    pub reload: f32,
    /// Metres out to which the crew engages.
    pub range: f32,
}
impl AntiAir {
    /// The pair Breakwater is built with, on her bridge wings.
    pub const STOCK: Self = Self {
        mounts: 2,
        damage: 1,
        reload: 1.1,
        range: 280.,
    };
    /// No guns at all, with the stock pair's figures.
    pub const NONE: Self = Self {
        mounts: 0,
        ..Self::STOCK
    };
    /// No hull has room for more mounts than this. Her deck room grows with her length,
    /// a pair on each open section: see `Design::aa_stations`.
    pub const MOST_MOUNTS: usize = 30;
}

use std::collections::HashMap;

/// A round at least this many times as wide as the plate is thick bites into it however
/// shallow the angle, rather than skipping off.
pub const OVERMATCH: f32 = 2.;
/// A round with this many times the penetration a plate needs may go clean through it
/// and fly on.
pub const OVERPENETRATION: f32 = 3.;
/// Share of a round's damage that reaches behind a plate it goes clean through.
pub const OVERPENETRATED_SHARE: f32 = 0.7;
/// Share of a round's damage that reaches behind a plate that holds it: this much at
/// the least, and this much more as it comes near to opening the plate, rising with
/// the square of how near.
pub const HELD_SHARE: (f32, f32) = (0.1, 0.5);
/// Share of a round's damage that a ricochet passes into the plate it skips off, when
/// it would have opened it striking square. Less as it would have opened less.
pub const RICOCHET_SHARE: f32 = 0.1;
/// Share of a cell's plate that a held round wears away, for each share of the plate's
/// thickness its penetration came to. The cells beside it lose half as much.
const CHIPPING: f32 = 0.25;
/// Share of that wear a round skipping off leaves.
const GRAZING: f32 = 0.25;
/// A round keeps 1 / (1 + `WAY_LOSS` t) of its penetration t seconds out.
const WAY_LOSS: f32 = 0.3;
/// How fast the chance of going clean through grows past [`OVERPENETRATION`], and the
/// most it comes to.
const THROUGH_CHANCE: (f32, f32) = (6., 0.5);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArmorGrade {
    Light,
    Reinforced,
    Capital,
}
impl ArmorGrade {
    /// Inches of sound plate of this grade.
    pub const fn inches(self) -> f32 {
        match self {
            Self::Light => 2.,
            Self::Reinforced => 6.,
            Self::Capital => 12.,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Light => "Light",
            Self::Reinforced => "Reinforced",
            Self::Capital => "Capital",
        }
    }
}

/// A round as it meets plate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Round {
    /// Inches across.
    pub calibre: f32,
    /// Inches of sound plate it goes through striking square.
    pub penetration: f32,
}
impl Round {
    /// The share of its penetration a round keeps `flight_seconds` out.
    pub fn retained(flight_seconds: f32) -> f32 {
        1. / (1. + WAY_LOSS * flight_seconds.max(0.))
    }
    /// A round with no caps from any gun: at the muzzle it goes through as many inches
    /// of plate as it is wide.
    pub fn plain(calibre: f32, flight_seconds: f32) -> Self {
        Self {
            calibre,
            penetration: calibre * Self::retained(flight_seconds),
        }
    }
    /// Whether it opens sound plate of `grade` striking square.
    pub fn opens(self, grade: ArmorGrade) -> bool {
        self.penetration >= grade.inches()
    }
    /// What is left of it once it has gone clean through `inches` of plate, in one
    /// side and out of the other.
    #[must_use]
    pub fn beyond(self, inches: f32) -> Self {
        Self {
            penetration: (self.penetration - 2. * inches).max(0.),
            ..self
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContactAngle {
    Direct,
    Glancing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Impact {
    /// It skipped off the plate, and flies on.
    Bounced,
    /// The plate held it. It burst against the plate, cratering it, and only some of
    /// its blast got through.
    Embedded,
    /// The shot opened solid cells and burst behind them.
    Penetrated,
    /// The plate there was already open.
    PassedThrough,
    /// It went clean through, spending only some of itself behind the plate, and flies
    /// on.
    Overpenetrated,
}
impl Impact {
    /// The round opened the plate, or found it already open, and reached what lay behind.
    pub fn opened(self) -> bool {
        matches!(
            self,
            Self::Penetrated | Self::PassedThrough | Self::Overpenetrated
        )
    }
}

/// What became of one round on one plate, and the share of its damage that reached
/// behind it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Strike {
    pub impact: Impact,
    pub share: f32,
}

pub type Cell = (u16, u16);

/// A contact at a cell the panel doesn't have.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OutsidePanel(pub Cell);

#[derive(Clone, Debug)]
pub struct ArmorPanel {
    grade: ArmorGrade,
    columns: u16,
    rows: u16,
    /// The share of each worn cell's plate that is gone. A cell worn through is open.
    wear: HashMap<Cell, f32>,
}

impl ArmorPanel {
    /// An intact panel. Cells are zero-based (column, row).
    pub fn new(grade: ArmorGrade, columns: u16, rows: u16) -> Self {
        Self {
            grade,
            columns,
            rows,
            wear: HashMap::new(),
        }
    }

    pub fn grade(&self) -> ArmorGrade {
        self.grade
    }

    /// The plate at `cell` is open: shot through, or worn away.
    pub fn is_breached(&self, cell: Cell) -> bool {
        self.worn(cell) >= 1.
    }

    /// The share of the plate at `cell` that is gone.
    pub fn worn(&self, cell: Cell) -> f32 {
        self.wear.get(&cell).copied().unwrap_or(0.)
    }

    /// Inches of plate left at `cell`.
    pub fn plate_at(&self, cell: Cell) -> f32 {
        self.grade.inches() * (1. - self.worn(cell)).max(0.)
    }

    /// Resolve one round on one cell. A round that opens the plate leaves a hole there
    /// that later rounds pass through. One the plate holds still gets some of its blast
    /// through, and wears the plate there and round about, so enough of them open it.
    /// A shallow one skips off, unless it is wide enough to bite. `luck`, from 0 to 1,
    /// decides whether a round with penetration to spare goes clean through. A cell
    /// outside the panel leaves it untouched.
    pub fn strike(
        &mut self,
        round: Round,
        angle: ContactAngle,
        cell: Cell,
        luck: f32,
    ) -> Result<Strike, OutsidePanel> {
        if cell.0 >= self.columns || cell.1 >= self.rows {
            return Err(OutsidePanel(cell));
        }
        let plate = self.plate_at(cell);
        if plate <= 0. {
            return Ok(Strike {
                impact: Impact::PassedThrough,
                share: 1.,
            });
        }
        let ratio = round.penetration / plate;
        if angle == ContactAngle::Glancing && round.calibre < OVERMATCH * self.grade.inches() {
            self.chip(cell, ratio.min(1.) * CHIPPING * GRAZING);
            return Ok(Strike {
                impact: Impact::Bounced,
                share: RICOCHET_SHARE * ratio.min(1.),
            });
        }
        if ratio >= 1. {
            self.wear.insert(cell, 1.);
            let (pace, most) = THROUGH_CHANCE;
            let chance = ((ratio - OVERPENETRATION) / pace).clamp(0., most);
            return Ok(if luck < chance {
                Strike {
                    impact: Impact::Overpenetrated,
                    share: OVERPENETRATED_SHARE,
                }
            } else {
                Strike {
                    impact: Impact::Penetrated,
                    share: 1.,
                }
            });
        }
        self.chip(cell, ratio * CHIPPING);
        let (least, more) = HELD_SHARE;
        Ok(Strike {
            impact: Impact::Embedded,
            share: least + more * ratio * ratio,
        })
    }

    /// Wear `amount` off the plate at `cell`, and half as much off each cell beside it.
    fn chip(&mut self, (column, row): Cell, amount: f32) {
        self.wear_cell((column, row), amount);
        let beside = [
            column.checked_sub(1).map(|c| (c, row)),
            (column + 1 < self.columns).then(|| (column + 1, row)),
            row.checked_sub(1).map(|r| (column, r)),
            (row + 1 < self.rows).then(|| (column, row + 1)),
        ];
        for cell in beside.into_iter().flatten() {
            self.wear_cell(cell, 0.5 * amount);
        }
    }
    fn wear_cell(&mut self, cell: Cell, amount: f32) {
        let worn = self.wear.entry(cell).or_insert(0.);
        *worn = (*worn + amount).min(1.);
    }
}

#[cfg(test)]
mod tests;
