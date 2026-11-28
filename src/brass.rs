//! Spent brass from her secondaries' gunhouses.
//!
//! A gunhouse throws each round's empty case out of the back of its mount as the breech
//! opens after the shot. The case tumbles onto the deck, rings, bounces and comes to
//! rest on its side. There it rolls as she heels, rolls and turns, slides along its
//! length less readily, knocks into the cases already lying about and the turrets and
//! deckhouses in its way, and drops over the side into the sea. Each of her main
//! battery's salvos makes the loose brass on deck jump.
//!
//! The cases live in her own frame, the frame of the [`Ship`] they are drawn under,
//! where her deck is still and the world swings about it: gravity leans as she rolls,
//! and her turning and changes of speed press the brass toward her side or her ends.
//! Under heavy fire they pile up until the oldest give way.
use crate::{
    scene::{self, Look, Ship, Turret, WING_HOUSE_LIFT, WING_HOUSE_RING, WingPosition},
    seascape::Sea,
    sound::Soundboard,
    state::{Campaign, Embarked, Encounter, PlayerHelm, Screen},
};
use bevy::{light::NotShadowCaster, prelude::*};
use shipvivor_acoustics::Sound;
use shipvivor_encounter::{Battery, EffectKind};
use std::f32::consts::FRAC_PI_2;

/// The most cases about her at once. Past this the oldest are cleared away.
const MOST: usize = 900;
/// A four-inch case's length and radius in metres, drawn a little large to read at the
/// distance she is watched from. A larger bore's grows with it.
const CASE: (f32, f32) = (0.62, 0.075);
/// Seconds from the shot until the case is thrown out, and the seconds between barrels.
const BREECH: (f32, f32) = (0.22, 0.42);
const STAGGER: f32 = 0.07;
/// Metres per second a case is thrown out of the back of the gunhouse, up and to the side.
const THROW: [(f32, f32); 3] = [(2.6, 4.4), (1.8, 3.2), (-1., 1.)];
/// Radians per second a case tumbles as it leaves the breech.
const TUMBLE: f32 = 18.;
const GRAVITY: f32 = 9.81;
/// Friction against the push on a case lying on the deck: along its length it slides,
/// across it it rolls, which takes very little.
const SLIDE: f32 = 0.32;
const ROLL: f32 = 0.008;
/// The share of a case's speed into the deck it bounces back with, and of its speed
/// along the deck it keeps at each bounce.
const BOUNCE: (f32, f32) = (0.34, 0.72);
/// Metres per second into the deck below which a falling case comes to rest on it.
const SETTLE: f32 = 0.9;
/// A case is wider at its base than its mouth, so it rolls round in a circle: radians
/// it turns for each metre it rolls, against its length.
const TAPER: f32 = 0.09;
/// The height of a step up that stops a case rolling into it, in metres.
const STEP: f32 = 0.12;
/// The share of a case's length it keeps clear of the next case by.
const ELBOW: f32 = 0.34;
/// Metres per second, toward or away, that a knock between cases or on a fitting is
/// heard at.
const HEARD: f32 = 0.7;
/// Cases heard each second at most, and the most saved up while she is quiet.
const VOICES: (f32, f32) = (36., 8.);
/// Seconds a case stays quiet after it has been heard.
const HUSH: f32 = 0.09;
/// Metres per second a salvo of her main battery throws the brass beside it up off the
/// deck, and the metres that dies away over.
const JOLT: (f32, f32) = (1.7, 14.);
/// Seconds her changes of speed and heading are smoothed over, as felt on the deck.
const FELT: f32 = 0.25;

pub struct BrassPlugin;
impl Plugin for BrassPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Brass>()
            .add_observer(sweep)
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (eject, tumble, draw)
                    .chain()
                    .after(scene::place_ship)
                    .after(scene::place_wing_guns)
                    .after(scene::place_turrets),
            );
    }
}

/// A spent case drawn about her deck. The pool grows as the brass does.
#[derive(Component)]
pub struct Spent;

/// The mesh and paint every case is drawn with.
#[derive(Resource)]
struct Casting {
    mesh: Handle<Mesh>,
    paint: Handle<StandardMaterial>,
}

