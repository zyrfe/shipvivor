use super::{
    Battery, Battle, Blow, Body, Effect, EffectKind, Events, Landmark, MainShell, Orders, Port,
    Raider, Vec2, WingGun, YardState, bearing, body,
};
use glam::{Vec3, Vec3Swizzles};
use shipvivor_artillery::{Ammo, Impact, ShellArc};
use shipvivor_shipyard::Reach;

pub const SIGHT_LIMIT: f32 = 1400.;
/// Metres per second of a light anti-aircraft round.
pub(super) const AA_SPEED: f32 = 300.;
/// A round that holes the player runs on this far inside her before it bursts.
const ROUND_RUN: f32 = 3.;
/// Height of the tops of her bays above the water, a little under her deck at 1.55 m.
const BAY_TOP: f32 = 1.2;

/// The part of a round's run into her, from where it struck along its flight, that
/// lies low enough to reach her bays. `None` if the whole run stays above them, as a
/// shot skimming the deck does.
fn below_bays(at: Vec3, travel: Vec3) -> Option<[Vec3; 2]> {
    let end = at + travel * ROUND_RUN;
    let (high, low) = (at.y, end.y);
    if high.min(low) > BAY_TOP {
        return None;
    }
    if (high - low).abs() < 1e-6 || high.max(low) <= BAY_TOP {
        return Some([at, end]);
    }
    let level = at + (end - at) * ((high - BAY_TOP) / (high - low));
    Some(if high > BAY_TOP {
        [level, end]
    } else {
        [at, level]
    })
}

pub struct Shot {
    pub body: Body,
    pub battery: Battery,
    origin: Vec2,
    velocity: Vec2,
    arc: ShellArc,
    age: f32,
    damage: u16,
    pub shell: MainShell,
    /// Visible projectile body width on the compressed sea, in scene metres.
    pub diameter: f32,
    /// Seconds after firing that a time-fused round bursts, if nothing stops it first.
    fuse: Option<f32>,
}

impl Shot {
    pub fn position(&self) -> Vec3 {
        self.point(self.age)
    }
    pub fn tangent(&self) -> Vec3 {
        Vec3::new(self.velocity.x, self.arc.slope(self.age), self.velocity.y).normalize()
    }
    /// Where on the chart the round left the muzzle.
    pub fn origin(&self) -> Vec2 {
        self.origin
    }
    /// Inches across the round. Every battery's round is drawn to the size of its gun,
    /// so its width is its calibre: under three inches from secondaries and aircraft,
    /// about five from a patrol boat, eight from a cruiser and twelve from a battleship
    /// or a shore gun.
    pub fn calibre(&self) -> f32 {
        self.diameter / 0.0254
    }
    fn point(&self, time: f32) -> Vec3 {
        let p = self.origin + self.velocity * time;
        Vec3::new(p.x, self.arc.height(time), p.y)
    }
}

/// Where a round goes: out along a straight line over the water at its muzzle speed,
/// rising and falling on its arc.
#[derive(Clone, Copy, Debug)]
pub struct Trajectory {
    origin: Vec2,
    velocity: Vec2,
    arc: ShellArc,
    /// Seconds from leaving the gun to reaching the aim, or coming down at its reach.
    pub duration: f32,
}
impl Trajectory {
    /// Where the round is `t` seconds after it leaves the gun. Past `duration` it keeps
    /// falling along the same arc.
    pub fn point(&self, t: f32) -> Vec3 {
        let p = self.origin + self.velocity * t;
        Vec3::new(p.x, self.arc.height(t), p.y)
    }
    /// Where it reaches the aim, or the point at its reach if the aim lies beyond.
    pub fn end(&self) -> Vec3 {
        self.point(self.duration)
    }
    /// Seconds until it comes down to `height`, on the way down.
    pub fn descends_to(&self, height: f32) -> Option<f32> {
        self.arc
            .contact(0., f32::MAX, height, height)
            .filter(|&t| t > 0.)
    }
}

/// The same arcade flight is used to train a mount and launch its round.
#[derive(Clone, Copy)]
pub(super) struct FlightPath {
    pub heading: f32,
    pub pitch: f32,
    muzzle: Vec2,
    velocity: Vec2,
    arc: ShellArc,
    shell: MainShell,
    diameter: f32,
    /// Seconds from the muzzle to the aim, or to the round's reach.
    pub duration: f32,
}

