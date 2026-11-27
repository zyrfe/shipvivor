//! An airfield's ground and buildings, built in the island's own frame: x along the
//! runway, z out from the shore, y up from the waterline. A building's own model stands
//! on its foot with its door toward the taxiway, out from the shore. A truck's nose is
//! local -Z, as every vessel's and aircraft's is.
use crate::scene::Facets;
use crate::surfaces::weathered_mesh;
use bevy::prelude::*;
use shipvivor_encounter::{
    DEPOT, FACTORIES, HANGAR_SIZE, HANGARS, RUNWAY, RUNWAY_HALF_WIDTH, RUNWAY_LINE, SHELTER_LINE,
    SHELTER_SIZE, SHELTERS, TAXIWAY_END, TAXIWAY_LINE, TOWER, TOWER_SIZE,
};

/// The island's level top, which the ground is laid on.
pub(super) const LEVEL: f32 = 3.13;

const ASPHALT: Color = Color::srgb(0.17, 0.18, 0.19);
const TARMAC: Color = Color::srgb(0.25, 0.26, 0.27);
const APRON: Color = Color::srgb(0.47, 0.47, 0.45);
const GRAVEL: Color = Color::srgb(0.38, 0.37, 0.34);
const PAINT: Color = Color::srgb(0.86, 0.85, 0.80);
const TAXI_LINE: Color = Color::srgb(0.82, 0.63, 0.18);
const CONCRETE: Color = Color::srgb(0.58, 0.57, 0.53);
const WEATHERED: Color = Color::srgb(0.46, 0.46, 0.43);
const DOOR: Color = Color::srgb(0.20, 0.23, 0.22);
const WALL: Color = Color::srgb(0.40, 0.44, 0.40);
const ROOF: Color = Color::srgb(0.43, 0.22, 0.17);
const STEEL: Color = Color::srgb(0.30, 0.32, 0.33);
const GLAZING: Color = Color::srgb(0.30, 0.52, 0.60);
const CHAR: Color = Color::srgb(0.10, 0.09, 0.08);
const TANK: Color = Color::srgb(0.74, 0.74, 0.70);
const BAND: Color = Color::srgb(0.70, 0.16, 0.12);
const DRAB: Color = Color::srgb(0.33, 0.36, 0.26);
const TYRE: Color = Color::srgb(0.08, 0.08, 0.08);

struct Build(Facets);
impl Build {
    fn new() -> Self {
        Self(Facets::default())
    }
    /// A flat patch of the field from `(x0, z0)` to `(x1, z1)` at height `y`, facing up.
    fn patch(&mut self, (x0, z0): (f32, f32), (x1, z1): (f32, f32), y: f32, color: Color) {
        let (x0, x1) = (x0.min(x1), x0.max(x1));
        let (z0, z1) = (z0.min(z1), z0.max(z1));
        self.0.quad(
            [
                Vec3::new(x0, y, z0),
                Vec3::new(x0, y, z1),
                Vec3::new(x1, y, z1),
                Vec3::new(x1, y, z0),
            ],
            color,
        );
    }
    /// A box about `center` of full `size`.
    fn block(&mut self, center: Vec3, size: Vec3, color: Color) {
        self.turned(center, size, Quat::IDENTITY, color);
    }
    fn turned(&mut self, center: Vec3, size: Vec3, rotation: Quat, color: Color) {
        let v = [
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
            self.0.quad(face.map(|i| v[i]), color);
        }
    }
    /// A girder of `width` from one point to another.
    fn girder(&mut self, from: Vec3, to: Vec3, width: f32, color: Color) {
        let axis = to - from;
        self.turned(
            (from + to) * 0.5,
            Vec3::new(width, axis.length(), width),
            Quat::from_rotation_arc(Vec3::Y, axis.normalize()),
            color,
        );
    }
    /// An upright drum standing on `foot`.
    fn drum(&mut self, foot: Vec3, radius: f32, height: f32, color: Color) {
        self.0
            .tube(foot, foot + Vec3::Y * height, radius, radius, color);
    }
    /// A low heap of broken concrete and bent steel over a scorched patch.
    fn rubble(&mut self, half: Vec2, seed: u32) {
        self.patch((-half.x, -half.y), (half.x, half.y), 0.06, CHAR);
        for i in 0..9_u32 {
            let k = (seed + i * 7919) as f32;
            let x = ((k * 0.37).sin() * 0.8) * half.x;
            let z = ((k * 0.73).cos() * 0.8) * half.y;
            let s = 0.8 + 0.9 * (k * 1.3).sin().abs();
            let tilt = Quat::from_rotation_y(k) * Quat::from_rotation_x(0.3 * (k * 0.5).sin());
            let color = if i % 3 == 0 { WEATHERED } else { CONCRETE };
            self.turned(
                Vec3::new(x, 0.35 * s, z),
                Vec3::new(1.6, 0.7, 1.1) * s,
                tilt,
                color,
            );
        }
        for i in 0..4_u32 {
            let k = (seed + i * 104_729) as f32;
            let from = Vec3::new(
                (k * 0.9).sin() * half.x * 0.7,
                0.2,
                (k * 0.4).cos() * half.y * 0.7,
            );
            let to = from
                + Vec3::new(
                    (k * 1.7).cos() * 3.5,
                    1.4 + (k * 0.3).sin(),
                    (k * 2.1).sin() * 2.5,
                );
            self.girder(from, to, 0.25, STEEL);
        }
    }
    fn finish(self) -> Mesh {
        weathered_mesh(self.0, 5.3)
    }
}