/// The brass about her deck, and the rounds whose cases are still to be thrown out.
#[derive(Resource, Default)]
pub struct Brass {
    cases: Vec<Case>,
    due: Vec<Due>,
    /// Times each wing gun had fired at the last look.
    seen: Vec<u32>,
    /// Her main battery's flashes seen at the last look.
    salvos: Vec<u64>,
    /// Her way over the ground at the last look, and the push it gives the deck.
    way: Option<Vec2>,
    felt: Vec2,
    /// Cases that may still be heard this moment.
    voices: f32,
    seed: u64,
    drawn: Vec<Entity>,
}

/// A round fired whose case is still in the breech: its gun, the barrel it came from,
/// its bore and the seconds until it is thrown out.
struct Due {
    gun: usize,
    barrel: f32,
    bore: u16,
    wait: f32,
}

/// A spent case, in her own frame.
struct Case {
    at: Vec3,
    way: Vec3,
    /// How it lies, and how fast it tumbles while in the air.
    turn: Quat,
    spin: Vec3,
    /// Lying on the deck: the way its length points, and how far it has rolled.
    lying: Option<(f32, f32)>,
    length: f32,
    radius: f32,
    /// It has been heard landing.
    landed: bool,
    overboard: bool,
    quiet: f32,
}
impl Case {
    /// The way its length points along the deck when it lies at `yaw`.
    fn along(yaw: f32) -> Vec2 {
        Vec2::new(-yaw.cos(), yaw.sin())
    }
    /// How it is turned lying at `yaw`, rolled `roll` about its length.
    fn lie(yaw: f32, roll: f32) -> Quat {
        Quat::from_rotation_y(yaw) * Quat::from_rotation_z(FRAC_PI_2) * Quat::from_rotation_y(roll)
    }
    /// How far below its middle it reaches, turned as it is.
    fn reach(&self) -> f32 {
        let up = (self.turn * Vec3::Y).y.abs();
        self.radius * (1. - up * up).max(0.).sqrt() + 0.5 * self.length * up
    }
}