impl FlightPath {
    pub fn to(mount: Vec3, target: Vec3, battery: Battery) -> Option<Self> {
        let barrel = match battery {
            Battery::Main => 4.1 * MainShell::STOCK.mount_scale(),
            Battery::Secondary | Battery::EnemySecondary | Battery::Aircraft => 1.8,
            Battery::AntiAir => 1.1,
            Battery::Raider => 2.8,
            Battery::Shore => 8.,
        };
        Self::from_barrel(mount, target, battery, barrel)
    }

    pub fn from_barrel(mount: Vec3, target: Vec3, battery: Battery, barrel: f32) -> Option<Self> {
        Self::solve(mount, target, battery, barrel, MainShell::STOCK)
    }
    pub fn main(mount: Vec3, target: Vec3, shell: MainShell) -> Option<Self> {
        Self::solve(
            mount,
            target,
            Battery::Main,
            4.1 * shell.mount_scale(),
            shell,
        )
    }
    fn solve(
        mount: Vec3,
        target: Vec3,
        battery: Battery,
        barrel: f32,
        shell: MainShell,
    ) -> Option<Self> {
        let (speed, reach) = ballistics(battery, shell);
        let offset = target.xz() - mount.xz();
        let dir = offset.try_normalize()?;
        let muzzle = mount.xz() + dir * barrel.min(offset.length() * 0.5);
        let distance = muzzle.distance(target.xz()).min(reach);
        let duration = (distance / speed).max(0.015);
        let arc = ShellArc::between(mount.y, target.y, duration);
        let diameter = match battery {
            Battery::Main => shell.diameter_metres(),
            Battery::Secondary | Battery::EnemySecondary | Battery::Aircraft => 0.07,
            Battery::AntiAir => 0.05,
            // The same gun asset is drawn at a different scale on each enemy class.
            Battery::Raider => 0.15 * barrel / 3.2,
            Battery::Shore => 0.32,
        };
        Some(Self {
            diameter,
            shell,
            muzzle,
            velocity: dir * speed,
            arc,
            heading: bearing(dir),
            pitch: arc.slope(0.).atan2(speed),
            duration,
        })
    }

    pub fn trajectory(&self) -> Trajectory {
        Trajectory {
            origin: self.muzzle,
            velocity: self.velocity,
            arc: self.arc,
            duration: self.duration,
        }
    }

    /// Send the round a barrel actually holds, which may not be the one now selected.
    pub fn load(&mut self, ammo: Ammo) {
        self.shell.ammo = ammo;
    }

    pub fn offset_muzzle(&mut self, lateral: f32) {
        self.muzzle += shipvivor_compass::direction(self.heading).perp() * lateral;
    }

    pub fn launch(
        self,
        shots: &mut Vec<Shot>,
        effects: &mut Vec<Effect>,
        serial: &mut u64,
        battery: Battery,
        damage: u16,
    ) {
        effects.push(Effect {
            body: body(serial, self.muzzle, self.heading),
            height: self.arc.height(0.),
            kind: EffectKind::Muzzle(battery),
            age: 0.,
        });
        shots.push(Shot {
            body: body(serial, self.muzzle, self.heading),
            battery,
            origin: self.muzzle,
            velocity: self.velocity,
            arc: self.arc,
            age: 0.,
            damage,
            shell: self.shell,
            diameter: self.diameter,
            fuse: (battery == Battery::AntiAir).then_some(self.duration),
        });
    }
}

/// Muzzle speed and greatest reach of a battery's rounds.
pub(super) fn ballistics(battery: Battery, shell: MainShell) -> (f32, f32) {
    match battery {
        Battery::Main => (shell.speed(), shell.reach()),
        Battery::AntiAir => (AA_SPEED, 2. * AA_SPEED),
        Battery::Secondary => (240., 290.),
        Battery::EnemySecondary | Battery::Aircraft => (240., 405.),
        Battery::Raider => (245., 545.),
        Battery::Shore => (280., super::SHORE_RANGE + 35.),
    }
}

/// Where a ship making `way` will be when a round from `from` reaches her.
pub(super) fn lead(from: Vec2, target: Vec2, way: Vec2, battery: Battery) -> Vec2 {
    let speed = ballistics(battery, MainShell::STOCK).0;
    let mut at = target;
    for _ in 0..3 {
        at = target + way * (from.distance(at) / speed);
    }
    at
}