/// The paving, roads and markings of the field, in the island's frame from its middle.
pub(super) fn ground() -> Mesh {
    let mut m = Build::new();
    let y = LEVEL + 0.02;
    // The concrete behind the runway, where the shelters and hangars stand, and the yard
    // round the depot, tower and factories behind them.
    m.patch((-50., SHELTER_LINE - 6.), (50., -2.6), y, APRON);
    let back = DEPOT.1 - 6.;
    m.patch((-38., back), (38., SHELTER_LINE - 6.), y, GRAVEL);
    // The roads the trucks run along, and the lanes between the shelters.
    m.patch((-30., -25.6), (36., -22.4), y + 0.01, TARMAC);
    for i in 0..SHELTERS.len() - 1 {
        let lane = (SHELTERS[i] + SHELTERS[i + 1]) * 0.5;
        m.patch((lane - 1.5, -24.), (lane + 1.5, -6.), y + 0.01, TARMAC);
    }
    m.patch((-30., -7.3), (30., -4.7), y + 0.01, TARMAC);
    // The taxiway, its links to the runway and its yellow line.
    let taxi = y + 0.02;
    m.patch(
        (-TAXIWAY_END - 2., TAXIWAY_LINE - 2.3),
        (TAXIWAY_END + 2., TAXIWAY_LINE + 2.3),
        taxi,
        ASPHALT,
    );
    m.patch(
        (RUNWAY.0 - 2.3, TAXIWAY_LINE),
        (RUNWAY.0 + 6., RUNWAY_LINE),
        taxi,
        ASPHALT,
    );
    m.patch((-17., TAXIWAY_LINE), (-9., RUNWAY_LINE), taxi, ASPHALT);
    let line = taxi + 0.01;
    m.patch(
        (-TAXIWAY_END, TAXIWAY_LINE - 0.12),
        (TAXIWAY_END, TAXIWAY_LINE + 0.12),
        line,
        TAXI_LINE,
    );
    for &along in &SHELTERS {
        m.patch(
            (along - 0.12, SHELTER_LINE + SHELTER_SIZE.1),
            (along + 0.12, TAXIWAY_LINE),
            line,
            TAXI_LINE,
        );
    }
    // The runway: its edges, a dashed centre line and the bars across each threshold.
    let runway = taxi + 0.01;
    let (low, high) = (
        RUNWAY_LINE - RUNWAY_HALF_WIDTH,
        RUNWAY_LINE + RUNWAY_HALF_WIDTH,
    );
    m.patch((RUNWAY.0 - 4., low), (RUNWAY.1 + 4., high), runway, ASPHALT);
    let paint = runway + 0.01;
    for edge in [low + 0.25, high - 0.25] {
        m.patch(
            (RUNWAY.0 - 3.6, edge - 0.12),
            (RUNWAY.1 + 3.6, edge + 0.12),
            paint,
            PAINT,
        );
    }
    let mut along = RUNWAY.0 + 6.;
    while along < RUNWAY.1 - 6. {
        m.patch(
            (along, RUNWAY_LINE - 0.14),
            (along + 3., RUNWAY_LINE + 0.14),
            paint,
            PAINT,
        );
        along += 6.;
    }
    for end in [RUNWAY.0, RUNWAY.1] {
        let inward = if end < 0. { 1. } else { -1. };
        for k in 0..6 {
            let z = low + 0.9 + k as f32 * 1.05;
            m.patch((end, z - 0.3), (end + inward * 3.2, z + 0.3), paint, PAINT);
        }
    }
    // Hardstandings under the hangars, the tower and the factories.
    for &(x, z) in HANGARS.iter().chain(FACTORIES.iter()) {
        m.patch((x - 7., z - 6.5), (x + 7., z + 6.5), y + 0.005, APRON);
    }
    m.patch(
        (TOWER.0 - 4., TOWER.1 - 4.),
        (TOWER.0 + 4., TOWER.1 + 4.),
        y + 0.005,
        APRON,
    );
    m.finish()
}

