//! How a passage looks for the ship as she stands: what its sea's fleet would cost her
//! hull, worked out from the game's own numbers for her guns and theirs, and the one or
//! two changes to her that would cut that cost the most. The yard's chart shows it beside
//! each passage, so the captain can pick the sea that suits her and see what the harder
//! ones want of her.
//!
//! It is an estimate, not a battle. Each base's squadron, the hulls on its slips and the
//! ones it launches while she works it, its torpedo boats and its shore guns, then the
//! flagship and her escort, are fought a group at a time. In a group she sinks first
//! whichever ship costs her most for the time it takes to sink, and every ship fires on
//! her until it goes down. A fleet the scouts missed is a surprise, so it is left out.
//!
//! The figures were set on 2026-11-26 against the simulator's bot, sailing every passage
//! on offer at every yard, for what does her harm (shore guns, ships, the air) and how
//! that grows round by round; and against the captain's own run logs for how hard she
//! fights. The bot lays its guns far worse than a practised captain, so the gunnery here
//! is the captain's: most rounds on target and the guns kept firing.
use crate::sector::{PRESSURE, Sector, doctrine, most_barrels};
use EnemyClass::*;
use shipvivor_artillery::{MainShell, OVERPENETRATED_SHARE, OVERPENETRATION, Round};
use shipvivor_campaign::{CALIBRES, Card, Exit, Fittings, ROUNDS, Run, Stage};
use shipvivor_encounter::{
    AIRFIELD_WORKS, Doctrine, EnemyClass, FLAGSHIP_EARLIEST, FLAGSHIP_SORTIE, IndustrialYard,
    RAID_TIERS, Weight, dock_count,
};
use shipvivor_shipyard::{Design, Mounting, Protection, Ship};

/// Metres off at which she trades fire with a warship, for the flight of her rounds.
const RANGE: f32 = 300.;
/// Metres a second the secondaries' rounds fly.
const SECONDARY_SPEED: f32 = 240.;
/// Inches across an enemy main round for each unit of her class's gun scale.
const ENEMY_CALIBRE: f32 = 5.9;
/// Share of the enemy's rounds that find her.
const ENEMY_AIM: f32 = 0.25;
/// Share of their damage the rounds her plate turns still do as they wear it through:
/// a warship's, fired close and often at the same plates until they give and the bared
/// section takes double, and a shore gun's, spread along her from afar.
const TURNED: (f32, f32) = (0.8, 0.05);
/// Warships of a squadron that engage her at once: a big squadron spreads out as it
/// comes on, and the rest come up as the first are sunk.
const ENGAGED: usize = 4;
/// Seconds she spends on a warship she can't sink before she leaves it astern and gets
/// on with the base. A flagship has to be sunk however long it takes.
const CHASE: f32 = 60.;
/// Damage a second each pair of an enemy's secondaries throws at her, and their bore.
const ENEMY_SECONDARY: (f32, f32) = (0.15, 2.75);
/// Damage a second a torpedo boat's runs do her, on average, between its torpedoes and
/// the little gun it harries her with.
const TORPEDO_BOAT: f32 = 0.15;
/// Steel a dock house and a factory stand before they fall, as the port builds them.
const DOCK: f32 = 24.;
const FACTORY: f32 = 8.;
/// Share of her main rounds that land on a base's works.
const WORKS_AIM: f32 = 0.6;
/// A light shore gun: damage from a single barrel and from each of two or more, inches
/// across, seconds to load and more for each barrel past the first, as the port mounts
/// them. She spends about half her time at a base inside their arcs, and a share of
/// their rounds find her.
const SHORE: (f32, f32, f32, f32, f32) = (11., 8., 12.6, 5., 0.8);
const SHORE_EXPOSURE: f32 = 0.5;
const SHORE_AIM: f32 = 0.03;
/// The heavy battery: damage, inches across and seconds to load, as `ranging` has it,
/// and the share of its rounds that find her across the reach.
const HEAVY: (f32, f32, f32, f32) = (12., 15., 9., 0.1);
/// Seconds she spends at a base however quick her guns: closing it, turning under its
/// guns and lining up on its works.
const BASE_SECONDS: f32 = 45.;
/// Share of the time her guns are laid on what she is fighting, rather than training
/// round, waiting on a turn or ranging.
const DUTY: f32 = 0.7;
/// Hull points a sound ship's crew mends over a passage, with the stores she picks up:
/// about four tenths of a point a second over the six or seven minutes one takes. It is
/// reckoned by the passage rather than by the second, so a slow fight never looks
/// cheaper for the mending done while it drags on.
const MENDED: f32 = 160.;
/// Seconds a barrel takes between rounds however quick its loaders: the turret trains
/// back on, the round is ranged and the trigger pressed.
const LAYING: f32 = 2.5;
/// Seconds a yard's slip takes to fit out between launches, in raid intervals.
const SLIP_TURNS: f32 = 3.;
/// Damage a second the strafers do under air cover from the second round, growing by a
/// share each round, and what the AA battery's bursts a second do to halve it. Every
/// base sends up its harriers as well, wherever she sails.
const AIR: (f32, f32) = (0.12, 0.2);
const HARRIERS: f32 = 0.04;
const AA_HALF: f32 = 3.;
/// The airfield's bombers and torpedo planes, over what the strafers do.
const AIRFIELD: f32 = 0.1;

