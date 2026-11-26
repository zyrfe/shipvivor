//! Damage as it shows on Breakwater herself, seen from the chase camera.
//!
//! Each battered section smoulders, the worse it is the thicker: pale wisps from one
//! that is scarred, grey smoke from one holed to a third of its structure, and black
//! smoke and embers from one flooded or with a fitting knocked out. A damaged engine
//! sends dark smoke up out of the machinery space. Each puff is let go into the air
//! where it rose, so under way the smoke streams astern of her and leans away downwind.
//!
//! A spanner hangs over the section the repair party is working on, rocking as they
//! work.
use crate::{
    camera::ShipView,
    insignia::Insignia,
    scene::{self, deck_height},
    seascape::Sea,
    state::{Campaign, Encounter, PlayerHelm, Screen},
};
use bevy::prelude::*;
use shipvivor_compass::direction;
use shipvivor_encounter::Hull;
use shipvivor_shipyard::{Component as Gear, Ship, State};

/// Puffs kept in the air at once.
const POOL: usize = 256;
/// Shades of smoke from pale to black, then the glow of embers, and the steps each
/// fades through.
const SHADES: [[u8; 3]; 5] = [
    [176, 174, 168],
    [118, 116, 114],
    [66, 64, 64],
    [30, 29, 31],
    [255, 146, 54],
];
const EMBER: usize = 4;
const FADES: usize = 8;
/// How opaque the thickest smoke is.
const THICK: f32 = 0.92;
/// Under this share of its structure a section smokes grey, and under the first one
/// it smokes at all.
const SCARRED: f32 = 0.7;
const HOLED: f32 = 0.35;
/// Puffs a second, their shade and their size in metres across, for each kind of
/// damage. A section shows each that applies.
const WISPS: (f32, usize, f32) = (2.5, 0, 0.9);
const SMOKE: (f32, usize, f32) = (5., 1, 1.3);
const FLOODED: (f32, usize, f32) = (6., 2, 1.7);
const KNOCKED: (f32, usize, f32) = (4.5, 3, 1.4);
const ENGINE: (f32, usize, f32) = (3., 2, 1.2);
const EMBERS: (f32, usize, f32) = (5., EMBER, 0.22);
/// Seconds a puff lasts, the metres a second it rises at first, how quickly the air
/// takes it, and the breeze in metres a second that carries it off. Embers are gone
/// in a fraction of the time and fly up faster.
const LIFE: (f32, f32) = (2.6, 4.);
const RISE: (f32, f32) = (1.8, 3.);
const DRAG: f32 = 0.55;
const BREEZE: f32 = 2.6;
const FLEETING: f32 = 0.22;
const SPARK: f32 = 1.8;
/// Share of her way a puff keeps as it leaves her.
const CARRIED: f32 = 0.55;
/// How far each side of the centreline smoke rises from, as a share of the beam, and
/// how far inside the bulkheads.
const SPREAD: f32 = 0.32;
const INSET: f32 = 0.6;

/// The spanner badge's side, the spanner's within it, its height in metres over the
/// deck, and how far and how often it rocks.
const BADGE: f32 = 30.;
const SPANNER: f32 = 18.;
const OVER: f32 = 4.2;
const ROCK: (f32, f32) = (0.35, 5.);

fn brass() -> Color {
    Color::srgb_u8(255, 201, 101)
}
fn plate() -> Color {
    Color::srgb_u8(12, 31, 40)
}

#[derive(Clone, Copy)]
struct Puff {
    at: Vec3,
    way: Vec3,
    age: f32,
    life: f32,
    size: f32,
    shade: usize,
}

/// The puffs in the air, the smoke still owed to each section, and the battle clock at
/// the last frame.
#[derive(Resource)]
pub struct Smoulder {
    puffs: Vec<Puff>,
    owed: Vec<f32>,
    clock: f32,
    seed: u32,
    shades: Vec<Handle<StandardMaterial>>,
}
impl Smoulder {
    fn unit(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        (self.seed >> 8) as f32 / (1 << 24) as f32
    }
    fn within(&mut self, low: f32, high: f32) -> f32 {
        low + (high - low) * self.unit()
    }
    fn material(&self, shade: usize, fade: usize) -> Handle<StandardMaterial> {
        self.shades[shade * FADES + fade.min(FADES - 1)].clone()
    }
}

/// One of the meshes the puffs are drawn with.
#[derive(Component)]
pub struct PuffModel(usize);

/// The spanner over the repair party's section, and the spanner itself.
#[derive(Component)]
pub struct RepairBadge;
#[derive(Component)]
pub struct RepairSpanner;

