//! Shipyard cranes, production buildings and the wreckage left on their foundations.
#[path = "harbour_halls.rs"]
mod halls;
#[path = "harbour_surfaces.rs"]
mod surfaces;
use crate::scene::Facets;
use bevy::prelude::*;
use shipvivor_encounter::YardState;
pub(super) use surfaces::material;

const CONCRETE: Color = Color::srgb(0.64, 0.63, 0.52);
const STEEL: Color = Color::srgb(0.36, 0.46, 0.48);
const ROOF: Color = Color::srgb(0.48, 0.57, 0.55);
const IRON: Color = Color::srgb(0.15, 0.20, 0.22);
const BRICK: Color = Color::srgb(0.54, 0.33, 0.24);
const CORAL: Color = Color::srgb(0.86, 0.25, 0.14);
const OCHRE: Color = Color::srgb(0.98, 0.67, 0.21);
const STRIPE: Color = Color::srgb(0.91, 0.86, 0.68);
const OLD_BLUE: Color = Color::srgb(0.38, 0.56, 0.63);
const GLAZING: Color = Color::srgb(0.46, 0.58, 0.59);
const WINDOW: Color = Color::srgb(0.13, 0.25, 0.28);
const CHAR: Color = Color::srgb(0.19, 0.21, 0.21);
const TIMBER: Color = Color::srgb(0.45, 0.32, 0.21);
const TIN: Color = Color::srgb(0.40, 0.51, 0.54);
const MARINE_BLUE: Color = Color::srgb(0.23, 0.44, 0.52);
const OXIDE: Color = Color::srgb(0.58, 0.26, 0.18);
const FURNACE: Color = Color::srgb(1., 0.70, 0.29);

#[derive(Clone, Copy, Hash, Eq, PartialEq)]
pub(super) enum Harbour {
    Cannery,
    TwinMills,
    Foundry,
}

impl Harbour {
    pub const ALL: [Self; 3] = [Self::Cannery, Self::TwinMills, Self::Foundry];

    pub fn for_factories(count: usize) -> Self {
        match count {
            1 => Self::Cannery,
            2 => Self::TwinMills,
            _ => Self::Foundry,
        }
    }

    pub fn chimney(self) -> Vec3 {
        let (x, z) = match self {
            Self::Cannery => (2.6, 1.9),
            Self::TwinMills => (2.5, 0.8),
            Self::Foundry => (-2.5, 2.1),
        };
        Vec3::new(x, 9., z)
    }
}

pub(super) fn flag_hoist(state: YardState) -> Vec3 {
    let height = match state {
        YardState::Working => 16.8,
        YardState::Damaged => 11.8,
        YardState::Burning => 5.8,
        YardState::Ruined => 0.,
    };
    Vec3::new(-5.6, height, 3.8)
}

#[derive(Default)]
struct Works(Facets);

impl Works {
    fn quad(&mut self, corners: [Vec3; 4], color: Color) {
        self.0.quad(corners, color);
    }

    fn block(&mut self, center: Vec3, size: Vec3, rotation: Quat, color: Color) {
        let vertices = [
            (-1., -1., -1.),
            (1., -1., -1.),
            (1., 1., -1.),
            (-1., 1., -1.),
            (-1., -1., 1.),
            (1., -1., 1.),
            (1., 1., 1.),
            (-1., 1., 1.),
        ]
        .map(|p| center + rotation * (Vec3::from(p) * size * 0.5));
        for face in [
            [0, 3, 2, 1],
            [1, 2, 6, 5],
            [4, 5, 6, 7],
            [0, 4, 7, 3],
            [3, 7, 6, 2],
            [0, 1, 5, 4],
        ] {
            self.quad(face.map(|i| vertices[i]), color);
        }
    }

    fn slab(&mut self, center: Vec3, size: Vec3, color: Color) {
        self.block(center, size, Quat::IDENTITY, color);
    }

    fn girder(&mut self, from: Vec3, to: Vec3, width: f32, color: Color) {
        let axis = to - from;
        self.block(
            (from + to) * 0.5,
            Vec3::new(width, axis.length(), width),
            Quat::from_rotation_arc(Vec3::Y, axis.normalize()),
            color,
        );
    }

    fn stack(&mut self, foot: Vec3, h: f32, base_radius: f32, top_radius: f32, color: Color) {
        let top = foot + Vec3::Y * h;
        for i in 0..8 {
            let radial = |n: u8| {
                let angle = f32::from(n) * std::f32::consts::FRAC_PI_4;
                Vec3::new(angle.cos(), 0., angle.sin())
            };
            let (a, b) = (radial(i), radial(i + 1));
            let (c, d) = (top + a * top_radius, top + b * top_radius);
            self.quad(
                [foot + a * base_radius, c, d, foot + b * base_radius],
                color,
            );
            self.0.triangle([top, d, c], IRON);
        }
    }

    fn tube(&mut self, from: Vec3, to: Vec3, radius: f32, end_radius: f32, color: Color) {
        self.0.tube(from, to, radius, end_radius, color);
    }

    fn rails(&mut self, a: Vec3, b: Vec3, spans: u8) {
        for h in [0.52, 1.03] {
            self.girder(a + Vec3::Y * h, b + Vec3::Y * h, 0.095, STRIPE);
        }
        for i in 0..=spans {
            let foot = a.lerp(b, f32::from(i) / f32::from(spans));
            self.girder(foot, foot + Vec3::Y * 1.09, 0.13, STEEL);
        }
    }
    fn pile_deck(&mut self) {
        let v = Vec3::new;
        self.slab(v(0., 0.13, 0.), v(8., 0.36, 8.), TIMBER);
        for x in [-3.55, 3.55] {
            self.slab(v(x, -0.2, 0.), v(0.48, 0.4, 8.), IRON);
            for z in [-3.55, 0., 3.55] {
                self.slab(v(x, -2., z), v(0.52, 4., 0.52), TIMBER);
            }
            self.girder(v(x, -2.4, -3.55), v(x, -0.24, 0.), 0.26, TIMBER);
            self.girder(v(x, -0.24, 0.), v(x, -2.4, 3.55), 0.26, TIMBER);
        }
    }

