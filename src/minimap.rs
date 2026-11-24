//! North-up chart. All symbols use the simulation's world positions.
use crate::state::{Campaign, Encounter, PlayerHelm, Screen, TrialCourse};
use bevy::{
    asset::RenderAssetUsages,
    image::ImageSampler,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use shipvivor_campaign::Stage;
use shipvivor_encounter::{Battle, EnemyClass, ISLAND_RADIUS, Sortie};
use shipvivor_navigation::{Course, Helm};

const SIZE: i32 = 192;
const METRES_PER_PIXEL: f32 = 12.5;
type Ink = [u8; 4];
const WATER: Ink = [25, 57, 68, 255];
const GRID: Ink = [46, 79, 86, 255];
const LAND: Ink = [178, 190, 192, 255];
/// Land under an ice cap, and ice adrift.
const GLACIER: Ink = [224, 236, 240, 255];
const ICE: Ink = [168, 226, 240, 255];
const GATE: Ink = [93, 226, 213, 255];
const ENEMY: Ink = [255, 111, 93, 255];
const GOLD: Ink = [255, 200, 94, 255];
const WHITE: Ink = [246, 248, 229, 255];
/// Backing behind the flagship's crown, so it reads over land and grid alike.
const SHADOW: Ink = [21, 38, 44, 255];

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
    // so the badge never substitutes for its position on the coastline.
    fn port_badges(&mut self, battle: &Battle) {
        const LETTERS: [[u8; 7]; 3] = [
            [14, 17, 17, 31, 17, 17, 17],
            [30, 17, 17, 30, 17, 17, 30],
            [14, 17, 16, 16, 16, 17, 14],
        ];
        let mut placed = Vec::with_capacity(3);
        for (index, port) in battle.ports().iter().enumerate() {
            let clear = port.yard_state() == shipvivor_encounter::YardState::Ruined;
            let pixel = self.project(port.yard_position());
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
            for (r, row) in LETTERS[index].iter().enumerate() {
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
                self.line(center + IVec2::new(1, 0), center + IVec2::new(3, 2), GATE);
                self.line(center + IVec2::new(3, 2), center + IVec2::new(5, -2), GATE);
            }
        }
    }

    fn ring(&mut self, center: IVec2, radius: i32, color: Ink) {
        let steps = (radius * 8).max(8);
        for k in 0..steps {
            let turn = Vec2::from_angle(k as f32 / steps as f32 * std::f32::consts::TAU);
            self.dot(center + (turn * radius as f32).round().as_ivec2(), color);
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
            Some(Sortie::Sailed { at, .. }) => {
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

    fn draw(&mut self, helm: &Helm, battle: &Battle, course: &Course) {
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
            if island.kind == shipvivor_encounter::LandmarkKind::Beacon {
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
            let status = match port.yard_state() {
                shipvivor_encounter::YardState::Ruined => GRID,
                shipvivor_encounter::YardState::Burning => ENEMY,
                _ if port.stock == 0 => WHITE,
                _ => GOLD,
            };
            self.line(c - IVec2::X, c + IVec2::X, status);
            let dock = self.project(port.berth());
            self.line(c, dock, [223, 211, 163, 255]);
            self.line(dock - IVec2::X, dock + IVec2::X, WHITE);
            for gun in port.turrets().iter().filter(|g| g.live()) {
                self.dot(self.project(gun.position(&port.body)), ENEMY);
            }
        }
        for (i, gate) in course.gates().iter().enumerate() {
            let active = i == course.active();
            let mut p = self.project(gate.center);
            if active {
                // Keep the next bearing visible even after sailing off the plotted course.
                p = self.keep_on_chart(p, 5);
            }
            let r = if active { 4 } else { 2 };
            let ink = if active { GATE } else { [115, 164, 162, 255] };
            for sign in [-1, 1] {
                let mid = p + IVec2::X * sign * r;
                self.line(mid - IVec2::Y * r, mid + IVec2::Y * r, ink);
                self.dot(mid - IVec2::Y * r - IVec2::X * sign, ink);
                self.dot(mid + IVec2::Y * r - IVec2::X * sign, ink);
            }
        }
        for cargo in battle.salvage() {
            let p = self.project(cargo.body.pos);
            let width = if cargo.stores >= 3 { 2 } else { 1 };
            self.line(p - IVec2::X * width, p + IVec2::X * width, GOLD);
            self.line(p - IVec2::Y, p + IVec2::Y, GOLD);
        }
        for (plane, port) in battle.spotters().iter().zip(battle.ports()) {
            if port.yard_state() == shipvivor_encounter::YardState::Ruined {
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
        for raider in battle.raiders() {
            let boss = raider.class.is_flagship();
            if raider.class == EnemyClass::Freighter {
                let p = self.keep_on_chart(self.project(raider.body.pos), 4);
                if let Some(port) = raider.destination_port().map(|i| &battle.ports()[i])
                    && port.yard_state() != shipvivor_encounter::YardState::Ruined
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
                    EnemyClass::Freighter | EnemyClass::Patrol => 2,
                    EnemyClass::Destroyer => 3,
                    EnemyClass::Cruiser => 4,
                    EnemyClass::Battleship | EnemyClass::Parliament | EnemyClass::Regent => 5,
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

pub fn update(
    mut images: ResMut<Assets<Image>>,
    chart: Res<ChartImage>,
    run: Res<Campaign>,
    helm: Res<PlayerHelm>,
    battle: Res<Encounter>,
    course: Res<TrialCourse>,
    mut last: Local<Option<(Stage, f32)>>,
) {
    if run.screen() != Screen::Sailing {
        *last = None;
        return;
    }
    if last.is_some_and(|(stage, time)| {
        stage == run.stage && (0.0..0.1).contains(&(run.elapsed - time))
    }) {
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
        .draw(&helm, &battle, &course);
    }
}
