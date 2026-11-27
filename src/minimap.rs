//! North-up chart. All symbols use the simulation's world positions.
use crate::state::{Campaign, Encounter, PlayerHelm, Screen};
use bevy::{
    asset::RenderAssetUsages,
    image::ImageSampler,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use shipvivor_campaign::Stage;
use shipvivor_encounter::{Battle, EnemyClass, ISLAND_RADIUS, Sortie};
use shipvivor_navigation::Helm;

const SIZE: i32 = 192;
/// The chart is drawn at half again its pixel size, which is three device pixels to one
/// chart pixel on a Retina display at full UI scale, so the symbols stay crisp.
pub const SHOWN: f32 = SIZE as f32 * 1.5;
const METRES_PER_PIXEL: f32 = 12.5;
/// Metres of an enemy torpedo's track drawn behind it.
const TORPEDO_TRACK: f32 = 90.;
type Ink = [u8; 4];
const WATER: Ink = [25, 57, 68, 255];
const GRID: Ink = [46, 79, 86, 255];
const LAND: Ink = [178, 190, 192, 255];
/// Land under an ice cap, and ice adrift.
const GLACIER: Ink = [224, 236, 240, 255];
const ICE: Ink = [168, 226, 240, 255];
const CLEARED: Ink = [93, 226, 213, 255];
const ENEMY: Ink = [255, 111, 93, 255];
/// A heavy battery's reach, while it has no plot of her.
const REACH: Ink = [158, 82, 72, 255];
const GOLD: Ink = [255, 200, 94, 255];
const WHITE: Ink = [246, 248, 229, 255];
/// Backing behind the flagship's crown, so it reads over land and grid alike.
const SHADOW: Ink = [21, 38, 44, 255];
/// Her own reach is ringed round her: the main battery's in her white, and the
/// secondaries' in the green of their refit cards. Pale ink is lost on land and ice, so
/// there each ring darkens the ground instead, in a deep shade of its own colour.
const MAIN_INKS: [Ink; 2] = [WHITE, SHADOW];
const SECONDARY_INKS: [Ink; 2] = [[132, 224, 120, 255], [36, 96, 44, 255]];
/// Ground at least this bright, on a 0–255 luma scale, takes a ring's dark ink.
const LIGHT_GROUND: f32 = 128.;
/// How far each of her rings tints the chart it crosses: faint, but plain on water,
/// land and ice alike.
const MAIN_TINT: f32 = 0.34;
const SECONDARY_TINT: f32 = 0.5;
/// Width of her rings in chart pixels. Their edges are smoothed, so a ring easing to a
/// new reach moves by fractions of a pixel instead of stepping.
const RING_WIDTH: f32 = 1.5;
/// Seconds a ring of hers takes to ease out to a new reach after a refit.
const RING_EASE: f32 = 1.;
/// How much more a ring tints the chart at the middle of its ease, so the eye catches it.
const RING_GLOW: f32 = 0.8;

/// One of her range rings, in metres: where it eases from, the reach it eases to, and
/// how far along it is.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Swing {
    from: f32,
    to: f32,
    /// Seconds since the ring set out for `to`. It has arrived at [`RING_EASE`].
    age: f32,
}
impl Swing {
    fn settled(reach: f32) -> Self {
        Self {
            from: reach,
            to: reach,
            age: RING_EASE,
        }
    }
    fn moving(&self) -> bool {
        self.age < RING_EASE
    }
    /// The reach drawn now, eased in and out on a cubic: slow to leave, quick through
    /// the middle, slow to arrive.
    fn shown(&self) -> f32 {
        let t = (self.age / RING_EASE).clamp(0., 1.);
        let eased = if t < 0.5 {
            4. * t * t * t
        } else {
            1. - (2. - 2. * t).powi(3) / 2.
        };
        self.from.lerp(self.to, eased)
    }
    /// Rises and falls across the ease, nothing once the ring has arrived.
    fn glow(&self) -> f32 {
        if self.moving() {
            (std::f32::consts::PI * self.age / RING_EASE).sin()
        } else {
            0.
        }
    }
    /// Head for `reach`, or `dt` seconds further toward it. A new reach takes up from
    /// wherever the ring stands, so a second refit mid-ease never jumps it.
    fn follow(&mut self, reach: f32, dt: f32) {
        if reach == self.to {
            self.age = (self.age + dt).min(RING_EASE);
        } else {
            *self = Self {
                from: self.shown(),
                to: reach,
                age: 0.,
            };
        }
    }
}