pub fn setup(
    mut cmd: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut paint: ResMut<Assets<StandardMaterial>>,
) {
    let ball = meshes.add(Sphere::new(0.5).mesh().ico(1).unwrap());
    let shades = (SHADES.iter())
        .flat_map(|&[r, g, b]| {
            (0..FADES).map(move |fade| {
                let alpha = THICK * (fade + 1) as f32 / FADES as f32;
                let glow = if [r, g, b] == SHADES[EMBER] {
                    LinearRgba::rgb(4., 1.6, 0.4) * alpha
                } else {
                    LinearRgba::BLACK
                };
                StandardMaterial {
                    base_color: Color::srgba_u8(r, g, b, (alpha * 255.) as u8),
                    emissive: glow,
                    alpha_mode: AlphaMode::Blend,
                    unlit: true,
                    ..default()
                }
            })
        })
        .map(|material| paint.add(material))
        .collect::<Vec<_>>();
    for i in 0..POOL {
        cmd.spawn((
            PuffModel(i),
            Mesh3d(ball.clone()),
            MeshMaterial3d(shades[0].clone()),
            Transform::default(),
            Visibility::Hidden,
        ));
    }
    cmd.insert_resource(Smoulder {
        puffs: Vec::with_capacity(POOL),
        owed: Vec::new(),
        clock: 0.,
        seed: 0x5A0C_E11D,
        shades,
    });
    let spanner = cmd
        .spawn((
            RepairSpanner,
            Node {
                width: px(SPANNER),
                height: px(SPANNER),
                ..default()
            },
            ImageNode::default().with_color(brass()),
            UiTransform::default(),
        ))
        .id();
    cmd.spawn((
        RepairBadge,
        Node {
            position_type: PositionType::Absolute,
            width: px(BADGE),
            height: px(BADGE),
            border: UiRect::all(px(2)),
            border_radius: BorderRadius::MAX,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            display: Display::None,
            ..default()
        },
        BackgroundColor(plate()),
        BorderColor::all(brass()),
    ))
    .add_child(spanner);
}

/// The kinds of smoke one section gives off, as in [`WISPS`].
fn smoke(ship: &Ship, module: usize) -> Vec<(f32, usize, f32)> {
    let hold = &ship.compartments()[module];
    let mut kinds = Vec::new();
    if hold.wrecked() {
        kinds.extend([FLOODED, EMBERS]);
    } else if hold.soundness() < HOLED {
        kinds.push(SMOKE);
    } else if hold.soundness() < SCARRED {
        kinds.push(WISPS);
    }
    for (index, gear) in ship.design().modules()[module].fit.iter().enumerate() {
        let berth = shipvivor_shipyard::Berth { module, index };
        match ship.state(berth) {
            State::Out if !hold.wrecked() => kinds.extend([KNOCKED, EMBERS]),
            State::Damaged if *gear == Gear::Engine => kinds.push(ENGINE),
            _ => {}
        }
    }
    kinds
}

/// Where along her a section stands in the model's own frame, and the half-width of
/// the deck smoke rises from.
fn section(ship: &Ship, module: usize) -> ((f32, f32), f32) {
    let design = ship.design();
    let (fore, aft) = design.span(module);
    let inset = INSET.min(0.25 * (aft - fore));
    let z = (
        fore + inset - Hull::CRUISER.bow,
        aft - inset - Hull::CRUISER.bow,
    );
    (z, SPREAD * design.module_beam(module))
}