impl Brass {
    fn roll(&mut self) -> f32 {
        // A xorshift: cheap, and plenty for brass.
        let mut x = self.seed.max(1);
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.seed = x;
        (x >> 40) as f32 / (1_u64 << 24) as f32
    }
    fn within(&mut self, (low, high): (f32, f32)) -> f32 {
        low + (high - low) * self.roll()
    }
    fn tumble(&mut self, rate: f32) -> Vec3 {
        Vec3::new(
            self.within((-1., 1.)),
            self.within((-1., 1.)),
            self.within((-1., 1.)),
        ) * rate
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // A case narrows a little toward its mouth.
    let mesh = meshes.add(
        ConicalFrustum {
            radius_top: 0.44,
            radius_bottom: 0.5,
            height: 1.,
        }
        .mesh()
        .resolution(10),
    );
    let paint = materials.add(StandardMaterial {
        // Bright and plain like the rest of her paint, with a polished sheen, and a
        // little glow so that it still reads as brass in her shadow.
        base_color: Color::srgb_u8(246, 196, 84),
        emissive: LinearRgba::rgb(0.09, 0.06, 0.015),
        perceptual_roughness: 0.3,
        reflectance: 0.6,
        ..default()
    });
    commands.insert_resource(Casting { mesh, paint });
}

/// A new voyage starts with her deck swept.
fn sweep(_: On<Embarked>, mut brass: ResMut<Brass>) {
    brass.cases.clear();
    brass.due.clear();
    brass.seen.clear();
    brass.salvos.clear();
    brass.way = None;
}

/// Each round a gunhouse fires leaves a case in its breech, thrown out a moment later
/// from the back of the mount.
fn eject(
    time: Res<Time>,
    run: Res<Campaign>,
    battle: Res<Encounter>,
    guns: Query<(&WingPosition, &Transform), Without<Spent>>,
    mut brass: ResMut<Brass>,
) {
    if matches!(run.screen(), Screen::Drydock(_)) {
        brass.cases.clear();
        brass.due.clear();
    }
    let fitted = battle.gunhouses();
    let wing = battle.wing_guns();
    let brass = &mut *brass;
    if brass.seen.len() != wing.len() {
        brass.seen = wing.iter().map(|gun| gun.fired()).collect();
    }
    let sailing = run.under_way();
    for (index, (gun, seen)) in wing.iter().zip(&mut brass.seen).enumerate() {
        // Rounds count only as she fires them. Rewound, or scrubbed along the rewind's
        // timeline, the count follows the battle and nothing is thrown out.
        let fired = gun.fired();
        let new = fired.saturating_sub(*seen);
        *seen = fired;
        // Only the secondary gunhouses throw out brass on deck.
        let gunhouse = gun.emplacement() == shipvivor_encounter::Emplacement::Gunhouse;
        if new == 0 || !sailing || !gunhouse {
            continue;
        }
        let bore = fitted.bore;
        let barrels = fitted.barrels.max(1);
        for round in 0..new.min(2) {
            for barrel in 0..barrels {
                let wait = BREECH.0
                    + (BREECH.1 - BREECH.0) * brass_luck(brass.seed, index, barrel)
                    + STAGGER * barrel as f32
                    + 0.5 * round as f32;
                brass.due.push(Due {
                    gun: index,
                    barrel: barrel as f32 - 0.5 * (barrels - 1) as f32,
                    bore,
                    wait,
                });
            }
        }
    }
    if !run.under_way() {
        return;
    }
    let dt = time.delta_secs();
    for due in &mut brass.due {
        due.wait -= dt;
    }
    let ready: Vec<Due> = brass.due.extract_if(.., |due| due.wait <= 0.).collect();
    for due in ready {
        let Some((_, pose)) = guns.iter().find(|(gun, _)| gun.index() == due.gun) else {
            continue;
        };
        let scale = scene::wing_house_scale(due.bore);
        let ring = WING_HOUSE_RING * scale;
        // Out of the back of the gunhouse, low down, from behind the barrel it served.
        let from = Vec3::new(
            due.barrel * 0.36 * scale,
            WING_HOUSE_LIFT + 0.45 * scale,
            ring * 1.02,
        );
        let throw = Vec3::new(
            brass.within(THROW[2]),
            brass.within(THROW[1]),
            brass.within(THROW[0]),
        );
        let heft = (f32::from(due.bore) / 4.).powf(0.75);
        let spin = brass.tumble(TUMBLE);
        // Thrown base first, its length along the line it leaves on.
        let turn = pose.rotation * Quat::from_rotation_x(FRAC_PI_2 + brass.within((-0.4, 0.4)));
        brass.cases.push(Case {
            at: pose.transform_point(from),
            way: pose.rotation * throw,
            turn,
            spin,
            lying: None,
            length: CASE.0 * heft,
            radius: CASE.1 * heft,
            landed: false,
            overboard: false,
            quiet: 0.,
        });
    }
    if brass.cases.len() > MOST {
        let extra = brass.cases.len() - MOST;
        brass.cases.drain(..extra);
    }
}

/// A share from 0 to 1 that differs from barrel to barrel and gun to gun.
fn brass_luck(seed: u64, gun: usize, barrel: usize) -> f32 {
    let x = seed
        .wrapping_add(gun as u64 * 0x9E37_79B9)
        .wrapping_mul(0x2545_F491_4F6C_DD1D)
        .wrapping_add(barrel as u64 * 0x632B_E5AB);
    ((x >> 40) & 0xFFFF) as f32 / 65_535.
}

/// Something on deck a case knocks against: its middle along her deck and its radius.
struct Post {
    at: Vec2,
    radius: f32,
}

/// Move the brass: tumbling through the air, ringing on the deck, rolling and sliding
/// as she moves under it, knocking into the rest and going over the side.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn tumble(
    time: Res<Time>,
    run: Res<Campaign>,
    battle: Res<Encounter>,
    helm: Res<PlayerHelm>,
    sea: Res<Sea>,
    board: Res<Soundboard>,
    ship: Single<&Transform, (With<Ship>, Without<Spent>)>,
    turrets: Query<(&Transform, &Visibility), (With<Turret>, Without<Spent>)>,
    wing: Query<(&WingPosition, &Transform, &Visibility), Without<Spent>>,
    mut brass: ResMut<Brass>,
) {
    if !run.under_way() {
        brass.way = None;
        return;
    }
    let dt = time.delta_secs().min(0.05);
    if dt <= 0. {
        return;
    }
    let brass = &mut *brass;
    // Her pushes on the deck: her changes of speed and heading, felt backward.
    let way = helm.forward() * helm.speed;
    if let Some(last) = brass.way {
        let push = (way - last) / dt;
        brass.felt += (push - brass.felt) * (1. - (-dt / FELT).exp());
    }
    brass.way = Some(way);
    let ship = **ship;
    let inward = ship.rotation.inverse();
    let pull = inward * (Vec3::NEG_Y * GRAVITY - Vec3::new(brass.felt.x, 0., brass.felt.y));
    brass.voices = (brass.voices + VOICES.0 * dt).min(VOICES.1);

    let design = run.display_design();
    let beam = design.beam_factor();
    let look = Look::of_run(&run);
    let edge = |z: f32| design.deck_edge(-z) * beam;
    let gunhouses = battle.gunhouses();
    let mut posts: Vec<Post> = turrets
        .iter()
        .filter(|(_, shown)| **shown != Visibility::Hidden)
        .map(|(pose, _)| Post {
            at: pose.translation.xz(),
            radius: 1.3 * pose.scale.x,
        })
        .collect();
    let ring = WING_HOUSE_RING * scene::wing_house_scale(gunhouses.bore);
    let housed = |gun: &WingPosition| {
        (battle.wing_guns().get(gun.index()))
            .is_some_and(|g| g.emplacement() == shipvivor_encounter::Emplacement::Gunhouse)
    };
    posts.extend(
        wing.iter()
            .filter(|(gun, _, shown)| **shown != Visibility::Hidden && housed(gun))
            .map(|(_, pose, _)| Post {
                at: pose.translation.xz(),
                radius: ring,
            }),
    );

    // A salvo from her main battery throws the loose brass near it up off the deck.
    let flashes: Vec<(u64, Vec2)> = battle
        .effects()
        .iter()
        .filter(|fx| fx.kind == EffectKind::Muzzle(Battery::Main))
        .map(|fx| {
            let world = Vec3::new(fx.body.pos.x, fx.height, fx.body.pos.y);
            (fx.body.id, (inward * (world - ship.translation)).xz())
        })
        .collect();
    for &(id, at) in &flashes {
        if brass.salvos.contains(&id) {
            continue;
        }
        for k in 0..brass.cases.len() {
            let case = &brass.cases[k];
            if case.overboard {
                continue;
            }
            let hop = JOLT.0 * (-case.at.xz().distance(at) / JOLT.1).exp();
            if hop < 0.25 {
                continue;
            }
            let lift = hop * (0.6 + 0.8 * brass.roll());
            let shove = Vec3::new(brass.within((-0.4, 0.4)), 0., brass.within((-0.4, 0.4)));
            let spin = brass.tumble(TUMBLE * 0.4 * hop);
            let case = &mut brass.cases[k];
            case.way += Vec3::Y * lift + shove * hop;
            case.spin = spin;
            case.lying = None;
        }
    }
    brass.salvos = flashes.into_iter().map(|(id, _)| id).collect();

    // Heard: the sound, where it was made in her frame, and how hard.
    let mut heard: Vec<(Sound, Vec3, f32)> = Vec::new();
    let steps = (dt / 0.01).ceil().max(1.);
    let h = dt / steps;
    for _ in 0..steps as usize {
        let mut k = 0;
        while k < brass.cases.len() {
            let case = &mut brass.cases[k];
            case.quiet = (case.quiet - h).max(0.);
            let gone = if let Some((yaw, roll)) = case.lying {
                roll_on_deck(case, (yaw, roll), pull, h, &look, &edge, &mut heard)
            } else {
                fly(case, pull, h, &look, &edge, &mut heard);
                // Over the side, until she leaves it in her wake.
                case.overboard && {
                    let world = ship.transform_point(case.at);
                    let sunk = world.y < sea.height(world.xz());
                    if sunk {
                        heard.push((Sound::CasePlunk, case.at, 1.));
                    }
                    sunk || case.at.y < -4.
                }
            };
            if gone {
                brass.cases.swap_remove(k);
            } else {
                k += 1;
            }
        }
    }
    knock(&mut brass.cases, &posts, &mut heard);

    // The loudest are heard, as many as the moment has room for.
    heard.sort_by(|a, b| b.2.total_cmp(&a.2));
    for (sound, at, _) in heard {
        if brass.voices < 1. {
            break;
        }
        brass.voices -= 1.;
        let serial = (brass.roll() * 1e6) as u64;
        board.emit(sound, ship.transform_point(at), serial);
    }
}