/// Her main battery's ring and her secondaries'.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Rings {
    main: Swing,
    secondaries: Swing,
}
impl Rings {
    /// Metres her main battery and her secondaries reach as she is fitted now, or
    /// nothing for a battery she doesn't carry.
    fn reach(run: &Campaign, battle: &Battle) -> [f32; 2] {
        let round = run.fittings.round();
        let main = battle
            .main_mounts()
            .iter()
            .map(|mount| mount.round(round).reach())
            .fold(0., f32::max);
        let secondaries = if battle.wing_guns().is_empty() {
            0.
        } else {
            battle.secondaries().range
        };
        [main, secondaries]
    }
    fn settled([main, secondaries]: [f32; 2]) -> Self {
        Self {
            main: Swing::settled(main),
            secondaries: Swing::settled(secondaries),
        }
    }
    fn moving(&self) -> bool {
        self.main.moving() || self.secondaries.moving()
    }
    fn follow(&mut self, [main, secondaries]: [f32; 2], dt: f32) {
        self.main.follow(main, dt);
        self.secondaries.follow(secondaries, dt);
    }
}

#[derive(Resource)]
pub struct ChartImage(pub Handle<Image>);

pub fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let mut image = Image::new_fill(
        Extent3d {
            width: SIZE as u32,
            height: SIZE as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &WATER,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.sampler = ImageSampler::nearest();
    commands.insert_resource(ChartImage(images.add(image)));
}

struct Chart<'a> {
    pixels: &'a mut [u8],
    center: Vec2,
}
impl Chart<'_> {
    fn project(&self, pos: Vec2) -> IVec2 {
        ((pos - self.center) / METRES_PER_PIXEL).round().as_ivec2() + IVec2::splat(SIZE / 2)
    }

    fn keep_on_chart(&self, point: IVec2, margin: i32) -> IVec2 {
        let middle = IVec2::splat(SIZE / 2);
        let offset = (point - middle).as_vec2();
        let limit = (SIZE / 2 - margin) as f32;
        middle + (offset * (limit / offset.abs().max_element().max(limit))).as_ivec2()
    }

    fn dot(&mut self, p: IVec2, color: Ink) {
        if (0..SIZE).contains(&p.x) && (0..SIZE).contains(&p.y) {
            let i = ((p.y * SIZE + p.x) * 4) as usize;
            self.pixels[i..i + 4].copy_from_slice(&color);
        }
    }

    fn line(&mut self, a: IVec2, b: IVec2, color: Ink) {
        let span = (b - a).abs().max_element().max(1);
        for i in 0..=span {
            self.dot(a + (b - a) * i / span, color);
        }
    }

    // Letters correspond to the objective strip. A short leader reaches the actual dock
    // so the badge never substitutes for its position on the coastline. A base that
    // sleeps further up the channel gets no badge until it is roused.
    fn port_badges(&mut self, battle: &Battle) {
        const LETTERS: [[u8; 7]; crate::mission_hud::MAX_BASES] = [
            [14, 17, 17, 31, 17, 17, 17],
            [30, 17, 17, 30, 17, 17, 30],
            [14, 17, 16, 16, 16, 17, 14],
            [28, 18, 17, 17, 17, 18, 28],
            [31, 16, 16, 30, 16, 16, 31],
            [31, 16, 16, 30, 16, 16, 16],
            [14, 17, 16, 23, 17, 17, 15],
            [17, 17, 17, 31, 17, 17, 17],
        ];
        let mut placed = Vec::with_capacity(battle.ports().len());
        for (index, port) in battle.ports().iter().enumerate() {
            if !crate::mission_hud::charted(port) {
                continue;
            }
            let clear = port.wrecked();
            let pixel = self.project(crate::mission_hud::anchor(port));
            let anchor = self.keep_on_chart(pixel, 2);
            let side = if anchor.x >= SIZE / 2 { 1 } else { -1 };
            let desired = anchor + IVec2::new(side * 10, -8);
            let fits = |p: IVec2| p.clamp(IVec2::splat(9), IVec2::splat(SIZE - 10));
            let center = [
                IVec2::ZERO,
                IVec2::new(0, 14),
                IVec2::new(0, -14),
                IVec2::new(18, 0),
                IVec2::new(-18, 0),
                IVec2::new(0, 28),
                IVec2::new(0, -28),
            ]
            .into_iter()
            .map(|bump| fits(desired + bump))
            .find(|p| {
                placed
                    .iter()
                    .all(|q: &IVec2| (q.x - p.x).abs() > 16 || (q.y - p.y).abs() > 12)
            })
            .unwrap_or_else(|| fits(desired));
            placed.push(center);
            let border = if clear { [132, 178, 175, 255] } else { ENEMY };
            self.line(anchor, center, border);
            let w = if clear { 7 } else { 5 };
            for y in -5i32..=5 {
                let edge = w - i32::from(y.abs() == 5);
                self.line(
                    center + IVec2::new(-edge, y),
                    center + IVec2::new(edge, y),
                    border,
                );
                if y.abs() < 5 {
                    self.line(
                        center + IVec2::new(-edge + 1, y),
                        center + IVec2::new(edge - 1, y),
                        [21, 38, 44, 255],
                    );
                }
            }
            let x = center.x - if clear { 5 } else { 2 };
            for (r, row) in LETTERS[index.min(LETTERS.len() - 1)].iter().enumerate() {
                for c in 0..5 {
                    if row & (1 << (4 - c)) != 0 {
                        self.dot(
                            IVec2::new(x + c, center.y - 3 + r as i32),
                            if clear { border } else { WHITE },
                        );
                    }
                }
            }
            if clear {
                self.line(
                    center + IVec2::new(1, 0),
                    center + IVec2::new(3, 2),
                    CLEARED,
                );
                self.line(
                    center + IVec2::new(3, 2),
                    center + IVec2::new(5, -2),
                    CLEARED,
                );
            }
            if let Some(base) = battle.air_station().filter(|base| base.port == index) {
                let ink = if base.operational(port) { ENEMY } else { GRID };
                let p = center + IVec2::new(0, 7);
                self.line(p - IVec2::X * 4, p + IVec2::X * 4, ink);
                self.line(p - IVec2::Y, p + IVec2::Y * 2, ink);
            }
        }
    }

    /// A circle of dots `gap` pixels apart.
    fn dotted(&mut self, center: IVec2, radius: f32, gap: f32, color: Ink) {
        let steps = (std::f32::consts::TAU * radius / gap).max(8.) as usize;
        for k in 0..steps {
            let turn = Vec2::from_angle(k as f32 / steps as f32 * std::f32::consts::TAU);
            self.dot(center + (turn * radius).round().as_ivec2(), color);
        }
    }

    fn ring(&mut self, center: IVec2, radius: i32, color: Ink) {
        let steps = (radius * 8).max(8);
        for k in 0..steps {
            let turn = Vec2::from_angle(k as f32 / steps as f32 * std::f32::consts::TAU);
            self.dot(center + (turn * radius as f32).round().as_ivec2(), color);
        }
    }

    /// Mixes `amount` of `color` into the pixel already there.
    fn tint(&mut self, p: IVec2, color: Ink, amount: f32) {
        if (0..SIZE).contains(&p.x) && (0..SIZE).contains(&p.y) {
            let i = ((p.y * SIZE + p.x) * 4) as usize;
            for (under, over) in self.pixels[i..i + 3].iter_mut().zip(color) {
                let under_f = f32::from(*under);
                *under = (under_f + (f32::from(over) - under_f) * amount).round() as u8;
            }
        }
    }

    /// Whether the pixel at `p`, which must be on the chart, is land or ice rather than
    /// water.
    fn light(&self, p: IVec2) -> bool {
        let i = ((p.y * SIZE + p.x) * 4) as usize;
        let [r, g, b] = [0, 1, 2].map(|c| f32::from(self.pixels[i + c]));
        0.3 * r + 0.59 * g + 0.11 * b >= LIGHT_GROUND
    }

    /// A ring [`RING_WIDTH`] wide and `radius` pixels round the middle of pixel
    /// `center`, tinting what it crosses by `strength`: with the first ink over water,
    /// and the second over light ground. Its edges are smoothed.
    fn halo(&mut self, center: IVec2, radius: f32, [on_water, on_land]: [Ink; 2], strength: f32) {
        if radius < 1. {
            return;
        }
        let half = RING_WIDTH / 2. + 0.5;
        let span = (radius + half).ceil() as i32;
        let lo = (center - span).max(IVec2::ZERO);
        let hi = (center + span).min(IVec2::splat(SIZE - 1));
        for y in lo.y..=hi.y {
            for x in lo.x..=hi.x {
                let p = IVec2::new(x, y);
                let off = (p - center).as_vec2().length();
                let cover = (half - (off - radius).abs()).min(1.);
                if cover > 0. {
                    let ink = if self.light(p) { on_land } else { on_water };
                    self.tint(p, ink, strength * cover);
                }
            }
        }
    }

    /// Her own reach, drawn round the middle of the chart where she always is.
    fn reach_rings(&mut self, rings: &Rings) {
        let middle = IVec2::splat(SIZE / 2);
        for (swing, inks, tint) in [
            (rings.main, MAIN_INKS, MAIN_TINT),
            (rings.secondaries, SECONDARY_INKS, SECONDARY_TINT),
        ] {
            let strength = (tint * (1. + RING_GLOW * swing.glow())).min(1.);
            self.halo(middle, swing.shown() / METRES_PER_PIXEL, inks, strength);
        }
    }

    /// A ringed crown that stays on the chart edge wherever she is. For a while after
    /// she puts to sea the crown blinks and rings spread from her, so the eye finds her.
    fn flagship(&mut self, battle: &Battle) {
        let Some(ship) = battle.raiders().iter().find(|r| r.class.is_flagship()) else {
            return;
        };
        let p = self.keep_on_chart(self.project(ship.body.pos), 9);
        let fresh = match battle.flagship() {
            Some(Sortie::Sailed { at, .. } | Sortie::Sighted { at }) => {
                Some(battle.at_sea() - at).filter(|&age| age < crate::flagship_hud::HERALD_SECONDS)
            }
            _ => None,
        };
        match fresh {
            Some(age) => {
                for lag in [0., 0.5] {
                    let spread = (age + lag).fract();
                    let ink = if spread < 0.5 { GOLD } else { ENEMY };
                    self.ring(p, 8 + (spread * 16.) as i32, ink);
                }
            }
            None => self.ring(p, 8, GOLD),
        }
        for y in -4..=5 {
            self.line(p + IVec2::new(-7, y), p + IVec2::new(7, y), SHADOW);
        }
        let crown = if fresh.is_some_and(|age| (age * 4.).fract() < 0.5) {
            WHITE
        } else {
            GOLD
        };
        // Three points over a red band.
        for x in -6_i32..=6 {
            let h = if x.abs() == 6 || x == 0 { 6 } else { 3 };
            self.line(p + IVec2::new(x, 2 - h), p + IVec2::new(x, 2), crown);
        }
        self.line(p + IVec2::new(-6, 4), p + IVec2::new(6, 4), ENEMY);
    }

    fn draw(&mut self, helm: &Helm, battle: &Battle, rings: &Rings) {
        self.pixels.as_chunks_mut::<4>().0.fill(WATER);
        let origin = self.project(Vec2::ZERO);
        // Sixteen pixels to one 200-metre grid square.
        for x in (origin.x.rem_euclid(16)..SIZE).step_by(16) {
            self.line(IVec2::new(x, 0), IVec2::new(x, SIZE - 1), GRID);
        }
        for y in (origin.y.rem_euclid(16)..SIZE).step_by(16) {
            self.line(IVec2::new(0, y), IVec2::new(SIZE - 1, y), GRID);
        }

        for island in battle.landmarks() {
            let c = self.project(island.body.pos);
            let r = (island.radius / METRES_PER_PIXEL).round() as i32;
            let ground = if island.kind == shipvivor_encounter::LandmarkKind::Glacier {
                GLACIER
            } else {
                LAND
            };
            for y in -r..=r {
                let w = ((r * r - y * y) as f32).sqrt() as i32;
                self.line(c + IVec2::new(-w, y), c + IVec2::new(w, y), ground);
            }
            // A lighthouse shows its light until it is brought down.
            if island.kind == shipvivor_encounter::LandmarkKind::Beacon
                && island.features.iter().all(|tower| tower.standing())
            {
                self.dot(c, WHITE);
            }
        }
        // Bergs big enough to hurt show on the plot. Growlers are too low to pick out.
        for berg in battle
            .icebergs()
            .iter()
            .filter(|b| b.radius >= shipvivor_encounter::AVOIDED_RADIUS)
        {
            let c = self.project(berg.body.pos);
            let r = (berg.radius / METRES_PER_PIXEL).round() as i32;
            for y in -r..=r {
                let w = ((r * r - y * y) as f32).sqrt() as i32;
                self.line(c + IVec2::new(-w, y), c + IVec2::new(w, y), ICE);
            }
        }
        for port in battle.ports() {
            let c = self.project(port.body.pos);
            let radius = (ISLAND_RADIUS / METRES_PER_PIXEL).ceil() as i32;
            for y in -radius..=radius {
                for x in -radius..=radius {
                    if x * x + y * y <= radius * radius {
                        self.dot(c + IVec2::new(x, y), LAND);
                    }
                }
            }
            // Grey once wrecked, red while a dock burns or lies down, white while the
            // base has no steel anywhere, and gold while it builds.
            let starved = port.stock == 0 && port.docks().iter().all(|d| d.laid() == 0);
            let status = if port.wrecked() {
                GRID
            } else if port.docks().iter().any(|d| {
                matches!(
                    d.state(),
                    shipvivor_encounter::YardState::Burning
                        | shipvivor_encounter::YardState::Ruined
                )
            }) {
                ENEMY
            } else if starved {
                WHITE
            } else {
                GOLD
            };
            // A sleeping base is only an island on the chart.
            if !crate::mission_hud::charted(port) {
                continue;
            }
            self.line(c - IVec2::X, c + IVec2::X, status);
            for slip in 0..port.docks().len() {
                let dock = self.project(port.dock_berth(slip));
                self.line(c, dock, [223, 211, 163, 255]);
                self.line(dock - IVec2::X, dock + IVec2::X, WHITE);
            }
            for gun in port.turrets().iter().filter(|g| g.live()) {
                let at = self.project(gun.position(&port.body));
                self.dot(at, ENEMY);
                // A heavy battery's reach is dotted round it, the water it can shell. The
                // dots brighten while it is ranging on her.
                if let Some(plot) = gun.ranging() {
                    let ink = if plot.ranging() { ENEMY } else { REACH };
                    self.dotted(at, gun.reach() / METRES_PER_PIXEL, 5., ink);
                }
            }
        }
        // Over the coast and under everything that moves.
        self.reach_rings(rings);
        for cargo in battle.salvage() {
            let p = self.project(cargo.body.pos);
            let width = if cargo.stores >= 3 { 2 } else { 1 };
            self.line(p - IVec2::X * width, p + IVec2::X * width, GOLD);
            self.line(p - IVec2::Y, p + IVec2::Y, GOLD);
        }
        for (plane, port) in battle.spotters().iter().zip(battle.ports()) {
            if port.wrecked() || !plane.airborne() {
                continue;
            }
            let p = self.project(plane.position);
            let nose = shipvivor_compass::direction(plane.heading);
            let wing = (nose.perp() * 3.).round().as_ivec2();
            let ink = if plane.alert_left > 0. { WHITE } else { GOLD };
            self.line(p - wing, p + wing, ink);
            self.line(
                p - (nose * 3.).round().as_ivec2(),
                p + (nose * 2.).round().as_ivec2(),
                ink,
            );
        }
        if let Some(plane) = battle.air_station().and_then(|base| base.aircraft.as_ref()) {
            let p = self.keep_on_chart(self.project(plane.body.pos), 6);
            let nose = plane.body.forward();
            let wing = (nose.perp() * 5.).round().as_ivec2();
            let tip = p + (nose * 2.).round().as_ivec2();
            self.line(p - wing, tip, ENEMY);
            self.line(tip, p + wing, ENEMY);
            self.line(tip, p - (nose * 4.).round().as_ivec2(), ENEMY);
        }
        // Harriers are the strafer's mark drawn small.
        for plane in battle.harriers() {
            let p = self.project(plane.body.pos);
            let nose = plane.body.forward();
            let wing = (nose.perp() * 3.).round().as_ivec2();
            let tip = p + nose.round().as_ivec2();
            self.line(p - wing, tip, ENEMY);
            self.line(tip, p + wing, ENEMY);
            self.line(tip, p - (nose * 2.).round().as_ivec2(), ENEMY);
        }
        // An airfield's bombers and torpedo planes are the strafer's mark.
        for plane in battle.warplanes() {
            let p = self.project(plane.body.pos);
            let nose = plane.body.forward();
            let wing = (nose.perp() * 4.).round().as_ivec2();
            let tip = p + (nose * 2.).round().as_ivec2();
            self.line(p - wing, tip, ENEMY);
            self.line(tip, p + wing, ENEMY);
            self.line(tip, p - (nose * 3.).round().as_ivec2(), ENEMY);
        }
        // An enemy torpedo is a white head on the red line of its track, so its run
        // across her course can be read off the chart.
        for fish in battle.torpedoes().iter().filter(|t| t.hostile()) {
            let p = self.project(fish.body.pos);
            let track = fish.run().min(TORPEDO_TRACK);
            let tail = self.project(fish.body.pos - fish.body.forward() * track);
            self.line(tail, p, ENEMY);
            self.dot(p, WHITE);
        }
        for raider in battle.raiders() {
            let boss = raider.class.is_flagship();
            if raider.class == EnemyClass::Coaster {
                // A coaster is a small gold block, with no route drawn: they are many,
                // and they trade only with the next yard along.
                let p = self.project(raider.body.pos);
                for y in [0, 1] {
                    self.line(p + IVec2::new(-2, y), p + IVec2::new(2, y), GOLD);
                }
                continue;
            }
            if raider.class == EnemyClass::Freighter {
                let p = self.keep_on_chart(self.project(raider.body.pos), 4);
                if let Some(port) = raider.destination_port().map(|i| &battle.ports()[i])
                    && !port.wrecked()
                    && !raider.is_disabled()
                {
                    let to = self.project(port.berth());
                    let from = self.project(raider.body.pos);
                    let steps = (to - from).abs().max_element().max(1);
                    for k in (3..steps).step_by(4) {
                        self.dot(from + (to - from) * k / steps, [171, 148, 96, 255]);
                    }
                }
                for y in [-1, 0, 1] {
                    self.line(p + IVec2::new(-3, y), p + IVec2::new(3, y), GOLD);
                }
                self.dot(p - IVec2::new(2, 2), WHITE);
                continue;
            }
            // The flagship is drawn last, over everything else.
            if boss {
                continue;
            }
            if raider.body.pos.distance_squared(helm.position)
                <= shipvivor_encounter::SIGHT_LIMIT.powi(2)
            {
                let p = self.project(raider.body.pos);
                let r: i32 = match raider.class {
                    EnemyClass::TorpedoBoat => 1,
                    EnemyClass::Freighter | EnemyClass::Coaster | EnemyClass::Patrol => 2,
                    EnemyClass::Destroyer => 3,
                    EnemyClass::Cruiser => 4,
                    EnemyClass::Battleship
                    | EnemyClass::Commodore
                    | EnemyClass::Parliament
                    | EnemyClass::Regent => 5,
                };
                for y in 0..=r {
                    let span = r - y;
                    self.line(
                        p + IVec2::new(-span, y - 1),
                        p + IVec2::new(span, y - 1),
                        ENEMY,
                    );
                }
                if raider.class == EnemyClass::Battleship {
                    self.line(p + IVec2::new(-r, -3), p + IVec2::new(r, -3), ENEMY);
                }
            }
        }
        self.port_badges(battle);
        self.flagship(battle);
        let c = self.project(helm.position);
        let forward = helm.forward();
        let tip = c + (forward * 6.).round().as_ivec2();
        let tail = c - (forward * 3.).round().as_ivec2();
        let side = (forward.perp() * 3.).round().as_ivec2();
        self.line(tail - side, tip, WHITE);
        self.line(tail + side, tip, WHITE);
        self.line(tail, tip, WHITE);
        self.line(tail - side, tail + side, WHITE);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn update(
    mut images: ResMut<Assets<Image>>,
    chart: Res<ChartImage>,
    run: Res<Campaign>,
    helm: Res<PlayerHelm>,
    battle: Res<Encounter>,
    time: Res<Time>,
    mut rings: Local<Option<Rings>>,
    mut last: Local<Option<(Stage, f32)>>,
) {
    // A new run opens with her rings at their stock reach. Through a patrol's refits,
    // yards and pauses they hold the reach last shown, so a card taken there eases them
    // out once she is under way again.
    let screen = run.screen();
    if !matches!(
        screen,
        Screen::Sailing
            | Screen::Secured
            | Screen::Refit
            | Screen::Drydock(_)
            | Screen::Chart
            | Screen::PassageMap
            | Screen::Intercept(_)
            | Screen::Paused
    ) {
        *rings = None;
    }
    if screen != Screen::Sailing {
        *last = None;
        return;
    }
    let reach = Rings::reach(&run, &battle);
    let was_moving = rings.as_ref().is_some_and(Rings::moving);
    let rings = rings.get_or_insert_with(|| Rings::settled(reach));
    rings.follow(reach, time.delta_secs());
    // An easing ring is redrawn every frame, and once more as it arrives.
    let easing = was_moving || rings.moving();
    if !easing
        && last.is_some_and(|(stage, time)| {
            stage == run.stage && (0.0..0.1).contains(&(run.elapsed - time))
        })
    {
        return;
    }
    *last = Some((run.stage, run.elapsed));
    if let Some(mut image) = images.get_mut(&chart.0)
        && let Some(pixels) = image.data.as_mut()
    {
        Chart {
            pixels,
            center: helm.position,
        }
        .draw(&helm, &battle, rings);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ring_eases_out_to_a_new_reach_slow_fast_slow_and_then_rests() {
        let mut ring = Swing::settled(780.);
        assert!(!ring.moving());
        ring.follow(900., 0.1);
        assert!(ring.moving());
        assert_eq!(ring.shown(), 780.);
        let mut shown = vec![ring.shown()];
        for _ in 0..10 {
            ring.follow(900., RING_EASE / 10.);
            shown.push(ring.shown());
        }
        assert!(!ring.moving());
        assert_eq!(ring.shown(), 900.);
        let steps: Vec<f32> = shown.windows(2).map(|w| w[1] - w[0]).collect();
        assert!(steps.iter().all(|&step| step >= 0.), "{steps:?}");
        // Eased in and out: the middle of the swing covers more ground than either end.
        assert!(
            steps[5] > 3. * steps[0] && steps[5] > 3. * steps[9],
            "{steps:?}"
        );
        // The glow comes and goes with the ease.
        assert_eq!(ring.glow(), 0.);
    }

    #[test]
    fn a_second_refit_mid_ease_takes_up_from_where_the_ring_stands() {
        let mut ring = Swing::settled(160.);
        ring.follow(200., 0.);
        ring.follow(200., RING_EASE / 2.);
        let midway = ring.shown();
        assert!((160. ..200.).contains(&midway));
        assert!(ring.glow() > 0.9);
        ring.follow(240., 0.);
        assert_eq!(ring.shown(), midway);
        for _ in 0..20 {
            ring.follow(240., RING_EASE / 10.);
        }
        assert_eq!(ring.shown(), 240.);
    }

    #[test]
    fn her_rings_tint_the_chart_at_their_reach_and_leave_the_rest_alone() {
        let mut pixels = vec![0; (SIZE * SIZE * 4) as usize];
        pixels.as_chunks_mut::<4>().0.fill(WATER);
        let mut chart = Chart {
            pixels: &mut pixels,
            center: Vec2::ZERO,
        };
        chart.reach_rings(&Rings::settled([780., 160.]));
        let at = |x: i32, y: i32| {
            let i = ((y * SIZE + x) * 4) as usize;
            [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
        };
        let middle = SIZE / 2;
        let main = (780. / METRES_PER_PIXEL).round() as i32;
        let wings = (160. / METRES_PER_PIXEL).round() as i32;
        // Clear water inside, between and outside the rings.
        for x in [middle, middle + wings + 4, middle + main + 4] {
            assert_eq!(at(x, middle), WATER, "x {x}");
        }
        // Faint: brighter than the grid, well short of the colour itself.
        let main_ink = at(middle + main, middle);
        assert!(
            main_ink[0] > GRID[0] + 30 && main_ink[0] < WHITE[0] / 2,
            "{main_ink:?}"
        );
        let wing_ink = at(middle, middle - wings);
        assert!(
            wing_ink[1] > GRID[1] + 30 && wing_ink[1] < SECONDARY_INKS[0][1],
            "{wing_ink:?}"
        );
        assert!(
            wing_ink[1] > wing_ink[0],
            "secondaries read green: {wing_ink:?}"
        );
        assert_eq!(main_ink[3], 255);
    }

    #[test]
    fn over_land_and_ice_her_rings_darken_the_ground_so_they_still_show() {
        for ground in [LAND, GLACIER, ICE] {
            let mut pixels = vec![0; (SIZE * SIZE * 4) as usize];
            pixels.as_chunks_mut::<4>().0.fill(ground);
            let mut chart = Chart {
                pixels: &mut pixels,
                center: Vec2::ZERO,
            };
            chart.reach_rings(&Rings::settled([780., 160.]));
            let middle = SIZE / 2;
            for metres in [780., 160.] {
                let x = middle + (metres / METRES_PER_PIXEL).round() as i32;
                let i = ((middle * SIZE + x) * 4) as usize;
                let luma = |p: &[u8]| {
                    0.3 * f32::from(p[0]) + 0.59 * f32::from(p[1]) + 0.11 * f32::from(p[2])
                };
                let drop = luma(&ground) - luma(&pixels[i..i + 3]);
                assert!(drop > 35., "{metres} m ring on {ground:?}: {drop}");
            }
        }
    }
}