    /// Narrow-gauge supply rails follow the bank, with sleepers and trestle feet above
    /// the low foreshore. The supplied points run in the island frame.
    fn supply_track(&mut self, path: &[Vec2]) {
        let at = |p: Vec2, y: f32| Vec3::new(p.x, y, p.y);
        let normal = |i: usize| {
            (path[(i + 1).min(path.len() - 1)] - path[i.saturating_sub(1)])
                .normalize()
                .perp()
        };
        for (i, pair) in path.windows(2).enumerate() {
            let [a, b] = [pair[0], pair[1]];
            let n0 = normal(i);
            let n1 = normal(i + 1);
            for offset in [-0.53, 0.53] {
                self.girder(
                    at(a + n0 * offset, 3.64),
                    at(b + n1 * offset, 3.64),
                    0.145,
                    STEEL,
                );
                self.girder(
                    at(a + n0 * offset, 3.37),
                    at(b + n1 * offset, 3.37),
                    0.24,
                    TIMBER,
                );
            }
            let sleepers = (a.distance(b) / 1.03).ceil() as usize;
            for j in 0..sleepers {
                let t = (j as f32 + 0.5) / sleepers as f32;
                let p = a.lerp(b, t);
                let n = n0.lerp(n1, t) * 0.87;
                self.girder(at(p - n, 3.51), at(p + n, 3.51), 0.19, TIMBER);
                if p.length_squared() > 17_f32.powi(2) && j % 3 == 0 {
                    for side in [-1., 1.] {
                        self.girder(
                            at(p + n * side, -0.2),
                            at(p + n * side * 0.8, 3.45),
                            0.31,
                            TIMBER,
                        );
                    }
                }
            }
        }
    }

    fn plate_trolley(&mut self, start: Vec2, end: Vec2) {
        let d = (end - start).normalize();
        let side = d.perp();
        let mid = start.midpoint(end);
        let position = |width: f32, along: f32, height: f32| {
            (mid + side * width + d * along).extend(height).xzy()
        };
        let turn = Quat::from_rotation_y(d.x.atan2(d.y));
        for z in [-0.92, 0.92] {
            for x in [-0.53, 0.53] {
                self.tube(
                    position(x - 0.12, z, 3.89),
                    position(x + 0.12, z, 3.89),
                    0.26,
                    0.26,
                    IRON,
                );
            }
        }
        self.block(
            position(0., 0., 4.08),
            Vec3::new(1.81, 0.30, 2.65),
            turn,
            OXIDE,
        );
        for i in 0..3 {
            self.block(
                position(0., 0.04 * i as f32, 4.31 + i as f32 * 0.14),
                Vec3::new(1.57, 0.13, 2.39),
                turn * Quat::from_rotation_y(0.05 * i as f32),
                if i % 2 == 0 { ROOF } else { STEEL },
            );
        }
    }

    // Beams lie across the old track and each end of the building keeps a broken footing.
    fn rubble(&mut self, half_width: f32, half_depth: f32) {
        let v = Vec3::new;
        self.slab(
            v(0., 0.12, 0.),
            v(half_width * 2., 0.24, half_depth * 2.),
            CHAR,
        );
        for (i, (x, z)) in [
            (-0.79, -0.64),
            (0.57, -0.75),
            (-0.87, 0.39),
            (0.74, 0.55),
            (0.24, 0.68),
        ]
        .into_iter()
        .enumerate()
        {
            let angle = (i as f32 * 1.8).sin() * 0.45;
            self.block(
                v(x * half_width, 0.4, z * half_depth),
                v(half_width * 0.40, 0.72, half_depth * 0.40),
                Quat::from_rotation_y(angle) * Quat::from_rotation_z(angle * 0.27),
                CONCRETE,
            );
        }
        self.girder(
            v(-half_width * 0.8, 0.43, -half_depth * 0.34),
            v(half_width * 0.31, 0.95, half_depth * 0.72),
            0.67,
            IRON,
        );
        self.girder(
            v(half_width * 0.8, 0.43, -half_depth * 0.5),
            v(-half_width * 0.18, 1.02, half_depth * 0.62),
            0.48,
            STEEL,
        );
    }
}

/// The heavy yard's open water cut sits exactly between these two retaining walls. The
/// quay stones finish below low water so the berth still has walls in a trough.
fn wet_berth(m: &mut Works) {
    let v = Vec3::new;
    let concrete = Color::srgb(0.57, 0.58, 0.53);
    let wet = Color::srgb(0.33, 0.39, 0.39);
    for side in [-1., 1.] {
        let inside = 6. + side * 3.30;
        for (z0, z1, top, width) in [
            (-12., 1.47, 3.81, 2.83),
            (-23., -12., 2.38, 2.53),
            (-30., -23., 2.04, 1.50),
            (-34., -30., 1.62, 1.05),
        ] {
            let x = inside + side * width * 0.5;
            let length = z1 - z0;
            let z = (z0 + z1) * 0.5;
            m.slab(
                v(x, (top - 2.5) * 0.5, z),
                v(width, top + 2.5, length + 0.05),
                concrete,
            );
            m.slab(
                v(inside - 0.015 * side, -0.25, z),
                v(0.10, 1.35, length),
                wet,
            );
            m.slab(v(x, top, z), v(width + 0.26, 0.24, length + 0.08), ROOF);
            let planks = (length / 1.4).ceil() as usize;
            for i in 0..planks {
                let pos = z0 + (i as f32 + 0.5) * length / planks as f32;
                m.slab(
                    v(inside - 0.065 * side, top * 0.46, pos),
                    v(0.16, top, 0.16),
                    STEEL,
                );
            }
        }
        for z in [-31.7, -27., -21.7, -16.3, -10.9, -5.6, -0.9] {
            let high = if z < -30. {
                1.76
            } else if z < -23. {
                2.18
            } else if z < -12. {
                2.52
            } else {
                3.91
            };
            m.slab(
                v(inside - 0.17 * side, high * 0.5 - 0.22, z),
                v(0.39, high + 0.52, 0.49),
                TIMBER,
            );
            m.slab(
                v(inside - 0.19 * side, high - 0.55, z),
                v(0.44, 0.17, 0.51),
                IRON,
            );
            let bollard = v(inside + 0.47 * side, high + 0.02, z);
            m.stack(bollard, 0.46, 0.225, 0.185, IRON);
            m.slab(bollard + Vec3::Y * 0.46, v(0.65, 0.14, 0.38), STEEL);
        }
        m.rails(
            v(inside + 2.69 * side, 3.94, -11.4),
            v(inside + 2.69 * side, 3.94, 0.9),
            5,
        );
        for step in 0..5 {
            let top = 3.66 - 0.27 * step as f32;
            m.slab(
                v(
                    inside + side * 1.57,
                    top - 0.16,
                    -12.15 - step as f32 * 0.36,
                ),
                v(1.62, 0.34, 0.47),
                CONCRETE,
            );
        }
        // Guide piles are strapped to the quay heads; leave the open-water entrance clear.
        let pile = v(inside + side * 0.32, -2.3, -34.2);
        m.stack(pile, 4.56, 0.38, 0.32, TIMBER);
        m.stack(pile + Vec3::Y * 4.18, 0.27, 0.37, 0.35, STRIPE);
    }
    m.slab(v(6., 1.02, 1.72), v(6.60, 4.86, 0.55), concrete);
    m.slab(v(6., 3.59, 1.75), v(6.86, 0.44, 0.83), ROOF);
}