/// What it takes to sink a class, measured by shelling each in the encounter: damage
/// behind her plate, the fewest hits that ever do it, the inches of plate a round must
/// open to do its full damage, and the share a round that can't open it still does as it
/// wears the plate down: the first figure, and the second times how near it came.
struct Toughness {
    damage: f32,
    least: f32,
    plate: f32,
    held: (f32, f32),
}
const HELD: (f32, f32) = (0.25, 0.35);
fn toughness(class: EnemyClass) -> Toughness {
    let (damage, least, plate, held) = match class {
        Patrol => (4., 1., 2., HELD),
        TorpedoBoat => (3., 1., 2., HELD),
        Destroyer => (14.5, 3., 2., HELD),
        Cruiser => (22.5, 3.3, 2., HELD),
        // The bosses carry the battleship's plate and outlast her, as shelled on
        // 2026-11-26 once they did: the Commodore about twice over, the Parliament five
        // times and the Regent nine. Her strakes come away to bare more of her hull. A
        // flagship's turrets screen her, so her telling plate is theirs, and she has to
        // be stripped of them first.
        Battleship => (85., 3., 12., HELD),
        Commodore => (174., 4., 12., (0.4, 0.35)),
        Parliament => (430., 6., 6., (0.6, 0.35)),
        Regent => (770., 10., 6., (0.5, 0.35)),
        Freighter | Coaster => (10., 1., 2., HELD),
    };
    Toughness {
        damage,
        least,
        plate,
        held,
    }
}
/// Hits of `damage` from `round` it takes to sink one of `class`.
fn hits_to_sink(class: EnemyClass, round: Round, damage: f32) -> f32 {
    let t = toughness(class);
    let ratio = round.penetration / t.plate;
    let share = if ratio >= 1. {
        // Past three times the plate a round may go clean through and do less.
        let through = ((ratio - OVERPENETRATION) / 6.).clamp(0., 0.5);
        1. - (1. - OVERPENETRATED_SHARE) * through
    } else {
        t.held.0 + t.held.1 * ratio
    };
    (t.damage / (damage * share).max(1e-3)).max(t.least)
}
/// Share of her rounds that find a ship of `class` at [`RANGE`], laid by a practised
/// captain: the small, quick hulls are the hardest to hit.
fn aim(class: EnemyClass) -> f32 {
    match class {
        TorpedoBoat => 0.4,
        Patrol => 0.5,
        Destroyer => 0.65,
        Cruiser => 0.75,
        Battleship | Commodore | Parliament | Regent => 0.85,
        Freighter | Coaster => 0.75,
    }
}
/// Pairs of secondaries an enemy class carries.
fn enemy_secondary_pairs(class: EnemyClass) -> f32 {
    match class {
        Destroyer => 1.,
        Cruiser => 2.,
        Battleship => 6.,
        Commodore => 8.,
        Parliament => 10.,
        Regent => 12.,
        _ => 0.,
    }
}
/// Share of a round's damage that a round `calibre` inches across does her, as the
/// campaign's `bore_share` has it: half at three inches, all of it from ten.
fn bore_share(calibre: f32) -> f32 {
    0.5 + 0.5 * ((calibre - 3.) / 7.).clamp(0., 1.)
}

