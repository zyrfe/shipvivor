//! Game rules for one shell contact with one armor panel.
//!
//! Collision supplies the portion of the shell's footprint overlapping this panel,
//! in panel-local cells. The result identifies whether the panel changed and
//! whether the shot can continue. Each ship owns its panels and their breaches.

mod shell;
pub use shell::{Ammo, MainShell};
mod flight;
pub use flight::ShellArc;

/// The player's main turret fitting. Its muzzle offsets are in unscaled model units.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TurretLayout {
    #[default]
    Twin,
    Triple,
    Quad,
}
impl TurretLayout {
    pub const fn count(self) -> usize {
        match self {
            Self::Twin => 2,
            Self::Triple => 3,
            Self::Quad => 4,
        }
    }
    pub fn barrel_x(self, tube: usize) -> f32 {
        let spacing = match self {
            Self::Twin => 0.88,
            Self::Triple => 0.70,
            Self::Quad => 0.60,
        };
        (tube as f32 - (self.count() - 1) as f32 * 0.5) * spacing
    }
}

/// The crew-worked close-range battery as fitted.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Secondaries {
    /// Guns on each beam.
    pub per_side: usize,
    /// Damage a hit does.
    pub damage: u16,
    /// Seconds between rounds from one gun.
    pub reload: f32,
    /// Metres out to which the crew engages.
    pub range: f32,
    /// Barrels on each gun. A twin mount fires both together.
    pub barrels: usize,
}
impl Secondaries {
    pub const STOCK: Self = Self {
        per_side: 1,
        damage: 1,
        reload: 0.82,
        range: 160.,
        barrels: 1,
    };
    /// The beam has deck room for this many guns a side.
    pub const MOST_PER_SIDE: usize = 3;
}

use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShellClass {
    Standard,
    Breaker,
    Heavy,
}

impl ShellClass {
    pub fn plate_label(self) -> &'static str {
        match self {
            Self::Standard => "Light",
            Self::Breaker => "Reinforced",
            Self::Heavy => "Capital",
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArmorGrade {
    Light,
    Reinforced,
    Capital,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContactAngle {
    Direct,
    Glancing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Impact {
    Bounced,
    Embedded,
    /// The shot opened solid cells and can continue.
    Penetrated,
    /// The entire footprint was already open.
    PassedThrough,
    /// An armour-piercing round went clean through light plate and spent little of itself.
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
    /// The share of a round's damage that reaches behind the plate.
    pub fn dealt(self, damage: u16) -> u16 {
        match self {
            Self::Penetrated | Self::PassedThrough => damage,
            Self::Overpenetrated => damage.div_ceil(2),
            Self::Bounced | Self::Embedded => 0,
        }
    }
}

pub type Cell = (u16, u16);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContactError {
    EmptyFootprint,
    OutsidePanel(Cell),
}

#[derive(Clone, Debug)]
pub struct ArmorPanel {
    grade: ArmorGrade,
    columns: u16,
    rows: u16,
    breaches: HashMap<Cell, u16>,
}

impl ArmorPanel {
    /// An intact panel. Cells are zero-based (column, row).
    pub fn new(grade: ArmorGrade, columns: u16, rows: u16) -> Self {
        Self {
            grade,
            columns,
            rows,
            breaches: HashMap::new(),
        }
    }

    pub fn is_breached(&self, cell: Cell) -> bool {
        self.breaches.contains_key(&cell)
    }

    /// Resolve one contact atomically. Invalid footprints leave the panel untouched.
    pub fn resolve_hit(
        &mut self,
        shell: ShellClass,
        angle: ContactAngle,
        footprint: &[Cell],
    ) -> Result<Impact, ContactError> {
        self.resolve_round(shell, angle, footprint, 0)
    }

    /// Resolve a round with its filling. HE bursts against the plate instead of glancing
    /// off, and AP that opens light plate goes clean through it.
    pub fn resolve_ammo(
        &mut self,
        ammo: Ammo,
        shell: ShellClass,
        angle: ContactAngle,
        footprint: &[Cell],
        bore: u16,
    ) -> Result<Impact, ContactError> {
        let angle = match ammo {
            Ammo::He => ContactAngle::Direct,
            Ammo::Ap => angle,
        };
        let outcome = self.resolve_round(shell, angle, footprint, bore)?;
        Ok(
            if ammo == Ammo::Ap && outcome == Impact::Penetrated && self.grade == ArmorGrade::Light
            {
                Impact::Overpenetrated
            } else {
                outcome
            },
        )
    }

    /// The aperture remembers the largest bore that actually opened each cell.
    pub fn resolve_round(
        &mut self,
        shell: ShellClass,
        angle: ContactAngle,
        footprint: &[Cell],
        bore: u16,
    ) -> Result<Impact, ContactError> {
        if footprint.is_empty() {
            return Err(ContactError::EmptyFootprint);
        }
        for &(column, row) in footprint {
            if column >= self.columns || row >= self.rows {
                return Err(ContactError::OutsidePanel((column, row)));
            }
        }
        if footprint.iter().all(|cell| {
            self.breaches
                .get(cell)
                .is_some_and(|opening| *opening >= bore)
        }) {
            return Ok(Impact::PassedThrough);
        }
        // Authored prototype outcomes for contact with any remaining solid material.
        match (angle, shell, self.grade) {
            (ContactAngle::Glancing, _, _) => Ok(Impact::Bounced),
            (
                ContactAngle::Direct,
                ShellClass::Standard,
                ArmorGrade::Reinforced | ArmorGrade::Capital,
            )
            | (ContactAngle::Direct, ShellClass::Breaker, ArmorGrade::Capital) => {
                Ok(Impact::Embedded)
            }
            (ContactAngle::Direct, _, _) => {
                for &cell in footprint {
                    let opening = self.breaches.entry(cell).or_insert(bore);
                    *opening = (*opening).max(bore);
                }
                Ok(Impact::Penetrated)
            }
        }
    }
}

#[cfg(test)]
mod tests;