/// Launch rails cross the shelf and slope into the sea. The posts stay after the yard is lost.
pub(super) fn slipway(harbour: Harbour) -> Mesh {
    let mut m = Works::default();
    let v = Vec3::new;
    if harbour == Harbour::Foundry {
        wet_berth(&mut m);
    } else {
        let timbers = [
            Color::srgb(0.51, 0.43, 0.32),
            Color::srgb(0.62, 0.53, 0.40),
            TIMBER,
            Color::srgb(0.55, 0.48, 0.37),
        ];
        for (az, ah, bz, bh, count) in [
            (-8.9, 3.55, -18., 3.55, 12),
            (-18., 3.55, -30., 2.06, 16),
            (-30., 2.06, -45., 0.80, 19),
        ] {
            let a = v(6., ah, az);
            let b = v(6., bh, bz);
            let lane = b - a;
            let rotation = Quat::from_rotation_arc(Vec3::NEG_Z, lane.normalize());
            for k in 0..count {
                let t = (k as f32 + 0.5) / count as f32;
                m.block(
                    a + lane * t,
                    v(6.27, 0.22, lane.length() / count as f32 - 0.04),
                    rotation,
                    timbers[(k * 5 + k / 3) % 4],
                );
            }
            for side in [-1., 1.] {
                let offset = v(side * 2.4, 0.20, 0.);
                m.girder(a + offset, b + offset, 0.25, STEEL);
                m.girder(
                    a + v(side * 2.69, -0.43, 0.),
                    b + v(side * 2.69, -0.43, 0.),
                    0.49,
                    TIMBER,
                );
            }
        }
        // Sedged trestles carry the sloping ways. Mooring stanchions and the side walk stay out
        // of the keel track so they also make sense when the yard has launched its hull.
        for (z, deck) in [
            (-21., 3.17),
            (-27., 2.44),
            (-33., 1.82),
            (-39., 1.31),
            (-44., 0.91),
        ] {
            for side in [-1., 1.] {
                let x = 6. + side * 2.65;
                m.girder(
                    v(x + 0.23 * side, -1.67, z),
                    v(x, deck + 0.52, z),
                    0.54,
                    TIMBER,
                );
                m.slab(v(x, deck + 0.28, z), v(0.61, 0.14, 0.61), IRON);
            }
            m.girder(v(3.43, 0.0, z), v(8.57, deck - 0.40, z), 0.29, TIMBER);
        }
    }
    // The level work aprons sit on the island's shelf, clear of the fairway.
    for side in [-1., 1.] {
        let center_x = 6. + side * 4.76;
        if harbour == Harbour::Foundry {
            m.slab(v(center_x, 3.92, -6.9), v(0.24, 0.18, 9.6), STEEL);
            m.slab(v(center_x, 3.96, -11.4), v(2.35, 0.08, 0.26), STRIPE);
            let sheet = v(center_x, 4.04, -9.66);
            m.block(
                sheet,
                v(1.86, 0.22, 3.15),
                Quat::from_rotation_y(side * 0.09),
                IRON,
            );
            m.block(
                sheet + v(0.09, 0.17, -0.08),
                v(1.66, 0.17, 2.94),
                Quat::from_rotation_y(-side * 0.08),
                ROOF,
            );
            continue;
        }
        let deck = if harbour == Harbour::Cannery {
            TIMBER
        } else {
            STEEL
        };
        let end = -18.6;
        m.slab(
            v(center_x, 3.5, (-8.8 + end) * 0.5),
            v(3.25, 0.73, -8.8 - end),
            deck,
        );
        for z in [-10.4, -14.3, end + 0.85] {
            let at = v(center_x + side * 1.27, 3.86, z);
            m.stack(at, 0.52, 0.19, 0.15, IRON);
            m.slab(at + Vec3::Y * 0.52, v(0.64, 0.18, 0.30), IRON);
            m.slab(v(at.x, 2., z), v(0.50, 3.7, 0.57), TIMBER);
        }
        if side < 0. {
            m.rails(
                v(center_x + side * 1.45, 3.88, -9.25),
                v(center_x + side * 1.45, 3.88, -14.),
                2,
            );
        }
    }
    if harbour == Harbour::Foundry {
        let east = [
            (6., 17.),
            (10.5, 16.9),
            (14.4, 14.8),
            (17.0, 11.5),
            (17.25, 7.3),
            (16.9, 2.7),
            (15.4, -1.),
            (12.2, -4.),
            (11.0, -7.2),
            (11., -10.1),
        ]
        .map(Vec2::from);
        m.supply_track(&east);
        m.plate_trolley(east[1], east[2]);
        let west = [
            (-17., -7.9),
            (-13.1, -8.),
            (-8.5, -8.2),
            (-4.8, -9.1),
            (-1.1, -10.4),
            (1.2, -11.8),
        ]
        .map(Vec2::from);
        m.supply_track(&west);
        m.plate_trolley(west[1], west[2]);
    }
    m.finish()
}