/// One main turret as she fights with it.
#[derive(Clone, Copy, Debug)]
struct Turret {
    shell: MainShell,
    barrels: f32,
    reload: f32,
    /// Share of the fight it bears: a centreline turret over either beam, a wing turret
    /// only over its own.
    bearing: f32,
    /// The biggest bore its cradle takes.
    capacity: u16,
}

/// One of her batteries along the deck edge, on one beam: rounds a second, the damage
/// each does, its bore and its reach.
#[derive(Clone, Copy, Debug, Default)]
struct Side {
    rate: f32,
    damage: f32,
    calibre: f32,
    range: f32,
}

/// The ship as the estimate sees her.
#[derive(Clone, Debug)]
struct Her {
    turrets: Vec<Turret>,
    /// Her deck guns, then her secondary gunhouses.
    secondaries: [Side; 2],
    /// Damage a second the AA battery's bursts do.
    anti_air: f32,
    /// Inches of side plate.
    thickness: f32,
    /// Hull points left, and the most she has.
    hull: f32,
    full: f32,
}
impl Her {
    fn of(ship: &Ship, fittings: &Fittings) -> Self {
        let design: &Design = ship.design();
        let turrets = (design.gun_stations().iter())
            .map(|station| {
                let local = fittings.for_station(station);
                Turret {
                    shell: local.round(),
                    barrels: local.turrets.count() as f32,
                    reload: local.reload().max(LAYING),
                    bearing: match station.mounting {
                        Mounting::Centre => 1.,
                        Mounting::Wing => 0.5,
                        Mounting::Double => 0.6,
                    },
                    capacity: local.capacity(),
                }
            })
            .collect();
        let houses = fittings.gunhouses();
        let room = design.gunhouse_room();
        let gunhouses = houses.per_side.min(room);
        let deck = fittings.secondaries();
        let per_side = deck.per_side.min(design.deck_gun_room(gunhouses)) as f32;
        let aa = fittings.anti_air();
        Self {
            turrets,
            secondaries: [
                Side {
                    rate: per_side * deck.barrels as f32 / deck.reload,
                    damage: f32::from(deck.damage),
                    calibre: deck.bore,
                    range: deck.range,
                },
                Side {
                    rate: gunhouses as f32 * houses.barrels as f32 / houses.reload,
                    damage: f32::from(houses.damage),
                    calibre: f32::from(houses.bore),
                    range: houses.range,
                },
            ],
            anti_air: aa.mounts as f32 * f32::from(aa.damage) / aa.reload,
            thickness: fittings.protection(Protection::BREAKWATER).thickness,
            hull: ship.hull(),
            full: design.structure(),
        }
    }

    /// Ships of `class` she sinks a second with every gun that bears on one.
    fn kill_rate(&self, class: EnemyClass) -> f32 {
        let main: f32 = (self.turrets.iter())
            .map(|t| {
                let round = t.shell.round_at(RANGE / t.shell.speed());
                let hits = hits_to_sink(class, round, f32::from(t.shell.breach_force()));
                t.bearing * t.barrels / t.reload / hits
            })
            .sum();
        // The light hulls close in to circle her, inside her secondaries' reach; the
        // heavy ones stand off beyond it.
        let secondary: f32 = (self.secondaries.iter())
            .filter(|side| side.rate > 0.)
            .map(|side| {
                let stand_off = class.profile().stand_off();
                let close = if class == TorpedoBoat || stand_off <= side.range {
                    1.
                } else {
                    0.2
                };
                let flight = RANGE.min(side.range) / SECONDARY_SPEED;
                let round = Round::plain(side.calibre, flight);
                close * side.rate / hits_to_sink(class, round, side.damage)
            })
            .sum();
        (main + secondary) * aim(class) * DUTY
    }

    /// Damage a second a ship of `class` does her in round `round`.
    fn threat(&self, class: EnemyClass, round: usize) -> f32 {
        if class == TorpedoBoat {
            return TORPEDO_BOAT;
        }
        let p = class.profile();
        let bearing = if class.is_flagship() { 0.6 } else { 1. };
        let barrels = class.main_count() as f32 * bearing * Weight::mean_barrels(class, round);
        let calibre = ENEMY_CALIBRE * p.gun_scale;
        let main = barrels * f32::from(p.damage()) * self.felt(calibre, TURNED.0) / p.reload();
        let (secondary, bore) = ENEMY_SECONDARY;
        let secondaries = enemy_secondary_pairs(class) * secondary * self.felt(bore, TURNED.0);
        (main + secondaries) * ENEMY_AIM
    }