/// A hardened shelter: a concrete barrel vault, its armoured doors toward the taxiway.
pub(super) fn shelter() -> Mesh {
    let mut m = Build::new();
    let (half_x, half_z, height) = SHELTER_SIZE;
    let arc = |i: usize, grow: f32| {
        let a = std::f32::consts::PI * i as f32 / 10.;
        Vec2::new(a.cos() * half_x * grow, a.sin() * height * grow)
    };
    let (back, front) = (-half_z, half_z);
    for i in 0..10 {
        let (p, q) = (arc(i, 1.), arc(i + 1, 1.));
        let paint = if i % 2 == 0 { CONCRETE } else { WEATHERED };
        m.0.quad(
            [
                Vec3::new(q.x, q.y, back),
                Vec3::new(q.x, q.y, front),
                Vec3::new(p.x, p.y, front),
                Vec3::new(p.x, p.y, back),
            ],
            paint,
        );
        // The back wall, and the thick lip round the doorway.
        m.0.triangle(
            [
                Vec3::new(0., 0., back),
                Vec3::new(q.x, q.y, back),
                Vec3::new(p.x, p.y, back),
            ],
            CONCRETE,
        );
        let (pl, ql) = (arc(i, 1.12), arc(i + 1, 1.12));
        m.0.quad(
            [
                Vec3::new(p.x, p.y, front + 0.35),
                Vec3::new(pl.x, pl.y, front + 0.35),
                Vec3::new(ql.x, ql.y, front + 0.35),
                Vec3::new(q.x, q.y, front + 0.35),
            ],
            WEATHERED,
        );
        m.0.quad(
            [
                Vec3::new(ql.x, ql.y, front),
                Vec3::new(ql.x, ql.y, front + 0.35),
                Vec3::new(pl.x, pl.y, front + 0.35),
                Vec3::new(pl.x, pl.y, front),
            ],
            CONCRETE,
        );
        // The doors, set back in the mouth, part open in the middle.
        let (dp, dq) = (arc(i, 0.97), arc(i + 1, 0.97));
        let door = front - 0.25;
        if dp.x.abs() > 0.9 || dq.x.abs() > 0.9 {
            m.0.triangle(
                [
                    Vec3::new(0., 0., door),
                    Vec3::new(dp.x, dp.y, door),
                    Vec3::new(dq.x, dq.y, door),
                ],
                DOOR,
            );
        }
    }
    // Earth banked up the flanks.
    for side in [-1., 1.] {
        let mut bank = [
            Vec3::new(side * (half_x + 1.4), 0., back),
            Vec3::new(side * half_x * 0.9, height * 0.45, back),
            Vec3::new(side * half_x * 0.9, height * 0.45, front),
            Vec3::new(side * (half_x + 1.4), 0., front),
        ];
        if side < 0. {
            bank.reverse();
        }
        m.0.quad(bank, GRAVEL);
    }
    m.finish()
}