/// Each dock shares the target envelope and keeps an open lane for the hull on the ways.
pub(super) fn dock(harbour: Harbour, state: YardState) -> Mesh {
    let mut m = Works::default();
    let v = Vec3::new;
    if state == YardState::Ruined {
        if harbour == Harbour::Foundry {
            for side in [-1., 1.] {
                let x = 5.06 * side;
                for (z, angle) in [(-3.4, -0.29), (0.1, 0.36), (2.9, -0.52)] {
                    m.block(
                        v(x, 0.44, z),
                        v(2.45, 0.87, 1.74),
                        Quat::from_rotation_y(angle),
                        CHAR,
                    );
                    m.block(
                        v(x - 0.23, 0.96, z + 0.32),
                        v(1.12, 0.50, 2.08),
                        Quat::from_rotation_y(-angle),
                        STEEL,
                    );
                }
            }
            m.girder(v(-5.9, 0.46, -3.9), v(-1.52, -2.4, -4.32), 0.64, OXIDE);
        } else {
            m.rubble(7., 5.5);
        }
        return m.finish();
    }
    let hoist = flag_hoist(state);
    let top = hoist.y;
    m.girder(v(hoist.x, 0., hoist.z), hoist, 0.24, STRIPE);
    if harbour == Harbour::Foundry {
        for side in [-1., 1.] {
            m.slab(v(side * 5.33, 0.45, 0.), v(3.25, 0.60, 10.92), CONCRETE);
            m.slab(v(side * 5.33, 0.78, 0.), v(2.88, 0.14, 10.95), IRON);
        }
    } else {
        m.slab(v(0., 0.18, 0.), v(14., 0.36, 11.), CONCRETE);
        m.slab(v(0., 0.4, 0.), v(7.9, 0.1, 10.9), IRON);
        for x in [-2.4, 2.4] {
            m.slab(v(x, 0.57, 0.), v(0.24, 0.31, 10.8), STRIPE);
        }
        let working = state == YardState::Working;
        let beam = match (harbour, state) {
            (_, YardState::Burning) => CHAR,
            (Harbour::Cannery, _) => TIMBER,
            _ => MARINE_BLUE,
        };
        let crown = v(-3.25, top - 1.3, 1.);
        // A leaning timber derrick above the cannery slip.
        // At the twin mills a steel hammerhead overhangs the slip on just one side.
        for z in [-3.8, 4.2] {
            m.girder(v(-5.9, 0.4, z), crown, 0.9, beam);
            m.girder(v(-2.85, 0.4, z), crown, 0.55, beam);
        }
        let tip = if working {
            v(5.9, top - 2.2, -3.4)
        } else {
            v(2.4, top * 0.53, -2.8)
        };
        if harbour == Harbour::Cannery {
            m.girder(v(-4.6, 2.7, 1.), tip, 0.86, beam);
            m.girder(crown, tip, 0.22, IRON);
            m.slab(v(-4.5, 1.15, 0.5), v(3.7, 1.7, 7.8), OXIDE);
            m.block(
                v(-4.5, 2.25, 0.5),
                v(4.1, 0.32, 8.1),
                Quat::from_rotation_z(0.13),
                TIN,
            );
        } else {
            let heel = crown + v(-2.7, 0., 1.45);
            m.girder(heel, tip, 1.05, beam);
            m.girder(crown - Vec3::Y * 3.5, tip, 0.47, beam);
            m.slab(heel - Vec3::Y * 0.78, v(1.85, 1.75, 2.28), CONCRETE);
            m.slab(crown - Vec3::Y * 1.7, v(2.4, 1.7, 2.48), MARINE_BLUE);
            m.slab(crown + v(0., -1.48, -1.26), v(1.85, 0.62, 0.12), WINDOW);
        }
        if working {
            let hook = tip - Vec3::Y * 4.6;
            for side in [-1., 1.] {
                m.girder(
                    tip + Vec3::Z * side * 0.34,
                    hook + Vec3::Z * side * 0.27,
                    0.12,
                    IRON,
                );
                m.tube(
                    tip + Vec3::Z * side * 0.18,
                    tip + Vec3::Z * side * 0.47,
                    0.47,
                    0.47,
                    STEEL,
                );
                m.girder(
                    hook,
                    v(hook.x - 1.61, hook.y - 1.55, hook.z + side * 0.66),
                    0.10,
                    IRON,
                );
                m.girder(
                    hook,
                    v(hook.x + 1.61, hook.y - 1.55, hook.z + side * 0.66),
                    0.10,
                    IRON,
                );
            }
            m.slab(hook, v(0.92, 0.56, 0.74), OCHRE);
            if harbour == Harbour::Cannery {
                // A netted bundle of baulks hangs from the old boat derrick.
                for z in [-0.53, 0., 0.53] {
                    m.tube(
                        hook + v(-1.68, -1.54, z),
                        hook + v(1.68, -1.54, z),
                        0.28,
                        0.31,
                        TIMBER,
                    );
                }
                let winch = v(-4.3, 3.03, 0.1);
                m.tube(
                    winch - Vec3::Z * 1.1,
                    winch + Vec3::Z * 1.1,
                    0.63,
                    0.63,
                    IRON,
                );
                for end in [-1., 1.] {
                    m.tube(
                        winch + Vec3::Z * end,
                        winch + Vec3::Z * end * 1.15,
                        0.85,
                        0.85,
                        STEEL,
                    );
                    let foot = v(-5.9, 0.4, end * 3.9);
                    for t in [0.32, 0.65, 0.88] {
                        m.slab(foot.lerp(crown, t), v(1.09, 0.27, 1.12), IRON);
                    }
                    m.girder(v(-6.13, 0.5, end * 4.9), crown, 0.11, STEEL);
                }
            } else {
                m.slab(hook - v(0., 1.62, 0.), v(3.42, 0.28, 1.54), TIN);
                // The long head has four chords and Warren webs; daylight shows through it.
                let heel = crown + v(-2.7, 0., 1.45);
                let axis = tip - heel;
                let side = Vec3::Y.cross(axis).normalize() * 0.49;
                for offset in [-side, side] {
                    let upper = heel + offset;
                    let lower = upper - Vec3::Y * 1.67;
                    m.girder(lower, lower + axis, 0.24, MARINE_BLUE);
                    for i in 0..7 {
                        let (a, b) = (i as f32 / 7., (i + 1) as f32 / 7.);
                        let (a_y, b_y) = if i % 2 == 0 {
                            (lower, upper)
                        } else {
                            (upper, lower)
                        };
                        m.girder(a_y + axis * a, b_y + axis * b, 0.14, STEEL);
                    }
                }
                m.slab(v(-4.41, top - 3.52, 1.55), v(2.78, 0.18, 3.15), STEEL);
                m.rails(v(-5.64, top - 3.43, 0.1), v(-5.64, top - 3.43, 3.0), 2);
            }
        }
        return m.finish();
    }
    // A pair of solid machine houses keeps the slip open down the middle.
    for side in [-1., 1.] {
        let x = side * 5.65;
        let h = if state == YardState::Burning && side < 0. {
            1.2
        } else {
            2.7
        };
        m.slab(
            v(x, h * 0.5, 0.),
            v(2.35, h, 10.1),
            if state == YardState::Burning {
                CHAR
            } else {
                BRICK
            },
        );
        m.slab(v(x, h, 0.), v(2.55, 0.25, 10.3), IRON);
        for z in [-3.8, 0., 3.8] {
            m.slab(
                v(x + side * 1.185, h * 0.58, z),
                v(0.1, h * 0.44, 1.65),
                WINDOW,
            );
        }
        for z in [-3.8, 3.8] {
            let broken = state != YardState::Working && side > 0. && z < 0.;
            let height = if broken { top * 0.40 } else { top - 1.38 };
            let crown = v(x * 0.81, height, z);
            m.girder(
                v(x, 0.45, z - 0.94),
                crown,
                0.89,
                if broken { IRON } else { OCHRE },
            );
            m.girder(v(x, 0.45, z + 0.94), crown, 0.56, STEEL);
            m.slab(
                v(x * 0.96, 3.15, z),
                v(1.14, 0.72, 2.2),
                if broken { CHAR } else { CORAL },
            );
        }
        if state != YardState::Burning {
            let high = top - 1.08;
            let low = if state == YardState::Damaged && side > 0. {
                high * 0.58
            } else {
                high
            };
            m.girder(v(x * 0.81, low, -4.1), v(x * 0.81, high, 4.25), 0.86, OCHRE);
        }
    }
    if state == YardState::Working {
        // Trussed side rails, the walk above them and the end stops on the trolley track.
        for side in [-1., 1.] {
            let x = side * 4.57;
            m.slab(v(x, top - 3.06, 0.), v(0.43, 0.46, 8.42), STEEL);
            for (k, z) in [-3.82, -1.92, -0.02, 1.88].into_iter().enumerate() {
                let (a, b) = if k % 2 == 0 {
                    (top - 2.90, top - 1.35)
                } else {
                    (top - 1.35, top - 2.90)
                };
                m.girder(v(x, a, z), v(x, b, z + 1.90), 0.35, OCHRE);
            }
            m.slab(v(side * 5.19, top - 0.58, 0.), v(0.83, 0.23, 8.4), STEEL);
        }
        for z in [-3.8, 3.8] {
            m.slab(v(0., top - 1.2, z), v(11.6, 1.45, 1.32), OCHRE);
            m.slab(v(0., top - 1.8, z), v(11.7, 0.28, 1.36), IRON);
            for x in [-4.8, 4.8] {
                m.slab(v(x, top - 1.2, z), v(0.72, 1.46, 1.34), STRIPE);
            }
        }
        m.slab(v(-3.6, top - 0.45, -1.8), v(2.2, 0.90, 2.7), CORAL);
        m.slab(v(-3.6, top - 0.52, -3.17), v(1.56, 0.51, 0.1), WINDOW);
        for x in [-2.35, 2.35] {
            let axle = v(x, 14.46, -3.8);
            m.slab(v(x, top - 2.06, -3.8), v(1.47, 1.11, 1.61), IRON);
            m.tube(
                axle - Vec3::X * 0.38,
                axle + Vec3::X * 0.38,
                0.59,
                0.59,
                STEEL,
            );
            m.slab(axle + v(0., 0., -0.48), v(1.16, 0.70, 0.24), OCHRE);
        }
        for side in [-1., 1.] {
            let z = side * 4.2;
            for t in [0.22, 0.46, 0.70] {
                let x = 5.65_f32.lerp(4.58, t);
                let y = 0.45_f32.lerp(top - 1.38, t);
                m.girder(v(-x, y, z), v(-x, y + 0.23, z - side * 1.96), 0.23, STEEL);
            }
            let x = side * 5.13;
            m.slab(v(x, top - 2.96, 0.), v(0.95, 0.17, 8.75), IRON);
            m.rails(
                v(x + side * 0.43, top - 2.87, -4.24),
                v(x + side * 0.43, top - 2.87, 4.24),
                5,
            );
        }

        // Enclosed travelling machinery and the row of lit cab windows at the crown.
        m.slab(v(1.7, top - 1.05, 3.8), v(5.8, 2., 2.15), OCHRE);
        for x in [-0.3, 1., 2.3, 3.6] {
            m.slab(v(x, top - 0.87, 2.68), v(0.96, 0.67, 0.12), WINDOW);
        }
    } else {
        let h = top - 0.45;
        m.girder(
            v(-5., h, 3.8),
            v(5., h - 0.75, 3.8),
            0.86,
            if state == YardState::Burning {
                CHAR
            } else {
                OCHRE
            },
        );
        m.girder(v(-4.6, h - 0.5, -3.8), v(1.8, 1.4, -2.4), 0.67, STEEL);
    }
    m.finish()
}