pub(super) fn fire(
    shots: &mut Vec<Shot>,
    effects: &mut Vec<Effect>,
    serial: &mut u64,
    mount: Vec3,
    target: Vec3,
    battery: Battery,
    damage: u16,
) {
    if let Some(path) = FlightPath::to(mount, target, battery) {
        path.launch(shots, effects, serial, battery, damage);
    }
}

/// Entry and exit of a swept point through a circular footprint.
pub(super) fn footprint(from: Vec2, to: Vec2, center: Vec2, radius: f32) -> Option<(f32, f32)> {
    let offset = from - center;
    let delta = to - from;
    let a = delta.length_squared();
    let c = offset.length_squared() - radius * radius;
    if a < 1e-10 {
        return (c <= 0.).then_some((0., 1.));
    }
    let b = offset.dot(delta);
    let disc = b * b - a * c;
    if disc < 0. {
        return None;
    }
    let root = disc.sqrt();
    let entry = ((-b - root) / a).max(0.);
    let exit = ((-b + root) / a).min(1.);
    (entry <= exit).then_some((entry, exit))
}

fn local(raider: &Raider, p: Vec2) -> Vec2 {
    let f = raider.body.forward();
    let d = p - raider.body.pos;
    let hull = raider.class.profile().hull;
    let half = (hull.bow + hull.stern) * 0.5;
    Vec2::new(
        d.dot(f.perp()) / hull.radius,
        (d.dot(f) - (hull.bow - half)) / half,
    )
}

#[derive(Clone, Copy)]
enum ShipPart {
    Hull,
    Tower,
    Mount(usize),
    SecondaryMount(usize),
}

impl Raider {
    fn contacts(&self, a: Vec2, b: Vec2, mut visit: impl FnMut((f32, f32), f32, f32, ShipPart)) {
        let p = self.class.profile();
        if footprint(a, b, self.body.pos, p.hull.bow + 3.).is_none() {
            return;
        }
        for (i, gun) in self.guns().iter().enumerate().filter(|(_, g)| g.live()) {
            let center = gun.base(&self.body, p);
            if let Some(span) = footprint(a, b, center.xz(), gun.radius(p)) {
                visit(span, p.deck, gun.top(p), ShipPart::Mount(i));
            }
        }
        for (i, gun) in self
            .secondaries
            .iter()
            .enumerate()
            .filter(|(_, g)| g.live())
        {
            let at = gun.station(&self.body, p);
            if let Some(span) = footprint(a, b, at.xz(), 0.84) {
                visit(span, p.deck, p.deck + 1., ShipPart::SecondaryMount(i));
            }
        }
        let hull = if self.class == super::EnemyClass::Patrol {
            footprint(local(self, a), local(self, b), Vec2::ZERO, 1.)
        } else {
            self.hull_span(a, b)
        };
        if let Some(span) = hull {
            // The small patrol silhouette keeps its forgiving acquisition volume.
            let roof = if self.class == super::EnemyClass::Patrol {
                3.8
            } else {
                p.deck
            };
            visit(span, 0., roof, ShipPart::Hull);
        }
        // The central tower is also a hull target after the flagship is stripped.
        if let Some(span) = footprint(a, b, self.body.pos, p.hull.radius * 0.67) {
            let tiers = if self.class.is_flagship() { 3.5 } else { 2.6 };
            visit(span, p.deck, p.deck + tiers * p.gun_scale, ShipPart::Tower);
        }
    }
}