    /// Share of a round `calibre` inches across that tells on her: a light round stings,
    /// and her plate turns one no wider than it is thick until it is worn through.
    fn felt(&self, calibre: f32, turned: f32) -> f32 {
        bore_share(calibre)
            * if calibre <= self.thickness {
                turned
            } else {
                1.
            }
    }

    /// Seconds she spends sinking a ship of `class`, or giving up on one she can't.
    fn time_on(&self, class: EnemyClass) -> f32 {
        let time = 1. / self.kill_rate(class).max(1e-6);
        if class.is_boss() {
            time
        } else {
            time.min(CHASE)
        }
    }

    /// Steel a second her main rounds put into a base's works.
    fn shelling(&self) -> f32 {
        (self.turrets.iter())
            .map(|t| t.bearing * t.barrels * f32::from(t.shell.breach_force()) / t.reload)
            .sum::<f32>()
            * WORKS_AIM
    }

    /// Damage a second a light shore gun of `barrels` does her while she is in its arc.
    fn shore_fire(&self, barrels: u8) -> f32 {
        let (single, each, bore, load, more) = SHORE;
        let barrels = f32::from(barrels.max(1));
        let hit = if barrels > 1. { each * barrels } else { single };
        hit * self.felt(bore, TURNED.1) / (load + more * (barrels - 1.)) * SHORE_AIM
    }

    /// Damage a second the heavy battery of `barrels` does her.
    fn heavy_fire(&self, barrels: u8) -> f32 {
        let (hit, bore, load, aim) = HEAVY;
        f32::from(barrels) * hit * self.felt(bore, TURNED.1) / load * aim
    }
}

/// One base up the channel as the estimate fights it.
struct Base {
    squadron: &'static [EnemyClass],
    docks: usize,
    factories: usize,
    /// Industrial works have two productive governors but build the heavy yard's roster.
    industrial: bool,
    /// Barrels in each of its shore guns, as many as the round allows.
    guns: Vec<u8>,
    /// It mounts the sector's heavy battery, which reaches out across the reach before it.
    heavy: bool,
    /// Its stage up the channel, counting from the mouth.
    stage: usize,
}

/// What a passage would cost her, part by part, in hull points.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Reckoning {
    /// From the warships she fights, the flagship and her escort among them.
    pub ships: f32,
    /// From the bases' shore guns.
    pub shore: f32,
    /// From the heavy battery.
    pub heavy: f32,
    /// From the air.
    pub air: f32,
    /// Seconds the passage takes her.
    pub seconds: f32,
}
impl Reckoning {
    pub fn damage(&self) -> f32 {
        self.ships + self.shore + self.heavy + self.air
    }
}