/// Both falls pay out from the same winch. Build the cables at their rest length, then
/// normalize Y so hoisting changes the wire length without stretching the load beneath it.
pub(super) fn plate_rigging() -> Mesh {
    let mut m = Works::default();
    for side in [-1., 1.] {
        for wire in [-0.19, 0.19] {
            let top = Vec3::new(side * 2.35 + wire, 0., 0.);
            let end = Vec3::new(side * 2.35 + wire * 0.42, -7., 0.);
            m.girder(top, end, 0.13, IRON);
        }
    }
    let mut mesh = m.finish();
    mesh.scale_by(Vec3::new(1., 1. / 7., 1.));
    mesh
}

/// A hanging repair strake, cambered around the hull, held by two short triangular slings.
pub(super) fn suspended_plate() -> Mesh {
    let mut m = Works::default();
    let v = Vec3::new;
    for side in [-1., 1.] {
        let hook = v(side * 2.35, 0., 0.);
        m.slab(hook, v(0.68, 0.61, 0.46), OCHRE);
        for offset in [-0.76, 0.76] {
            let clamp = v(side * 2.35 + offset, -1.15, 0.);
            m.girder(hook, clamp, 0.13, IRON);
            m.slab(clamp, v(0.34, 0.42, 0.42), OCHRE);
        }
    }
    for i in 0..3 {
        let x = -2.56 + i as f32 * 2.56;
        let slope = -0.10;
        let y = -2.38 + slope * x;
        let z = 0.24 - (x.abs() / 2.56) * 0.16;
        let turn = Quat::from_rotation_z(slope)
            * Quat::from_rotation_x(0.24)
            * Quat::from_rotation_y(0.12 * (i as f32 - 1.));
        m.block(
            v(x, y, z),
            v(2.64, 2.53, 0.23),
            turn,
            if i == 1 {
                Color::srgb(0.40, 0.60, 0.64)
            } else {
                Color::srgb(0.49, 0.64, 0.65)
            },
        );
        let stripe = v(x, y, z) + turn * v(0., -0.96, -0.14);
        m.block(stripe, v(2.64, 0.51, 0.095), turn, OXIDE);
        for k in 0..4 {
            m.block(
                v(x, y, z) + turn * v(-0.93 + k as f32 * 0.60, 0.94, -0.15),
                v(0.14, 0.14, 0.075),
                turn,
                IRON,
            );
        }
    }
    m.finish()
}