#[derive(Clone, Copy)]
enum Contact {
    Water,
    Land,
    /// A piece of ice, by its index among the battle's icebergs.
    Ice(usize),
    /// Her hull, or with `upperworks` the bridge and funnel standing above the deck.
    Player {
        upperworks: bool,
    },
    Raider(usize, ShipPart),
    Port(usize, PortPart),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PortPart {
    Dock,
    Factory(usize),
    Gun(usize),
    Ground,
}

impl super::Port {
    /// The gunsight and flying rounds use the same building and terrain volumes.
    fn contacts(&self, a: Vec2, b: Vec2, mut visit: impl FnMut((f32, f32), f32, f32, PortPart)) {
        if let Some(span) = self.yard_span(a, b) {
            visit(span, 3.2, self.yard_top(), PortPart::Dock);
        }
        for i in 0..self.factories().len() {
            if let Some(span) = self.factory_span(i, a, b) {
                visit(span, 3.2, self.factory_top(i), PortPart::Factory(i));
            }
        }
        for (i, gun) in self.turrets().iter().enumerate() {
            if let Some(span) = footprint(a, b, gun.position(&self.body), 5.) {
                visit(span, 0., 3.9, PortPart::Ground);
                if gun.live() {
                    visit(span, 3.9, 7.5, PortPart::Gun(i));
                }
            }
        }
        for (center, radius, top) in self.land_volumes() {
            if let Some(span) = footprint(a, b, center, radius) {
                visit(span, 0., top, PortPart::Ground);
            }
        }
    }
}

// A structure can stand on a low shelf while a bluff or another building masks it.
// Use the same three-dimensional sightline for the crew and the player's reticle.
fn first_visible(
    eye: Vec3,
    end: Vec3,
    raiders: &[Raider],
    ports: &[Port],
    landmarks: &[Landmark],
    ice: &[super::Iceberg],
) -> Option<(f32, Contact)> {
    let hit = |span: Option<(f32, f32)>, bottom: f32, top: f32| {
        let (mut enter, mut exit) = span?;
        let dy = end.y - eye.y;
        if dy.abs() < 1e-5 {
            return (bottom..=top).contains(&eye.y).then_some(enter);
        }
        let a = (bottom - eye.y) / dy;
        let b = (top - eye.y) / dy;
        enter = enter.max(a.min(b));
        exit = exit.min(a.max(b));
        (enter <= exit).then_some(enter)
    };
    let mut best: Option<(f32, Contact)> = None;
    let mut record = |time: Option<f32>, contact| {
        if let Some(t) = time
            && best.is_none_or(|(old, _)| t < old)
        {
            best = Some((t, contact));
        }
    };
    for (i, raider) in raiders.iter().enumerate() {
        raider.contacts(eye.xz(), end.xz(), |span, low, high, part| {
            record(hit(Some(span), low, high), Contact::Raider(i, part));
        });
    }
    for island in landmarks {
        for (center, radius, top) in island.volumes() {
            record(
                hit(footprint(eye.xz(), end.xz(), center, radius), 0., top),
                Contact::Land,
            );
        }
    }
    for (i, berg) in ice.iter().enumerate() {
        let (center, radius, top) = berg.volume();
        record(
            hit(footprint(eye.xz(), end.xz(), center, radius), 0., top),
            Contact::Ice(i),
        );
    }
    for (i, port) in ports.iter().enumerate() {
        port.contacts(eye.xz(), end.xz(), |span, low, high, part| {
            record(hit(Some(span), low, high), Contact::Port(i, part));
        });
    }
    best
}

impl WingGun {
    pub(super) fn shore_target(
        &self,
        orders: &Orders,
        range: f32,
        raiders: &[Raider],
        ports: &[Port],
        landmarks: &[Landmark],
        ice: &[super::Iceberg],
    ) -> Option<Vec3> {
        let eye = self.pivot(orders.player, orders.heading);
        let mut nearest = None;
        let mut limit = range * range;
        for (i, port) in ports
            .iter()
            .enumerate()
            .filter(|(_, p)| p.yard_state() != YardState::Ruined)
        {
            let dock = (
                PortPart::Dock,
                port.yard_position(),
                (3.2 + port.yard_top()) * 0.5,
            );
            let guns = port
                .turrets()
                .iter()
                .enumerate()
                .filter(|(_, g)| g.live())
                .map(|(j, g)| (PortPart::Gun(j), g.position(&port.body), 5.7));
            let factories = port
                .factories()
                .iter()
                .enumerate()
                .filter(|(_, f)| f.live())
                .map(|(j, _)| {
                    (
                        PortPart::Factory(j),
                        port.factory_position(j),
                        (3.2 + port.factory_top(j)) * 0.5,
                    )
                });
            for (part, pos, height) in std::iter::once(dock).chain(guns).chain(factories) {
                let distance = pos.distance_squared(orders.player);
                if distance > limit || !self.covers(orders.player, orders.heading, pos) {
                    continue;
                }
                let point = pos.extend(height).xzy();
                if matches!(first_visible(eye, point, raiders, ports, landmarks, ice),
                    Some((_,Contact::Port(hit, object))) if hit == i && object == part)
                {
                    nearest = Some(point);
                    limit = distance;
                }
            }
        }
        nearest
    }
}

#[derive(Clone, Copy, Default)]
pub struct Sighting {
    pub point: Vec3,
    pub yard: Option<usize>,
    /// The port and factory under the crosshair. Both indices start at zero.
    pub factory: Option<(usize, usize)>,
    pub shore_turret: Option<(usize, usize)>,
    pub ship: Option<super::EnemyClass>,
    pub ship_id: Option<u64>,
    /// Number painted on the spotting display, counting from one.
    pub turret: Option<usize>,
    pub director: Option<bool>,
}

impl Battle {
    pub fn sight_point(&self, eye: Vec3, ray: Vec3, fallback: Vec2) -> Vec3 {
        self.sight(eye, ray, fallback).point
    }