/// What holds a passage, as the estimate lays it out.
struct Sea {
    round: usize,
    doctrine: Doctrine,
    bases: Vec<Base>,
    boss: Option<EnemyClass>,
    escort: &'static [EnemyClass],
    /// Seconds along their escalation the raids begin.
    head_start: f32,
    raid_interval: f32,
    /// How many times over the bases moor their squadrons and make their steel.
    strength: f32,
    /// Barrels in the heavy battery.
    heavy: u8,
    /// Damage a second from the air, before her AA.
    air: f32,
}
impl Sea {
    fn of(exit: &Exit) -> Self {
        let sector = Sector::sailed(exit);
        let round = exit.stage.round();
        let doctrine = doctrine(exit.stage.fleet());
        let plan = sector.plan();
        let last = plan.len().saturating_sub(1);
        let built = |works: u8| {
            if works == AIRFIELD_WORKS {
                2
            } else {
                usize::from(works.clamp(1, 3))
            }
        };
        let heaviest = plan
            .get(last)
            .and_then(|stage| stage.iter().map(|&w| built(w)).max())
            .unwrap_or(0);
        let cap = most_barrels(exit.stage);
        let mut bases = Vec::new();
        for (stage, works) in plan.iter().enumerate() {
            for &works in works.iter() {
                let factories = built(works);
                // An airfield's harbour is laid out as a cannery's.
                let yard = if works == AIRFIELD_WORKS {
                    1
                } else {
                    factories
                };
                let place = match stage {
                    0 => 0,
                    s if s == last && factories == heaviest => 2,
                    _ => 1,
                };
                let industrial = IndustrialYard::from_works(works).is_some();
                let guns: &[u8] = match (industrial, yard) {
                    (true, _) => &IndustrialYard::LIGHT_GUNS,
                    (_, 1) => &[1],
                    (_, 2) => &[2, 2],
                    _ => &[4, 3, 3],
                };
                bases.push(Base {
                    squadron: doctrine.squadron(round, place),
                    docks: dock_count(yard),
                    factories: if industrial { 2 } else { factories },
                    industrial,
                    guns: guns.iter().map(|&b| b.min(cap)).collect(),
                    heavy: place == 2,
                    stage,
                });
            }
        }
        let growth = 1. + AIR.1 * round.saturating_sub(2) as f32;
        let air = HARRIERS * growth
            + if exit.under_air_cover() {
                AIR.0 * growth
            } else {
                0.
            }
            + if sector.airfield() {
                AIRFIELD * growth
            } else {
                0.
            };
        Self {
            round,
            doctrine,
            bases,
            boss: sector.boss(),
            escort: sector.escort(),
            head_start: sector.head_start(),
            raid_interval: sector.raid_interval(),
            strength: sector.strength() as f32,
            heavy: 2.min(cap),
            air,
        }
    }

    /// What the passage would cost her, part by part.
    fn reckon(&self, her: &Her) -> Reckoning {
        let pressure = PRESSURE[self.round.clamp(1, ROUNDS) - 1];
        let mut cost = Reckoning::default();
        let shelling = her.shelling().max(1e-3);
        // The heavy battery reaches out over the last reach before its base.
        let heavy_from = self
            .bases
            .iter()
            .find(|b| b.heavy && !b.industrial)
            .map_or(usize::MAX, |b| b.stage);
        for base in &self.bases {
            let raids = self.head_start + cost.seconds;
            let tier = RAID_TIERS.iter().filter(|&&t| raids >= t).count();
            let rotation = self
                .doctrine
                .rotation(tier, if base.industrial { 3 } else { base.factories });
            // The hulls waiting on its slips sail when it stirs, with its squadron and
            // its torpedo boats.
            let mut group: Vec<EnemyClass> = (base.squadron.iter().copied())
                .cycle()
                .take(base.squadron.len() * self.strength as usize)
                .collect();
            group.extend((0..base.docks).map(|i| rotation[i % rotation.len()].0));
            group.extend(std::iter::repeat_n(
                TorpedoBoat,
                self.doctrine.torpedo_boats(),
            ));
            let sinking: f32 = group.iter().map(|&c| her.time_on(c)).sum();
            let works = DOCK * base.docks as f32 + FACTORY * base.factories as f32;
            let wrecking = works / shelling;
            // Until she brings its docks down the yard launches what it can fund and fit
            // out. With a dock down its factories turn to raising it, and launch nothing.
            let bill: f32 =
                rotation.iter().map(|&(_, b)| f32::from(b)).sum::<f32>() / rotation.len() as f32;
            let steel = self.strength * (base.docks as f32 + 3. * base.factories as f32) / 60.;
            let fitted = base.docks as f32 / (SLIP_TURNS * self.raid_interval);
            let launches = (steel / bill).min(fitted);
            let each: f32 =
                rotation.iter().map(|&(c, _)| her.time_on(c)).sum::<f32>() / rotation.len() as f32;
            let docks = DOCK * base.docks as f32 / shelling;
            let launched = launches * (0.5 * BASE_SECONDS + docks);
            // Each launch comes out on its own, and waits on the ones before it when
            // they come thick.
            let busy = (launches * each).min(0.6);
            let time = BASE_SECONDS + sinking + wrecking + launched * each;
            let harm: f32 = (rotation.iter())
                .map(|&(c, _)| her.threat(c, self.round) * her.time_on(c))
                .sum::<f32>()
                / rotation.len() as f32;
            cost.ships +=
                self.fight(her, &group, pressure) + launched * harm * pressure / (1. - busy);
            let shore: f32 = base
                .guns
                .iter()
                .map(|&barrels| her.shore_fire(barrels))
                .sum();
            cost.shore += shore * pressure * SHORE_EXPOSURE * time;
            if base.stage + 1 >= heavy_from {
                cost.heavy += her.heavy_fire(self.heavy) * pressure * time;
            } else if base.industrial {
                // The cliff mount only joins in after both governors fail.
                cost.heavy += her.heavy_fire(self.heavy) * pressure * docks;
            }
            cost.seconds += time;
        }
        if let Some(boss) = self.boss {
            let mut group = vec![boss];
            group.extend_from_slice(self.escort);
            cost.ships += self.fight(her, &group, pressure);
            let sails = (FLAGSHIP_SORTIE - self.head_start).max(FLAGSHIP_EARLIEST);
            let sinking: f32 = group.iter().map(|&c| her.time_on(c)).sum();
            cost.seconds = cost.seconds.max(sails) + sinking;
        }
        let shot_down = her.anti_air / (her.anti_air + AA_HALF);
        cost.air = self.air * (1. - shot_down) * pressure * cost.seconds;
        cost
    }