/// A floating service hull on the heavy ways. The deck acquires frames and plate as the
/// build order advances. It lies below the crane rails with only the pilothouse above them.
pub(super) fn lighter(progress: u8) -> Mesh {
    let mut m = Works::default();
    let v = Vec3::new;
    let hull = Color::srgb(0.23, 0.41, 0.44);
    let rust = Color::srgb(0.65, 0.32, 0.22);
    let stations = [
        (-10.15, 0.09, 3.98),
        (-8.3, 1.48, 3.74),
        (-6.25, 2.38, 3.29),
        (-4.3, 2.68, 2.74),
        (-2.4, 2.78, 2.28),
        (3.7, 2.78, 2.24),
        (5.4, 2.45, 2.35),
        (6.4, 1.79, 2.49),
    ];
    for pair in stations.windows(2) {
        let [(z1, w1, h1), (z2, w2, h2)] = [pair[0], pair[1]];
        for side in [-1., 1.] {
            let section = |z: f32, w: f32, h: f32| {
                [
                    v(w * side * 0.68, -1.05, z),
                    v(w * side * 0.89, 0.74, z),
                    v(w * side, h - 0.43, z),
                    v(w * side * 0.975, h, z),
                ]
            };
            let (a, b) = (section(z1, w1, h1), section(z2, w2, h2));
            for (k, col) in [rust, hull, STRIPE].into_iter().enumerate() {
                let mut face = [a[k], a[k + 1], b[k + 1], b[k]];
                if side < 0. {
                    face.reverse();
                }
                m.quad(face, col);
            }
        }
        m.quad(
            [
                v(-w1 * 0.975, h1, z1),
                v(-w2 * 0.975, h2, z2),
                v(w2 * 0.975, h2, z2),
                v(w1 * 0.975, h1, z1),
            ],
            if z2 <= -4.3 {
                Color::srgb(0.28, 0.54, 0.53)
            } else {
                TIMBER
            },
        );
    }
    let (z, w, h) = stations[stations.len() - 1];
    m.quad(
        [
            v(-w, 0.53, z),
            v(w, 0.53, z),
            v(w * 0.975, h, z),
            v(-w * 0.975, h, z),
        ],
        hull,
    );
    // The ivory wheelhouse sits aft, leaving the cargo hold directly under the hoist.
    m.slab(
        v(0., 3.28, 4.52),
        v(3.02, 1.93, 2.15),
        Color::srgb(0.81, 0.78, 0.65),
    );
    for side in [-1., 1.] {
        for x in [-0.9, 0., 0.9] {
            m.slab(v(x, 3.68, 4.52 + 1.1 * side), v(0.73, 0.76, 0.09), WINDOW);
        }
        m.slab(v(side * 1.54, 3.68, 4.47), v(0.12, 0.76, 1.37), WINDOW);
    }
    m.slab(v(0., 4.35, 4.52), v(3.43, 0.23, 2.49), OXIDE);
    m.stack(v(-0.95, 4.42, 4.96), 1.04, 0.22, 0.19, IRON);
    m.slab(v(0., 2.31, -1.02), v(3.42, 0.23, 6.16), IRON);
    for side in [-1., 1.] {
        m.girder(
            v(side * 1.67, 2.46, -4.08),
            v(side * 1.67, 2.46, 2.0),
            0.18,
            ROOF,
        );
        m.rails(v(side * 2.38, 2.39, -3.66), v(side * 2.38, 2.32, 2.94), 4);
        m.rails(v(side * 0.19, 3.94, -9.92), v(side * 1.43, 3.75, -8.26), 2);
        m.rails(v(side * 1.43, 3.75, -8.26), v(side * 2.26, 3.36, -6.37), 2);
        let bollard = v(side * 1.17, 3.62, -7.78);
        m.stack(bollard, 0.41, 0.19, 0.15, IRON);
        m.girder(
            bollard + Vec3::Y * 0.28,
            v(side * 3.77, 2.94, -8.3),
            0.12,
            TIMBER,
        );
        m.girder(
            v(side * 1.59, 2.53, 5.95),
            v(side * 3.77, 4.27, 7.1),
            0.12,
            TIMBER,
        );
    }
    if progress >= 1 {
        for z in [-3.32, -1.32, 0.68] {
            m.slab(v(0., 2.56, z), v(3.03, 0.32, 0.34), OXIDE);
        }
    }
    if progress >= 2 {
        m.slab(v(0., 2.87, -1.25), v(2.58, 0.35, 4.67), STEEL);
        m.block(
            v(0., 3.12, -1.14),
            v(2.35, 0.19, 4.46),
            Quat::from_rotation_y(0.065),
            ROOF,
        );
    }
    m.finish()
}