/// A case in the air, falling as she leans under it, until it strikes her deck and
/// either bounces or comes to rest, or misses her and goes over the side.
fn fly(
    case: &mut Case,
    pull: Vec3,
    h: f32,
    look: &Look,
    edge: &impl Fn(f32) -> f32,
    heard: &mut Vec<(Sound, Vec3, f32)>,
) {
    case.way += pull * h;
    case.at += case.way * h;
    case.turn = (Quat::from_scaled_axis(case.spin * h) * case.turn).normalize();
    if case.overboard {
        return;
    }
    if case.at.x.abs() > edge(case.at.z) {
        case.overboard = true;
        return;
    }
    let floor = look.floor(case.at.x, case.at.z);
    let low = case.at.y - case.reach();
    if low > floor {
        return;
    }
    case.at.y += floor - low;
    let into = -case.way.y;
    if into > SETTLE {
        case.way.y = into * BOUNCE.0;
        case.way.x *= BOUNCE.1;
        case.way.z *= BOUNCE.1;
        // It lands on an end or an edge and is thrown into a fresh tumble.
        case.spin = case.spin * 0.3
            + Vec3::new(case.way.z, case.spin.y.signum(), -case.way.x).normalize_or_zero()
                * into
                * 7.;
        if case.quiet == 0. {
            let sound = if case.landed {
                Sound::CaseKnock
            } else {
                Sound::CaseDrop
            };
            heard.push((sound, case.at, into + 2. * f32::from(!case.landed)));
            case.landed = true;
            case.quiet = HUSH;
        }
        return;
    }
    // It comes to rest on its side, its length the way it last pointed along the deck.
    let length = case.turn * Vec3::Y;
    let yaw = if length.xz().length_squared() > 1e-6 {
        length.z.atan2(-length.x)
    } else {
        case.spin.y
    };
    case.way.y = 0.;
    case.at.y = floor + case.radius;
    case.lying = Some((yaw, 0.));
    if !case.landed && case.quiet == 0. {
        heard.push((Sound::CaseDrop, case.at, 2.));
        case.quiet = HUSH;
    }
    case.landed = true;
}