/// A hangar: a long shed with a pitched roof and great doors toward the taxiway, or its
/// wreck.
pub(super) fn hangar(live: bool) -> Mesh {
    let mut m = Build::new();
    let (hx, hz, top) = HANGAR_SIZE;
    if !live {
        m.rubble(Vec2::new(hx, hz), 11);
        m.block(
            Vec3::new(-hx + 0.3, 1.2, 0.),
            Vec3::new(0.5, 2.4, hz * 2.),
            WALL,
        );
        m.girder(
            Vec3::new(-hx, 0., -hz),
            Vec3::new(-hx + 3., 4.2, -hz + 2.),
            0.35,
            STEEL,
        );
        return m.finish();
    }
    let eaves = top - 2.;
    m.block(
        Vec3::new(0., eaves * 0.5, 0.),
        Vec3::new(hx * 2., eaves, hz * 2.),
        WALL,
    );
    // The roof pitches along the shed, its ridge running out from the shore.
    for side in [-1., 1.] {
        let mut slope = [
            Vec3::new(side * (hx + 0.4), eaves - 0.1, -hz - 0.4),
            Vec3::new(0., top, -hz - 0.4),
            Vec3::new(0., top, hz + 0.4),
            Vec3::new(side * (hx + 0.4), eaves - 0.1, hz + 0.4),
        ];
        if side < 0. {
            slope.reverse();
        }
        m.0.quad(slope, ROOF);
        for z in [-hz, hz] {
            let mut gable = [
                Vec3::new(side * hx, eaves, z),
                Vec3::new(0., top - 0.2, z),
                Vec3::new(0., eaves, z),
            ];
            if (side < 0.) == (z > 0.) {
                gable.reverse();
            }
            m.0.triangle(gable, WALL);
        }
    }
    // The doors, and a strip of glazing above them.
    m.block(
        Vec3::new(0., eaves * 0.42, hz + 0.05),
        Vec3::new(hx * 1.6, eaves * 0.84, 0.15),
        DOOR,
    );
    m.block(
        Vec3::new(0., eaves * 0.92, hz + 0.05),
        Vec3::new(hx * 1.6, 0.45, 0.12),
        GLAZING,
    );
    for x in [-hx * 0.8, 0., hx * 0.8] {
        m.block(
            Vec3::new(x, eaves * 0.45, hz + 0.12),
            Vec3::new(0.18, eaves * 0.9, 0.12),
            STEEL,
        );
    }
    m.finish()
}