/// The fixed masonry apron and landing ladder belong to the island, below the turning gun.
pub(super) fn gun_platform() -> Mesh {
    let mut m = Works::default();
    let v = Vec3::new;
    let joint = Color::srgb(0.35, 0.38, 0.36);
    m.stack(v(0., -1.6, 0.), 5.25, 4.95, 4.55, joint);
    for row in 0..4 {
        let y = row as f32 * 0.96 - 0.32;
        let r = 4.93 - row as f32 * 0.09;
        for i in 0..14 {
            let angle = (i as f32 + 0.5 * (row % 2) as f32) * std::f32::consts::TAU / 14.;
            let color = match (i + 3 * row) % 4 {
                0 => Color::srgb(0.53, 0.54, 0.49),
                1 => Color::srgb(0.61, 0.60, 0.52),
                _ => Color::srgb(0.57, 0.57, 0.52),
            };
            let edge = |a: f32, h: f32, radius: f32| v(a.cos() * radius, h, a.sin() * radius);
            let (a, b) = (angle + 0.005, angle + std::f32::consts::TAU / 14. - 0.005);
            m.quad(
                [
                    edge(a, y, r),
                    edge(a, y + 0.90, r - 0.075),
                    edge(b, y + 0.90, r - 0.075),
                    edge(b, y, r),
                ],
                color,
            );
        }
    }
    m.stack(v(0., 3.45, 0.), 0.47, 4.76, 4.76, CONCRETE);
    // Thick timber fenders hang in front of the seaward masonry, with the steps to land astern.
    for x in [-2.5, 0., 2.5] {
        let z = -(4.75_f32.powi(2) - x * x).sqrt();
        m.slab(v(x, 1.14, z), v(0.40, 4.5, 0.44), TIMBER);
        m.slab(v(x, 2.55, z - 0.05), v(0.48, 0.2, 0.50), IRON);
    }
    for i in 0..4 {
        let h = 0.24 * i as f32;
        m.slab(
            v(0., 3.68 - h, 4.4 + 0.49 * i as f32),
            v(2.2, 0.31, 0.7),
            CONCRETE,
        );
    }
    m.finish()
}

/// The shield, service walk and sight turn with the shore mount. A silenced station keeps a
/// twisted shield and a dark central socket on the same base.
pub(super) fn battery(barrels: u8) -> Mesh {
    let mut m = Works::default();
    let v = Vec3::new;
    m.stack(v(0., 0., 0.), 0.48, 3.65, 3.5, IRON);
    m.stack(v(0., 0.48, 0.), 0.46, 3.12, 3.05, STEEL);
    if barrels == 0 {
        m.stack(v(0., 0.94, 0.), 0.22, 2.32, 2.20, CHAR);
        m.block(
            v(0.8, 1.13, 0.8),
            v(3.1, 0.21, 2.1),
            Quat::from_rotation_z(0.31),
            OXIDE,
        );
        m.girder(v(-2.2, 1.55, -0.1), v(-1.65, 0.83, -4.24), 0.42, IRON);
        return m.finish();
    }
    let width = 0.65 + 0.15 * f32::from(barrels);
    let outline = [
        (-2.4, -1.9),
        (-1.78, -2.52),
        (1.78, -2.52),
        (2.4, -1.9),
        (2.5, 1.26),
        (1.75, 2.),
        (-1.75, 2.),
        (-2.5, 1.26),
    ];
    for i in 0..outline.len() {
        let (x, z) = outline[i];
        let (u, w) = outline[(i + 1) % outline.len()];
        let a = v(x * width, 0.94, z);
        let b = v(u * width, 0.94, w);
        let c = v(x * width * 0.84, 3.12, z * 0.80);
        let d = v(u * width * 0.84, 3.12, w * 0.80);
        m.quad([a, c, d, b], if i == 1 { OXIDE } else { STEEL });
        m.0.triangle([c, v(0., 3.26, 0.), d], ROOF);
    }
    for tube in 0..barrels {
        let x = shipvivor_encounter::ShoreTurret::barrel_x(barrels, tube);
        m.slab(v(x, 1.9, -2.45), v(1.24, 1.07, 0.32), CHAR);
        m.tube(v(x, 1.98, -2.5), v(x, 2.21, -4.48), 0.51, 0.41, STEEL);
        m.tube(v(x, 2.21, -4.48), v(x, 2.63, -8.13), 0.36, 0.25, IRON);
        m.tube(v(x, 2.56, -7.58), v(x, 2.65, -8.24), 0.31, 0.32, STEEL);
        m.tube(v(x, 2.65, -8.24), v(x, 2.65, -8.25), 0.20, 0.20, CHAR);
    }
    m.slab(v(0., 2.71, 1.88), v(1.7, 0.22, 0.11), WINDOW);
    m.slab(v(-1.45 * width, 3.23, 0.24), v(0.46, 0.4, 0.55), IRON);
    for side in [-1., 1.] {
        m.slab(
            v(side * 2.67 * width, 1.05, 0.30),
            v(0.63, 0.19, 2.85),
            STEEL,
        );
        m.rails(
            v(side * 2.90 * width, 1.16, -0.99),
            v(side * 2.90 * width, 1.16, 1.58),
            2,
        );
        m.slab(
            v(side * 2.14 * width, 2.65, 0.4),
            v(0.18, 0.42, 0.86),
            CORAL,
        );
    }
    m.finish()
}

/// Three progress silhouettes, exposed keel, ribs, then plated sides.
pub(super) fn cradle(progress: u8) -> Mesh {
    let mut m = Works::default();
    let v = Vec3::new;
    m.slab(v(0., 0.20, 0.), v(0.40, 0.40, 9.3), IRON);
    for z in [-3.9, -2.45, -0.9, 0.65, 2.2, 3.75] {
        let width = if z < -3. || z > 3. { 1.05 } else { 2.35 };
        m.slab(v(0., 0.30, z), v(width * 1.6, 0.38, 0.47), BRICK);
        if progress >= 1 {
            for side in [-1., 1.] {
                m.girder(v(0., 0.48, z), v(side * width, 1.36, z), 0.23, ROOF);
                m.girder(
                    v(side * width, 1.36, z),
                    v(side * width * 1.1, 2.42, z),
                    0.23,
                    ROOF,
                );
            }
        }
    }
    if progress >= 2 {
        for side in [-1., 1.] {
            m.block(
                v(side * 2.40, 1.49, 0.),
                v(0.25, 1.40, 6.70),
                Quat::from_rotation_z(-side * 0.13),
                STEEL,
            );
            m.girder(
                v(side * 2.5, 1.94, -3.25),
                v(side * 0.45, 2.2, -4.65),
                0.29,
                STRIPE,
            );
        }
    }
    m.finish()
}