/// A case lying on her deck. Pushed across its length it rolls, and round in a circle
/// for its taper. Pushed along it, it slides, which plating resists far more. It stops
/// at a step up, drops off a step down, and goes over the side at her deck edge. It
/// returns whether the case is gone.
fn roll_on_deck(
    case: &mut Case,
    (mut yaw, mut roll): (f32, f32),
    pull: Vec3,
    h: f32,
    look: &Look,
    edge: &impl Fn(f32) -> f32,
    heard: &mut Vec<(Sound, Vec3, f32)>,
) -> bool {
    let weight = -pull.y;
    if weight <= 0. || case.way.y > 0.3 {
        case.lying = None;
        return false;
    }
    let along = Case::along(yaw);
    let across = along.perp();
    let push = pull.xz();
    let way = case.way.xz();
    let slide = resist(way.dot(along), push.dot(along), SLIDE * weight, h);
    // Half the push across it goes into turning it.
    let rolled = resist(way.dot(across), 0.5 * push.dot(across), ROLL * weight, h);
    roll += rolled * h / case.radius;
    yaw += TAPER * rolled * h / case.length;
    let way = along * slide + across * rolled;
    let was = case.at;
    let floor = look.floor(was.x, was.z);
    case.at.x += way.x * h;
    case.at.z += way.y * h;
    case.way = Vec3::new(way.x, 0., way.y);
    if case.at.x.abs() > edge(case.at.z) {
        // Off her deck edge, under the rail.
        case.overboard = true;
        case.lying = None;
        return false;
    }
    let next = look.floor(case.at.x, case.at.z);
    if next > floor + STEP {
        // Into a deckhouse or a coaming, and back off it.
        case.at = was;
        let speed = way.length();
        case.way = -case.way * BOUNCE.0;
        if speed > HEARD && case.quiet == 0. {
            heard.push((Sound::CaseKnock, case.at, speed));
            case.quiet = HUSH;
        }
    } else if next < floor - 0.05 {
        // Off the edge of a raised deck, to fall to the one below.
        case.lying = None;
        return false;
    } else {
        case.at.y = next + case.radius;
    }
    case.lying = Some((yaw, roll));
    false
}

/// A speed `way` under a push `push`, against friction that takes up to `grip` of it,
/// after `h` seconds. At rest it stays at rest unless pushed harder than it grips.
fn resist(way: f32, push: f32, grip: f32, h: f32) -> f32 {
    if way.abs() < 1e-3 && push.abs() <= grip {
        return 0.;
    }
    let pushed = way + push * h;
    let held = grip * h;
    if pushed.abs() <= held {
        0.
    } else {
        pushed - held * pushed.signum()
    }
}