/// Let smoke go from each battered section and carry what is aloft on the air.
#[allow(clippy::too_many_arguments)]
pub fn smoulder(
    run: Res<Campaign>,
    battle: Res<Encounter>,
    helm: Res<PlayerHelm>,
    sea: Res<Sea>,
    smoulder: Option<ResMut<Smoulder>>,
    hull: Single<&Transform, (With<scene::Ship>, Without<PuffModel>)>,
    mut models: Query<(
        &PuffModel,
        &mut Transform,
        &mut Visibility,
        &mut MeshMaterial3d<StandardMaterial>,
    )>,
) {
    let Some(mut air) = smoulder else {
        return;
    };
    let clock = battle.at_sea();
    // A new voyage starts with clear air, and the clock stands still while she waits.
    if clock < air.clock || run.screen() != Screen::Sailing {
        air.puffs.clear();
    }
    let dt = (clock - air.clock).clamp(0., 0.1);
    air.clock = clock;
    let ship = &run.ship;
    let modules = ship.compartments().len();
    air.owed.resize(modules, 0.);
    if run.screen() == Screen::Sailing && ship.afloat() {
        let way = (helm.forward() * helm.speed * CARRIED).extend(0.).xzy();
        for module in 0..modules {
            let ((fore, aft), half) = section(ship, module);
            for (rate, shade, size) in smoke(ship, module) {
                air.owed[module] += rate * dt;
                while air.owed[module] >= 1. {
                    air.owed[module] -= 1.;
                    if air.puffs.len() >= POOL {
                        continue;
                    }
                    let z = air.within(fore, aft);
                    let x = air.within(-half, half);
                    let at = hull.transform_point(Vec3::new(x, deck_height(z) + 0.2, z));
                    let (fleeting, spark) = if shade == EMBER {
                        (FLEETING, SPARK)
                    } else {
                        (1., 1.)
                    };
                    let rise = spark * air.within(RISE.0, RISE.1);
                    let puff = Puff {
                        at,
                        way: way + Vec3::Y * rise,
                        age: 0.,
                        life: fleeting * air.within(LIFE.0, LIFE.1),
                        size: size * air.within(0.8, 1.2),
                        shade,
                    };
                    air.puffs.push(puff);
                }
            }
        }
    }
    let breeze = (direction(sea.weather().wind) * BREEZE).extend(0.).xzy();
    let calm = 1. - (-DRAG * dt).exp();
    air.puffs.retain_mut(|puff| {
        puff.age += dt;
        // The air takes the puff: it slows to the breeze and stops rising.
        puff.way += (breeze - puff.way) * calm;
        puff.at += puff.way * dt;
        puff.age < puff.life
    });
    for (model, mut pose, mut visible, mut material) in &mut models {
        let Some(puff) = air.puffs.get(model.0).copied() else {
            visible.set_if_neq(Visibility::Hidden);
            continue;
        };
        let t = puff.age / puff.life;
        // Smoke billows out as it rises and thins away. An ember shrinks as it cools.
        let thick = (t / 0.06).min(1.) * (1. - t).powf(0.7);
        let fade = (thick * FADES as f32).ceil() as usize;
        if fade == 0 {
            visible.set_if_neq(Visibility::Hidden);
            continue;
        }
        let swell = if puff.shade == EMBER {
            puff.size * (1. - 0.7 * t)
        } else {
            puff.size * (0.4 + 1.8 * t.powf(0.6))
        };
        *pose = Transform::from_translation(puff.at).with_scale(Vec3::new(1., 0.85, 1.) * swell);
        let wanted = air.material(puff.shade, fade - 1);
        if material.0 != wanted {
            material.0 = wanted;
        }
        visible.set_if_neq(Visibility::Inherited);
    }
}

/// Hang the spanner over the section the repair party is working on.
#[allow(clippy::too_many_arguments)]
pub fn mark_repairs(
    run: Res<Campaign>,
    battle: Res<Encounter>,
    helm: Res<PlayerHelm>,
    view: Res<ShipView>,
    viewport: (Single<&Window>, Res<UiScale>),
    stamps: Option<Res<Insignia>>,
    hull: Single<&Transform, With<scene::Ship>>,
    mut badge: Single<&mut Node, With<RepairBadge>>,
    mut spanner: Single<(&mut ImageNode, &mut UiTransform), With<RepairSpanner>>,
) {
    let ship = &run.ship;
    let module = match ship.party() {
        Some(shipvivor_shipyard::Task::Module(module)) => Some(module),
        Some(shipvivor_shipyard::Task::Fitting(berth)) => Some(berth.module),
        None => None,
    };
    let (window, scale) = viewport;
    let size = window.size() / scale.0;
    let at = module
        .filter(|_| run.screen() == Screen::Sailing && ship.afloat())
        .and_then(|module| {
            let ((fore, aft), _) = section(ship, module);
            let z = 0.5 * (fore + aft);
            let over = hull.transform_point(Vec3::new(0., deck_height(z) + OVER, z));
            view.viewport_point(helm.position, over, size)
        })
        .filter(|p| p.x > -BADGE && p.x < size.x + BADGE && p.y > -BADGE && p.y < size.y);
    let display = if at.is_some() {
        Display::Flex
    } else {
        Display::None
    };
    if badge.display != display {
        badge.display = display;
    }
    let Some(at) = at else {
        return;
    };
    badge.left = px(at.x - 0.5 * BADGE);
    badge.top = px(at.y - 0.5 * BADGE);
    let (image, turn) = &mut *spanner;
    if let Some(stamps) = stamps
        && image.image != stamps.order
    {
        image.image = stamps.order.clone();
    }
    // The party at work: the spanner rocks back and forth on its nut.
    turn.rotation = Rot2::radians(ROCK.0 * (battle.at_sea() * ROCK.1).sin());
}