    /// What the passage would cost her: the damage she would take, against her hull and
    /// what her crew mends on the way. Every hit lowers what the crew can mend a section
    /// back to, so a battered ship mends less.
    fn cost(&self, her: &Her) -> f32 {
        let cost = self.reckon(her);
        let sound = her.hull / her.full.max(1.);
        cost.damage() / (her.hull + MENDED * sound).max(1.)
    }

    /// Damage she takes sinking `group`, each ship firing on her until it goes down. She
    /// sinks first the ship that does her most harm for the time it takes to sink. A
    /// big squadron spreads out as it comes on, so she fights it [`ENGAGED`] at a time.
    fn fight(&self, her: &Her, group: &[EnemyClass], pressure: f32) -> f32 {
        let mut ships: Vec<(f32, f32)> = (group.iter())
            .map(|&class| (her.threat(class, self.round) * pressure, her.time_on(class)))
            .collect();
        ships.sort_by(|a, b| (b.0 / b.1).total_cmp(&(a.0 / a.1)));
        let mut damage = 0.;
        for wave in ships.chunks(ENGAGED) {
            let mut clock = 0.;
            for (threat, time) in wave {
                clock += time;
                damage += threat * clock;
            }
        }
        damage
    }
}

/// How hard a passage looks for her as she stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Rating {
    Easy,
    Fair,
    Hard,
    Deadly,
}
impl Rating {
    pub const ALL: [Self; 4] = [Self::Easy, Self::Fair, Self::Hard, Self::Deadly];
    /// The cost of a passage of each rating, at most: what it costs her hull against her
    /// hull and what her crew mends on the way. A practised captain's stock ship comes
    /// through an anchorage at about 0.6 with half her hull or more, and a round-two
    /// passage at about 1.2 with a third to a half of it.
    const BOUNDS: [f32; 3] = [0.5, 1., 2.2];
    fn of(cost: f32) -> Self {
        let rank = Self::BOUNDS.iter().filter(|&&b| cost >= b).count();
        Self::ALL[rank]
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Easy => "Easy",
            Self::Fair => "Fair",
            Self::Hard => "Hard",
            Self::Deadly => "Deadly",
        }
    }
    /// One for an easy passage, up to four for a deadly one.
    pub fn level(self) -> usize {
        self as usize + 1
    }
}