/// Cases near the deck knock against each other and the fittings standing on it, and
/// give way, so that the brass spreads out in a drift rather than through itself.
fn knock(cases: &mut [Case], posts: &[Post], heard: &mut Vec<(Sound, Vec3, f32)>) {
    for case in cases.iter_mut().filter(|c| !c.overboard) {
        let elbow = ELBOW * case.length;
        for post in posts {
            let off = case.at.xz() - post.at;
            let room = post.radius + elbow;
            let apart = off.length();
            if apart >= room || apart < 1e-4 {
                continue;
            }
            let out = off / apart;
            case.at.x += out.x * (room - apart);
            case.at.z += out.y * (room - apart);
            let into = -case.way.xz().dot(out);
            if into > 0. {
                let back = out * into * (1. + BOUNCE.0);
                case.way.x += back.x;
                case.way.z += back.y;
                if into > HEARD && case.quiet == 0. {
                    heard.push((Sound::CaseKnock, case.at, into));
                    case.quiet = HUSH;
                }
            }
        }
    }
    // Sorted along her length, so only near neighbours are weighed against each other.
    let mut order: Vec<usize> = (0..cases.len()).filter(|&k| !cases[k].overboard).collect();
    order.sort_by(|&a, &b| cases[a].at.z.total_cmp(&cases[b].at.z));
    let widest = order
        .iter()
        .map(|&k| ELBOW * cases[k].length)
        .fold(0., f32::max);
    for i in 0..order.len() {
        for j in i + 1..order.len() {
            let (a, b) = (order[i], order[j]);
            if cases[b].at.z - cases[a].at.z > 2. * widest {
                break;
            }
            let room = ELBOW * (cases[a].length + cases[b].length);
            let off = cases[b].at - cases[a].at;
            if off.y.abs() > 0.6 * room {
                continue;
            }
            let flat = off.xz();
            let apart = flat.length();
            if apart >= room || apart < 1e-4 {
                continue;
            }
            let out = flat / apart;
            let shift = 0.5 * (room - apart);
            let closing = (cases[a].way - cases[b].way).xz().dot(out);
            for (k, sign) in [(a, -1.), (b, 1.)] {
                let case = &mut cases[k];
                case.at.x += sign * out.x * shift;
                case.at.z += sign * out.y * shift;
                if closing > 0. {
                    let swap = out * (0.5 * closing * (1. + BOUNCE.0)) * sign;
                    case.way.x += swap.x;
                    case.way.z += swap.y;
                }
            }
            if closing > HEARD && cases[a].quiet == 0. && cases[b].quiet == 0. {
                heard.push((Sound::CaseKnock, cases[a].at, closing));
                cases[a].quiet = HUSH;
                cases[b].quiet = HUSH;
            }
        }
    }
}

/// Draw each case where it lies or tumbles, from a pool of models under her hull that
/// grows as the brass does. A case coming to rest eases onto its side.
fn draw(
    time: Res<Time>,
    run: Res<Campaign>,
    casting: Res<Casting>,
    ship: Single<Entity, With<Ship>>,
    mut brass: ResMut<Brass>,
    mut drawn: Query<(&mut Transform, &mut Visibility), With<Spent>>,
    mut commands: Commands,
) {
    let ease = 1. - (-time.delta_secs() / 0.05).exp();
    let brass = &mut *brass;
    if run.under_way() {
        for case in &mut brass.cases {
            if let Some((yaw, roll)) = case.lying {
                case.turn = case.turn.slerp(Case::lie(yaw, roll), ease);
            }
        }
    }
    while brass.drawn.len() < brass.cases.len() {
        let model = commands
            .spawn((
                Spent,
                Mesh3d(casting.mesh.clone()),
                MeshMaterial3d(casting.paint.clone()),
                Transform::default(),
                Visibility::Hidden,
                NotShadowCaster,
                ChildOf(*ship),
            ))
            .id();
        brass.drawn.push(model);
    }
    for (k, &model) in brass.drawn.iter().enumerate() {
        let Ok((mut pose, mut shown)) = drawn.get_mut(model) else {
            continue;
        };
        let Some(case) = brass.cases.get(k) else {
            shown.set_if_neq(Visibility::Hidden);
            continue;
        };
        shown.set_if_neq(Visibility::Inherited);
        *pose = Transform::from_translation(case.at)
            .with_rotation(case.turn)
            .with_scale(Vec3::new(2. * case.radius, case.length, 2. * case.radius));
    }
}