/// The gallery is modelled at its built length so its weathering and steelwork retain their
/// proportions. The actor scales its Z axis from one unit to the distance between mill centres.
pub(super) fn conveyor() -> Mesh {
    const SPAN: f32 = 18.;
    let mut m = Works::default();
    let v = Vec3::new;
    m.slab(v(0., 4.13, 0.), v(2.38, 1.71, SPAN), MARINE_BLUE);
    m.block(
        v(-0.64, 5.07, 0.),
        v(1.44, 0.16, SPAN),
        Quat::from_rotation_z(0.13),
        TIN,
    );
    m.block(
        v(0.64, 5.07, 0.),
        v(1.44, 0.16, SPAN),
        Quat::from_rotation_z(-0.13),
        TIN,
    );
    for side in [-1., 1.] {
        let x = 1.23 * side;
        for i in -4..=4 {
            let z = i as f32 * 1.82;
            m.slab(v(x, 4.25, z), v(0.11, 0.84, 1.45), GLAZING);
            m.slab(v(x + side * 0.05, 4.25, z), v(0.11, 0.86, 0.10), IRON);
            m.slab(v(x, 4.05, z + 0.89), v(0.16, 1.67, 0.11), OLD_BLUE);
        }
        m.girder(v(x, 2.63, -8.9), v(x, 2.63, 8.9), 0.20, STEEL);
        for i in 0..9 {
            let z = -8.9 + i as f32 * 1.98;
            let (a, b) = if i % 2 == 0 {
                (2.63, 3.34)
            } else {
                (3.34, 2.63)
            };
            m.girder(v(x, a, z), v(x, b, z + 1.98), 0.15, STEEL);
        }
        for z in [-4.55, 4.55] {
            m.girder(v(x * 1.4, -3.38, z + 0.55), v(x, 3.24, z), 0.32, TIMBER);
            m.girder(v(-x * 1.4, -2.7, z + 0.50), v(x, 2.65, z), 0.21, TIMBER);
        }
    }
    let mut mesh = m.finish();
    mesh.scale_by(Vec3::new(1., 1., 1. / SPAN));
    mesh
}

/// Seaplane shed with its doors toward the water and a timber launching slip.
pub(super) fn hangar(live: bool) -> Mesh {
    let mut w = Works::default();
    let v = Vec3::new;
    w.slab(v(0., 0.18, 0.), v(8.4, 0.36, 8.6), CONCRETE);
    let incline = Quat::from_rotation_z(0.30);
    w.block(v(-8.2, -1.3, 0.), v(9.2, 0.35, 5.), incline, TIMBER);
    for z in [-1.55, 1.55] {
        w.block(v(-8.2, -1.07, z), v(9.2, 0.14, 0.14), incline, STEEL);
    }
    if live {
        w.slab(v(0., 2., 0.), v(7.9, 3.8, 7.7), OLD_BLUE);
        w.slab(v(-3.99, 1.8, 0.), v(0.12, 3.15, 6.1), IRON);
        for z in [-2.7, 2.7] {
            w.slab(v(-4.08, 1.8, z), v(0.14, 3.16, 0.8), STEEL);
        }
        for side in [-1., 1.] {
            w.block(
                v(0., 4.54, side * 2.),
                v(8.6, 0.36, 4.45),
                Quat::from_rotation_x(side * 0.31),
                OXIDE,
            );
            w.slab(v(0., 2.9, side * 3.91), v(6.6, 0.68, 0.1), WINDOW);
            for x in [-2.1, 0., 2.1] {
                w.slab(v(x, 2.9, side * 4.), v(0.13, 0.75, 0.12), STRIPE);
            }
        }
        // Striped windsock alongside the control cabin.
        w.slab(v(3., 4., 2.7), v(2., 7.8, 2.2), CONCRETE);
        w.slab(v(3., 7.4, 2.7), v(2.2, 1., 2.4), WINDOW);
        w.slab(v(3., 8., 2.7), v(2.5, 0.2, 2.7), IRON);
        w.girder(v(3., 8., 2.7), v(3., 9., 2.7), 0.12, IRON);
        for i in 0..4 {
            let t = i as f32;
            let sock = |n: f32| v(3. - n * 0.52, 8.95 - n * 0.14, 2.7 + n * 0.05);
            w.0.tube(
                sock(t),
                sock(t + 1.),
                0.35 - t * 0.05,
                0.30 - t * 0.05,
                if i % 2 == 0 { CORAL } else { STRIPE },
            );
        }
    } else {
        w.slab(v(0., 0.52, 0.), v(7.8, 0.7, 7.5), CHAR);
        for x in [-3., 0., 3.] {
            w.girder(v(x, 0.7, -3.6), v(x + 0.5, 1.7, -0.8), 0.26, IRON);
        }
        w.block(v(1., 1., 0.7), v(5.5, 0.35, 2.7), incline, OXIDE);
    }
    w.finish()
}

/// The source of the plume is the open lip of the stack on each working building.
pub(super) fn factory(harbour: Harbour, live: bool) -> Mesh {
    let mut m = Works::default();
    let v = Vec3::new;
    let lip = harbour.chimney();
    m.pile_deck();
    if !live {
        m.rubble(3.9, 3.9);
        m.stack(v(lip.x, 0.17, lip.z), 1.05, 0.91, 0.82, CHAR);
        return m.finish();
    }

    match harbour {
        Harbour::Cannery => m.cannery(),
        Harbour::TwinMills => m.tidal_mill(),
        Harbour::Foundry => m.foundry_hall(),
    }
    m.chimney(harbour);
    m.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::camera::primitives::MeshAabb;

    #[test]
    fn the_targetable_skyline_drops_with_each_stage_of_dock_damage() {
        let states = [
            (YardState::Working, 20.),
            (YardState::Damaged, 15.),
            (YardState::Burning, 9.),
            (YardState::Ruined, 4.5),
        ];
        for harbour in Harbour::ALL {
            for (state, roof) in states {
                let bounds = dock(harbour, state).get_aabb().unwrap();
                let top = bounds.max().y + 3.2;
                assert!(
                    (top - roof).abs() < 0.5,
                    "{state:?}: visible roof {top} m, target roof {roof} m"
                );
            }
            for (live, roof) in [(true, 12.2), (false, 4.5)] {
                let top = factory(harbour, live).get_aabb().unwrap().max().y + 3.2;
                assert!((top - roof).abs() < 0.5);
            }
        }
    }
}