    /// First visible contact, shared by the main battery and the spotting display.
    pub fn sight(&self, eye: Vec3, ray: Vec3, fallback: Vec2) -> Sighting {
        let sea = Vec3::new(fallback.x, 0., fallback.y);
        let length = (sea.xz() - eye.xz()).length() / ray.xz().length().max(0.01);
        let end = eye + ray * length;
        let best = first_visible(
            eye,
            end,
            &self.raiders,
            &self.ports,
            &self.landmarks,
            &self.floe.icebergs,
        );
        let mut sight = Sighting {
            point: sea,
            ..Sighting::default()
        };
        if let Some((time, object)) = best {
            sight.point = eye.lerp(end, time);
            match object {
                Contact::Port(i, PortPart::Gun(j)) => sight.shore_turret = Some((i, j)),
                Contact::Port(i, PortPart::Dock) => sight.yard = Some(i),
                Contact::Port(i, PortPart::Factory(j)) => sight.factory = Some((i, j)),
                Contact::Raider(i, part) => {
                    let r = &self.raiders[i];
                    sight.ship = Some(r.class);
                    sight.ship_id = Some(r.body.id);
                    sight.turret = match part {
                        ShipPart::Mount(i) => Some(i + 1),
                        ShipPart::SecondaryMount(i) => Some(r.class.main_count() + i + 1),
                        ShipPart::Hull | ShipPart::Tower => None,
                    };
                    sight.director = (matches!(part, ShipPart::Tower)
                        && r.class == super::EnemyClass::Battleship)
                        .then_some(!r.director_lost);
                }
                _ => {}
            }
        }
        sight
    }