/// The control tower: a concrete shaft with a glazed cab, a gallery and a mast, or its
/// broken stump.
pub(super) fn tower(live: bool) -> Mesh {
    let mut m = Build::new();
    let (hx, hz, top) = TOWER_SIZE;
    if !live {
        m.rubble(Vec2::new(hx + 1.5, hz + 1.5), 23);
        m.block(
            Vec3::new(0., 1., 0.),
            Vec3::new(hx * 2., 2., hz * 2.),
            WEATHERED,
        );
        m.turned(
            Vec3::new(2.2, 0.8, 1.4),
            Vec3::new(5.2, 0.4, 5.2),
            Quat::from_rotation_z(0.5) * Quat::from_rotation_x(0.2),
            CONCRETE,
        );
        return m.finish();
    }
    let shaft = top - 3.2;
    m.block(
        Vec3::new(0., shaft * 0.5, 0.),
        Vec3::new(hx * 2., shaft, hz * 2.),
        CONCRETE,
    );
    for y in [shaft * 0.35, shaft * 0.7] {
        m.block(Vec3::new(0., y, hz + 0.02), Vec3::new(1.2, 0.6, 0.06), DOOR);
    }
    m.block(
        Vec3::new(0., shaft + 0.1, 0.),
        Vec3::new(hx * 2. + 1.8, 0.2, hz * 2. + 1.8),
        WEATHERED,
    );
    m.block(
        Vec3::new(0., shaft + 1.2, 0.),
        Vec3::new(hx * 2. + 0.8, 2.2, hz * 2. + 0.8),
        GLAZING,
    );
    for (x, z) in [(1., 1.), (-1., 1.), (1., -1.), (-1., -1.)] {
        let at = Vec3::new(x * (hx + 0.38), shaft + 1.2, z * (hz + 0.38));
        m.block(at, Vec3::new(0.2, 2.2, 0.2), STEEL);
    }
    m.block(
        Vec3::new(0., shaft + 2.45, 0.),
        Vec3::new(hx * 2. + 1.4, 0.3, hz * 2. + 1.4),
        STEEL,
    );
    m.0.tube(
        Vec3::new(0.6, shaft + 2.6, 0.),
        Vec3::new(0.6, top + 2., 0.),
        0.07,
        0.04,
        STEEL,
    );
    m.0.tube(
        Vec3::new(-0.9, shaft + 2.6, 0.4),
        Vec3::new(-0.9, shaft + 3.4, 0.4),
        0.22,
        0.22,
        BAND,
    );
    m.finish()
}

/// The fuel depot: three tanks in a row on a bunded pad with their pipework, or the
/// burnt-out shells.
pub(super) fn depot(live: bool) -> Mesh {
    let mut m = Build::new();
    m.patch(
        (-8.2, -3.6),
        (8.2, 3.6),
        0.05,
        if live { WEATHERED } else { CHAR },
    );
    for x in [-5.3, 0., 5.3] {
        if live {
            m.drum(Vec3::new(x, 0.05, 0.), 2.3, 3.9, TANK);
            m.drum(Vec3::new(x, 2.4, 0.), 2.33, 0.45, BAND);
            m.0.tube(
                Vec3::new(x, 3.95, 0.),
                Vec3::new(x, 4.5, 0.),
                2.3,
                0.6,
                TANK,
            );
        } else {
            m.drum(Vec3::new(x, 0.05, 0.), 2.3, 1.1, CHAR);
            m.turned(
                Vec3::new(x + 0.8, 0.5, 1.2),
                Vec3::new(2.5, 0.3, 1.9),
                Quat::from_rotation_z(0.6),
                STEEL,
            );
        }
    }
    if live {
        m.0.tube(
            Vec3::new(-7., 0.8, 2.8),
            Vec3::new(7., 0.8, 2.8),
            0.18,
            0.18,
            STEEL,
        );
        for x in [-5.3, 0., 5.3] {
            m.0.tube(
                Vec3::new(x, 0.8, 2.3),
                Vec3::new(x, 0.8, 2.8),
                0.15,
                0.15,
                STEEL,
            );
        }
    }
    m.finish()
}

/// A fuel truck: a tanker on a short chassis, its cab at the front.
pub(super) fn truck() -> Mesh {
    let mut m = Build::new();
    m.block(Vec3::new(0., 0.55, 0.2), Vec3::new(1.7, 0.35, 4.2), STEEL);
    m.block(Vec3::new(0., 1.25, -1.45), Vec3::new(1.75, 1.3, 1.1), DRAB);
    m.block(
        Vec3::new(0., 1.55, -2.02),
        Vec3::new(1.5, 0.5, 0.05),
        GLAZING,
    );
    m.0.tube(
        Vec3::new(0., 1.45, -0.75),
        Vec3::new(0., 1.45, 2.2),
        0.78,
        0.78,
        DRAB,
    );
    m.block(Vec3::new(0., 1.45, 0.7), Vec3::new(1.6, 0.3, 0.6), BAND);
    for z in [-1.4, 0.6, 1.6] {
        for x in [-0.85, 0.85] {
            m.block(Vec3::new(x, 0.38, z), Vec3::new(0.3, 0.76, 0.76), TYRE);
        }
    }
    m.finish()
}