/// One change to her that would make a passage easier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Need {
    /// Piercing caps: her rounds go through heavier plate.
    Penetration,
    /// Every turret rebored a step, as far as its cradle takes.
    BiggerGuns,
    /// Another turret like her heaviest.
    MoreGuns,
    /// Quicker loaders on the main battery.
    Loading,
    /// More secondaries, or a second barrel on each.
    Secondaries,
    /// More side plate.
    Armor,
    /// Another pair of AA guns.
    AntiAir,
    /// Her hull mended.
    Repairs,
}
impl Need {
    pub const ALL: [Self; 8] = [
        Self::Penetration,
        Self::BiggerGuns,
        Self::MoreGuns,
        Self::Loading,
        Self::Secondaries,
        Self::Armor,
        Self::AntiAir,
        Self::Repairs,
    ];
    /// What she needs, as a captain would ask for it: "Needs penetration, armor".
    pub fn name(self) -> &'static str {
        match self {
            Self::Penetration => "penetration",
            Self::BiggerGuns => "bigger guns",
            Self::MoreGuns => "more guns",
            Self::Loading => "faster loading",
            Self::Secondaries => "secondaries",
            Self::Armor => "armor",
            Self::AntiAir => "AA",
            Self::Repairs => "repairs",
        }
    }
    /// Her as she would be with this, or `None` if she can't have it.
    fn apply(self, run: &Run, her: &Her) -> Option<Her> {
        let fitted = |card: Card| {
            (run.fittings.can_fit(card)).then(|| Her::of(&run.ship, &run.fittings.with(card)))
        };
        match self {
            Self::Penetration => fitted(Card::Caps),
            Self::Loading => fitted(Card::Loader),
            Self::Armor => fitted(Card::Armor),
            Self::AntiAir => fitted(Card::MoreAntiAir),
            // Another gun a side while her deck has room, or else a second barrel on each.
            Self::Secondaries => {
                let room = run.fittings.secondaries().per_side < run.wing_room();
                if room && run.fittings.can_fit(Card::MoreSecondaries) {
                    fitted(Card::MoreSecondaries)
                } else {
                    fitted(Card::TwinSecondaries)
                }
            }
            Self::BiggerGuns => {
                let mut bigger = her.clone();
                let mut rebored = false;
                for t in &mut bigger.turrets {
                    let calibre = t.shell.bore + CALIBRES[0];
                    if let Some(&next) =
                        (CALIBRES.iter()).find(|&&c| c > calibre && c <= t.capacity)
                    {
                        let drill = shipvivor_campaign::drilled(run.fittings.loader);
                        t.shell.bore = next - CALIBRES[0];
                        t.reload = (t.shell.load_seconds() * drill).max(LAYING);
                        rebored = true;
                    }
                }
                rebored.then_some(bigger)
            }
            Self::MoreGuns => {
                let room = her.turrets.len() < run.fittings.beam.most_turrets();
                let heaviest = (her.turrets.iter())
                    .max_by_key(|t| (t.shell.bore, t.barrels as u16))
                    .copied()?;
                let mut more = her.clone();
                more.turrets.push(Turret {
                    bearing: 1.,
                    ..heaviest
                });
                room.then_some(more)
            }
            Self::Repairs => (her.hull < 0.85 * her.full).then(|| Her {
                hull: her.full,
                ..her.clone()
            }),
        }
    }
}

/// How a passage looks for her.
#[derive(Clone, Debug, PartialEq)]
pub struct Outlook {
    /// The damage it would cost her against her hull and what her crew mends on the way:
    /// about half a fresh ship's in an anchorage, and past two she would likely be lost.
    pub cost: f32,
    pub rating: Rating,
    /// What would help her most, the most telling first: none for an easy or a fair
    /// passage, and at most two.
    pub needs: Vec<Need>,
}
/// A need is named only if it takes at least this share off the cost.
const TELLING: f32 = 0.04;

/// How `exit` looks for `run`'s ship as she stands.
pub fn outlook(run: &Run, exit: &Exit) -> Outlook {
    let her = Her::of(&run.ship, &run.fittings);
    let sea = Sea::of(exit);
    let cost = sea.cost(&her);
    let rating = Rating::of(cost);
    let mut needs: Vec<(f32, Need)> = if rating >= Rating::Hard {
        (Need::ALL.iter())
            .filter_map(|&need| {
                let better = sea.cost(&need.apply(run, &her)?);
                (cost - better >= TELLING * cost).then_some((cost - better, need))
            })
            .collect()
    } else {
        Vec::new()
    };
    needs.sort_by(|a, b| b.0.total_cmp(&a.0));
    Outlook {
        cost,
        rating,
        needs: needs.into_iter().take(2).map(|(_, need)| need).collect(),
    }
}

/// What `exit` would cost `run`'s ship as she stands, part by part, before her crew's
/// mending: for checking the estimate against the simulator.
pub fn reckoning(run: &Run, exit: &Exit) -> Reckoning {
    Sea::of(exit).reckon(&Her::of(&run.ship, &run.fittings))
}

/// How the anchorage `stage` looks for a ship fresh from the yard.
pub fn anchorage(run: &Run, stage: Stage) -> Outlook {
    outlook(run, &Exit::new(stage, shipvivor_campaign::Grade::Moderate))
}