    pub(super) fn advance_shots(&mut self, dt: f32, player: Vec2, events: &mut Events) {
        let (forward, hull) = self.keel;
        let vertices = hull.outline().map(Vec2::from);
        let local =
            |p: Vec2| Vec2::new((p - player).dot(forward.perp()), -(p - player).dot(forward));
        let mut bursts = Vec::new();
        self.shots.retain_mut(|shot| {
            let start = shot.age;
            // A fused round goes no farther than its burst.
            let step = shot.fuse.map_or(dt, |fuse| (fuse - start).clamp(0., dt));
            let finish = start + step;
            let a = shot.body.pos;
            let b = shot.point(finish).xz();
            let contact_time = |span: Option<(f32, f32)>, low, high| {
                let (enter, leave) = span?;
                shot.arc
                    .contact(start + enter * step, start + leave * step, low, high)
            };
            let mut first = shot
                .arc
                .contact(start, finish, 0., 0.)
                .map(|t| (t, Contact::Water));
            let mut consider = |time: Option<f32>, contact| {
                if let Some(t) = time
                    && first.is_none_or(|(old, _)| t < old)
                {
                    first = Some((t, contact));
                }
            };
            let friendly = matches!(
                shot.battery,
                Battery::Main | Battery::Secondary | Battery::AntiAir
            );
            for (i, berg) in self.floe.icebergs.iter().enumerate() {
                let (center, radius, top) = berg.volume();
                consider(
                    contact_time(footprint(a, b, center, radius), 0., top),
                    Contact::Ice(i),
                );
            }
            for island in &self.landmarks {
                for (center, radius, top) in island.volumes() {
                    consider(
                        contact_time(footprint(a, b, center, radius), 0., top),
                        Contact::Land,
                    );
                }
            }
            for (i, port) in self.ports.iter().enumerate() {
                port.contacts(a, b, |span, low, high, part| {
                    let object = if friendly {
                        Contact::Port(i, part)
                    } else {
                        Contact::Land
                    };
                    consider(contact_time(Some(span), low, high), object);
                });
            }
            if friendly {
                for (i, r) in self.raiders.iter().enumerate().filter(|(_, r)| r.afloat()) {
                    r.contacts(a, b, |span, low, high, part| {
                        consider(
                            contact_time(Some(span), low, high),
                            Contact::Raider(i, part),
                        );
                    });
                }
            } else {
                let start = local(a);
                let delta = local(b) - start;
                consider(
                    contact_time(super::plating::clip_hull(&vertices, start, delta), 0., 1.55),
                    Contact::Player { upperworks: false },
                );
                // The bridge is taller than the deck, and occupies only the fore superstructure.
                let bridge_z = self.deck.bridge_z;
                let bridge = [
                    Vec2::new(-1.85, -5.1 + bridge_z),
                    Vec2::new(1.85, -5.1 + bridge_z),
                    Vec2::new(1.85, 0.9 + bridge_z),
                    Vec2::new(-1.85, 0.9 + bridge_z),
                ];
                // Her forward funnel stands within it. A round that bursts up there
                // reaches nothing below the deck.
                consider(
                    contact_time(super::plating::clip_hull(&bridge, start, delta), 1.55, 4.6),
                    Contact::Player { upperworks: true },
                );
            }
            if let Some((time, object)) = first {
                let p = shot.point(time);
                let travel = Vec3::new(shot.velocity.x, shot.arc.slope(time), shot.velocity.y)
                    .normalize_or_zero();
                let mut heading = shot.body.heading;
                let shell = shot.shell.at_contact(time);
                let mut armor_effect = |outcome: Impact, normal: Vec3| {
                    let out = if outcome == Impact::Bounced {
                        travel.reflect(normal)
                    } else {
                        normal
                    };
                    if out.xz().length_squared() > 0.0001 {
                        heading = bearing(out.xz());
                    }
                    EffectKind::Armor {
                        outcome,
                        bore: shot.shell.bore,
                        battery: shot.battery,
                    }
                };
                if matches!(object, Contact::Raider(..)) {
                    events.hits += 1;
                }
                let mut effect_at = p;
                let kind = match object {
                    Contact::Water => EffectKind::Splash,
                    Contact::Ice(i) => {
                        self.floe.icebergs[i].integrity -= f32::from(shot.damage);
                        EffectKind::Burst
                    }
                    Contact::Land | Contact::Port(_, PortPart::Ground) => EffectKind::Burst,
                    Contact::Player { upperworks } => {
                        events.damage = events.damage.saturating_add(shot.damage);
                        let at = local(p.xz());
                        let plan = |q: Vec3| {
                            let at = local(q.xz());
                            (hull.bow + at.y, at.x)
                        };
                        let track = (!upperworks)
                            .then(|| below_bays(p, travel))
                            .flatten()
                            .map(|ends| ends.map(plan));
                        events.blows.push(Blow {
                            aft: (hull.bow + at.y).clamp(0., hull.bow + hull.stern),
                            abeam: at.x,
                            damage: shot.damage,
                            calibre: Some(shot.calibre()),
                            reach: Reach::Track(track),
                        });
                        EffectKind::Burst
                    }
                    Contact::Port(i, PortPart::Dock) => {
                        let port = &mut self.ports[i];
                        let standing = port.live_factories() as u8;
                        if port.damage_yard(shot.damage) {
                            events.recovered += self
                                .rewards
                                .at_once(super::DRYDOCK_STORES + super::FACTORY_STORES * standing);
                            effect_at = port.yard_position().extend(3.2).xzy();
                            EffectKind::Demolition { dock: true }
                        } else {
                            EffectKind::Burst
                        }
                    }
                    Contact::Port(i, PortPart::Factory(j)) => {
                        let port = &mut self.ports[i];
                        if port.damage_factory(j, shot.damage) {
                            events.recovered += self.rewards.at_once(super::FACTORY_STORES);
                            effect_at = port.factory_position(j).extend(3.2).xzy();
                            EffectKind::Demolition { dock: false }
                        } else {
                            EffectKind::Burst
                        }
                    }
                    Contact::Port(i, PortPart::Gun(j)) => {
                        let gun = &mut self.ports[i].turrets[j];
                        gun.strength = gun.strength.saturating_sub(shot.damage);
                        EffectKind::Burst
                    }
                    Contact::Raider(i, ShipPart::SecondaryMount(index)) => {
                        let gun = &mut self.raiders[i].secondaries[index];
                        gun.strength = gun.strength.saturating_sub(shot.damage);
                        if !gun.live() {
                            self.effects.push(Effect {
                                body: body(&mut self.serial, p.xz(), shot.body.heading),
                                height: p.y,
                                kind: EffectKind::MountLost(super::EnemyClass::Patrol),
                                age: 0.,
                            });
                        }
                        armor_effect(Impact::Penetrated, -travel)
                    }
                    Contact::Raider(i, ShipPart::Mount(index)) => {
                        let r = &mut self.raiders[i];
                        let (outcome, normal) = r.strike_mount(
                            index,
                            p,
                            travel,
                            (shot.shell.ammo, shell),
                            shot.shell.bore,
                        );
                        let dealt = outcome.dealt(shot.damage);
                        if dealt > 0 && r.hit_gun(index, dealt) {
                            let gun = &r.guns[index];
                            let base = gun.base(&r.body, r.class.profile());
                            self.effects.push(Effect {
                                body: body(&mut self.serial, base.xz(), gun.bearing(&r.body)),
                                height: base.y,
                                kind: EffectKind::MountLost(r.class),
                                age: 0.,
                            });
                        }
                        if dealt > 0 && r.class == super::EnemyClass::Patrol {
                            r.hit_hull(p.xz(), dealt);
                        }
                        armor_effect(outcome, normal)
                    }
                    // The Parliament's bridge can be shot away while her turrets still
                    // turn every round from the hull.
                    Contact::Raider(i, ShipPart::Tower) if self.raiders[i].bridge_standing() => {
                        let r = &mut self.raiders[i];
                        let (outcome, normal) = r.strike_hull(
                            p,
                            travel,
                            (shot.shell.ammo, shell),
                            shot.shell.bore,
                            true,
                        );
                        let dealt = outcome.dealt(shot.damage);
                        if dealt > 0 && r.hit_bridge(dealt) {
                            self.effects.push(Effect {
                                body: body(&mut self.serial, p.xz(), shot.body.heading),
                                height: p.y,
                                kind: EffectKind::Burst,
                                age: 0.,
                            });
                        }
                        armor_effect(outcome, normal)
                    }
                    Contact::Raider(i, ShipPart::Hull | ShipPart::Tower)
                        if self.raiders[i].protected() =>
                    {
                        let normal = (p.xz() - self.raiders[i].body.pos)
                            .normalize_or_zero()
                            .extend(0.)
                            .xzy();
                        armor_effect(Impact::Bounced, normal)
                    }
                    Contact::Raider(i, part) => {
                        let r = &mut self.raiders[i];
                        let tower = matches!(part, ShipPart::Tower);
                        let (outcome, normal) = r.strike_hull(
                            p,
                            travel,
                            (shot.shell.ammo, shell),
                            shot.shell.bore,
                            tower,
                        );
                        let dealt = outcome.dealt(shot.damage);
                        if dealt > 0 {
                            if tower && r.class == super::EnemyClass::Battleship {
                                r.disable_director();
                            } else {
                                r.hit_hull(p.xz(), dealt);
                            }
                        }
                        armor_effect(outcome, normal)
                    }
                };
                if let EffectKind::Armor {
                    battery: Battery::Main,
                    outcome,
                    ..
                } = kind
                {
                    self.last_strike = Some((outcome, 1.35));
                }
                self.effects.push(Effect {
                    body: body(&mut self.serial, effect_at.xz(), heading),
                    height: effect_at.y.max(0.),
                    kind,
                    age: 0.,
                });
                return false;
            }
            if shot.fuse.is_some_and(|fuse| finish >= fuse) {
                bursts.push((shot.point(finish), shot.damage));
                return false;
            }
            shot.age = finish;
            shot.body.pos = b;
            true
        });
        for (at, damage) in bursts {
            self.burst_flak(at, damage);
        }
    }
}