#[cfg(test)]
mod tests {
    use super::*;
    use shipvivor_campaign::Grade;

    fn stock() -> Run {
        Run::default()
    }

    /// The stock ship with `turrets` on her centreline, as (bore past four inches,
    /// barrels), each loading at the stock pace for its bore.
    fn armed(turrets: &[(u16, f32)]) -> Her {
        let run = stock();
        Her {
            turrets: (turrets.iter())
                .map(|&(bore, barrels)| {
                    let shell = MainShell {
                        bore,
                        ..MainShell::STOCK
                    };
                    Turret {
                        shell,
                        barrels,
                        reload: shell.load_seconds(),
                        bearing: 1.,
                        capacity: 18,
                    }
                })
                .collect(),
            ..Her::of(&run.ship, &run.fittings)
        }
    }

    #[test]
    fn small_guns_sink_light_hulls_sooner_and_big_guns_battleships() {
        let alone = |her: Her| Her {
            secondaries: [Side::default(); 2],
            ..her
        };
        let small = alone(armed(&[(0, 1.)]));
        let big = alone(armed(&[(8, 1.)]));
        assert!(small.kill_rate(Patrol) > 1.5 * big.kill_rate(Patrol));
        assert!(small.kill_rate(Destroyer) > big.kill_rate(Destroyer));
        let biggest = alone(armed(&[(14, 1.)]));
        assert!(biggest.kill_rate(Battleship) > 2.5 * small.kill_rate(Battleship));
    }

    #[test]
    fn caps_tell_on_a_battleship_and_not_on_a_destroyer() {
        let her = armed(&[(4, 2.)]);
        let mut capped = her.clone();
        capped.turrets[0].shell.caps = 2;
        assert!(capped.kill_rate(Battleship) > 1.15 * her.kill_rate(Battleship));
        let light = (capped.kill_rate(Destroyer) / her.kill_rate(Destroyer) - 1.).abs();
        assert!(light < 0.05, "{light}");
    }

    #[test]
    fn plate_wears_down_light_rounds_and_not_a_battleships() {
        let run = stock();
        let bare = Her::of(&run.ship, &run.fittings);
        let plated = Her {
            thickness: 7.,
            ..bare.clone()
        };
        assert!(plated.threat(Patrol, 2) < 0.9 * bare.threat(Patrol, 2));
        // Only a battleship's secondaries are turned, not her main battery.
        assert!(plated.threat(Battleship, 2) > 0.85 * bare.threat(Battleship, 2));
        assert_eq!(plated.threat(TorpedoBoat, 2), bare.threat(TorpedoBoat, 2));
    }

    #[test]
    fn many_quick_guns_suit_flotillas_and_a_few_big_ones_a_battle_line() {
        let flotillas = Sea::of(&Exit::new(Stage::PetrelSound, Grade::Tough));
        let battle_line = Sea::of(&Exit::new(Stage::WhalebackReach, Grade::Tough));
        let leaning = |her: &Her| battle_line.cost(her) / flotillas.cost(her);
        let quick = armed(&[(0, 2.); 6]);
        let heavy = armed(&[(14, 2.); 2]);
        assert!(leaning(&quick) > 1.2 * leaning(&heavy));
    }

    #[test]
    fn a_stock_ship_finds_the_anchorages_easier_than_the_last_patrol() {
        let run = stock();
        for stage in shipvivor_campaign::anchorages(7) {
            let first = anchorage(&run, stage);
            assert!(first.rating <= Rating::Fair, "{stage:?} {first:?}");
        }
        let last = outlook(&run, &Exit::new(Stage::CitadelBay, Grade::Tough));
        assert_eq!(last.rating, Rating::Deadly, "{last:?}");
        assert!(!last.needs.is_empty() && last.needs.len() <= 2, "{last:?}");
    }

    #[test]
    fn a_battered_hull_makes_a_passage_harder() {
        let mut run = stock();
        let exit = Exit::new(Stage::OuterRoad, Grade::Tough);
        let sound = outlook(&run, &exit);
        for _ in 0..12 {
            run.ship.strike_hull(12., 5.);
            run.ship.strike_hull(25., 5.);
        }
        let battered = outlook(&run, &exit);
        assert!(battered.cost > 1.3 * sound.cost, "{sound:?} {battered:?}");
    }
}
