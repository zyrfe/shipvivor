//! Heavy yards from the last two patrols. All dimensions are metres, with the
//! seaward face toward -Z. Moving loads have their own pivot and mesh.
use super::*;
use shipvivor_encounter::IndustrialYard;
use std::f32::consts::{PI, TAU};

const ENAMEL: Color = Color::srgb(0.98, 0.94, 0.77);
const COPPER: Color = Color::srgb(0.72, 0.48, 0.24);
const COAL: Color = Color::srgb(0.12, 0.14, 0.16);
const HEAT: Color = Color::srgb(1., 0.42, 0.12);
const ASH: Color = Color::srgb(0.34, 0.32, 0.29);

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub(crate) enum IndustrialPart {
    /// Roof bays or hammer supports collapsing in order, from 0 to 3.
    Hall(u8),
    Heat(u8),
    Machine,
    Discharge,
    /// True once the housing or tank has been destroyed.
    Governor(bool),
    GaugeLight,
    Fuel(bool),
    Casemate,
    Shutter,
    Quay,
    DockHouse(YardState),
}

impl IndustrialPart {
    pub const ALL: [Self; 22] = [
        Self::Hall(0),
        Self::Hall(1),
        Self::Hall(2),
        Self::Hall(3),
        Self::Heat(0),
        Self::Heat(1),
        Self::Heat(2),
        Self::Heat(3),
        Self::Machine,
        Self::Discharge,
        Self::Governor(false),
        Self::Governor(true),
        Self::GaugeLight,
        Self::Fuel(false),
        Self::Fuel(true),
        Self::Casemate,
        Self::Shutter,
        Self::Quay,
        Self::DockHouse(YardState::Working),
        Self::DockHouse(YardState::Damaged),
        Self::DockHouse(YardState::Burning),
        Self::DockHouse(YardState::Ruined),
    ];
}

pub(crate) fn industrial(kind: IndustrialYard, part: IndustrialPart) -> Mesh {
    let mut m = Works::default();
    match part {
        IndustrialPart::Hall(stage) => match kind {
            IndustrialYard::RedShift => mill(&mut m, stage),
            IndustrialYard::BellWorks => forge(&mut m, stage),
        },
        IndustrialPart::Heat(stage) => furnace_light(&mut m, kind, stage),
        IndustrialPart::Machine => match kind {
            IndustrialYard::RedShift => hopper(&mut m),
            IndustrialYard::BellWorks => hammer(&mut m),
        },
        IndustrialPart::Discharge => ore_stream(&mut m),
        IndustrialPart::Governor(ruined) => governor(&mut m, ruined),
        IndustrialPart::GaugeLight => {
            m.slab(
                Vec3::new(0., 2.29, -1.36),
                Vec3::new(2.27, 0.46, 0.12),
                FURNACE,
            );
            // The illuminated upper rim catches the eye at bridge distance.
            // Its centre stays pale enamel, with the dark needle free to read.
            let rim = |i: usize| {
                let angle = (i as f32 / 8. - 0.5) * PI;
                Vec3::new(angle.sin() * 2.37, 5.85 + angle.cos() * 2.37, -1.52)
            };
            for arc in 0..8 {
                m.girder(rim(arc), rim(arc + 1), 0.29, FURNACE);
            }
        }
        IndustrialPart::Fuel(ruined) => fuel(&mut m, ruined),
        IndustrialPart::Casemate => casemate(&mut m),
        IndustrialPart::Shutter => shutter(&mut m),
        IndustrialPart::Quay => quay(&mut m),
        IndustrialPart::DockHouse(state) => winch_house(&mut m, state),
    }
    m.finish()
}

pub(crate) fn industrial_chimney(kind: IndustrialYard) -> Vec3 {
    match kind {
        IndustrialYard::RedShift => Vec3::new(-21.8, 41., 7.),
        IndustrialYard::BellWorks => Vec3::new(-8.8, 28., 7.5),
    }
}

/// The hopper pivots about its discharge end. The forge ram strokes on its
/// guides until the governors trip, then drops below its ordinary stroke.
pub(crate) fn machine_pose(kind: IndustrialYard, collapse: f32, clock: f32) -> Transform {
    let t = collapse.clamp(0., 1.);
    match kind {
        IndustrialYard::RedShift => Transform::from_xyz(15. - 1.4 * t, 28.7 - 21.3 * t * t, 0.)
            .with_rotation(Quat::from_rotation_z(0.37 * t * t)),
        IndustrialYard::BellWorks => {
            // The engine lifts the head slowly. It drops in the last fifth of
            // a turn, hits the glowing die, then dwells before the next lift.
            let turn = (clock / IndustrialYard::HAMMER_PERIOD).rem_euclid(1.);
            let lift = (turn / 0.72).min(1.);
            let impact = ((turn - 0.72) / (IndustrialYard::HAMMER_STRIKE - 0.72)).clamp(0., 1.);
            let working = 3.25 + 11. * lift * (1. - impact.powi(2));
            let drop = (t * 1.7).min(1.).powi(2);
            Transform::from_xyz(0., working * (1. - drop) + 1.4 * drop, 0.)
                .with_rotation(Quat::from_rotation_z(0.18 * drop))
        }
    }
}

fn smokestack(m: &mut Works, lip: Vec3, height: f32) {
    let bottom = lip - Vec3::Y * height;
    m.slab(bottom + Vec3::Y * 0.9, Vec3::new(4., 1.8, 4.), ASH);
    m.stack(bottom, height - 0.8, 1.65, 1.04, BRICK);
    for fraction in [0.32, 0.49, 0.67, 0.84] {
        let radius = 1.76 - 0.62 * fraction;
        m.stack(
            bottom + Vec3::Y * (fraction * height),
            0.28,
            radius,
            radius,
            IRON,
        );
    }
    m.stack(lip - 0.8 * Vec3::Y, 0.8, 1.28, 1.26, CHAR);
    m.stack(lip, 0.08, 0.87, 0.87, COAL);
}

const MILL_BAYS: [u8; 6] = [1, 1, 2, 2, 3, 4];

fn mill(m: &mut Works, stage: u8) {
    let v = Vec3::new;
    // Even after the roof comes down the masonry plinth remains on its rock.
    m.slab(v(0., -0.75, 0.), v(56., 2.1, 20.), CONCRETE);
    // A black interior behind separate brick piers lets the furnace doors and
    // clerestory read from across the bay. The bays below the falling end
    // of the hopper give first, then the collapse runs down the waterfront.
    for (bay, falls) in MILL_BAYS.into_iter().enumerate() {
        let x = (bay as f32 - 2.5) * 9.;
        let down = stage >= falls;
        let floor: f32 = if down { 1.25 } else { 13.2 };
        let roof_low = floor + 0.2;
        let roof_high = floor + if down { 1.2 } else { 4.6 };
        m.slab(
            v(x, floor * 0.5, 0.),
            v(8.9, floor, 16.9),
            if down { CHAR } else { BRICK },
        );
        // The sloping plate of each roof ends at a glazed vertical north light.
        m.block(
            v(x, (roof_low + roof_high) * 0.5, 0.),
            v(
                (9_f32.powi(2) + (roof_high - roof_low).powi(2)).sqrt(),
                0.42,
                18.,
            ),
            Quat::from_rotation_z(((roof_high - roof_low) / 9.).atan()),
            if down { ASH } else { TIN },
        );
        m.slab(
            v(x + 4.35, (roof_low + roof_high) * 0.5, 0.),
            v(0.36, roof_high - roof_low, 17.4),
            if down { CHAR } else { GLAZING },
        );
        if down {
            m.girder(
                v(x - 4.2, 0.65, -7.9),
                v(x + 3.8, roof_high + 0.8, -5.2),
                0.65,
                OXIDE,
            );
            continue;
        }
        for side in [-1., 1.] {
            let z = side * 8.55;
            // The north lights climb each sawtooth above a triangular brick
            // cheek. A high amber sash sits clear of the furnace door below.
            let low = v(x - side * 4.4, roof_low, z);
            let high = v(x + 4.4, roof_high, z);
            let right = v(x + side * 4.4, roof_low, z);
            m.0.triangle([low, right, high], BRICK);
            m.slab(v(x + 2.38, roof_low + 0.82, z), v(3.4, 1.02, 0.46), GLAZING);
            m.slab(
                v(x + 2.38, roof_low + 0.82, z + side * 0.24),
                v(0.22, 1.1, 0.11),
                IRON,
            );
            m.slab(v(x, 4.45, z), v(6.65, 7.7, 0.38), IRON);
            m.slab(v(x, 3.9, z + 0.26 * side), v(5.8, 5.9, 0.26), HEAT);
            m.slab(v(x, 2.6, z + 0.41 * side), v(4.98, 2.3, 0.16), FURNACE);
            m.slab(v(x, 3.7, z + 0.58 * side), v(4.96, 0.39, 0.20), CHAR);
            for offset in [-2.25, 0., 2.25] {
                m.slab(
                    v(x + offset, 4., z + 0.47 * side),
                    v(0.27, 6.45, 0.24),
                    IRON,
                );
            }
            m.slab(v(x, 10.7, z), v(7.65, 3.35, 0.83), IRON);
            m.slab(v(x - 4.1, 6.6, z), v(0.8, 13.2, 0.95), OXIDE);
            m.girder(
                v(x - 4.52, roof_low, z),
                v(x + 4.52, roof_high, z),
                0.4,
                IRON,
            );
        }
    }
    let mouth = industrial_chimney(IndustrialYard::RedShift);
    smokestack(m, mouth, mouth.y);
    smokestack(m, v(-14.8, 32.5, 7.), 32.5);
    // Paired bent frames carry the ore bin high enough to silhouette its belly.
    for x in [-18_f32, -6., 6., 15.] {
        for side in [-1., 1.] {
            let height = if stage >= 2 && x.abs() < 10. {
                5.3
            } else {
                28.8
            };
            let foot = v(x, 0., side * 5.7);
            let crown = v(x, height, side * 3.);
            m.girder(foot, crown, 1.42, IRON);
            m.girder(foot + Vec3::X * 4.3, crown, 0.94, STEEL);
            if stage == 0 && x < 15. {
                m.girder(foot.lerp(crown, 0.59), crown + Vec3::X * 8., 0.88, TIN);
            }
        }
    }
    if stage == 0 {
        m.slab(v(-1.5, 28.15, -4.35), v(35., 0.76, 2.6), STEEL);
        m.rails(v(-19., 28.54, -5.48), v(16., 28.54, -5.48), 11);
    }
}

fn hopper(m: &mut Works) {
    let v = Vec3::new;
    // The asymmetric trough pivots at the right. A ridge of actual ore rises
    // above the lip. Projecting ribs show the tip even against a dark hillside.
    let bottom = [
        v(-31., 0., -3.85),
        v(0.2, 0., -3.85),
        v(0.2, 0., 3.85),
        v(-31., 0., 3.85),
    ];
    let top = [
        v(-35., 7., -4.7),
        v(3., 7., -4.7),
        v(3., 7., 4.7),
        v(-35., 7., 4.7),
    ];
    for i in 0..4 {
        let j = (i + 1) % 4;
        m.quad([bottom[i], top[i], top[j], bottom[j]], OXIDE);
        m.girder(top[i], top[j], 0.57, IRON);
        // The continuous rolled lips and deep vertical side plates distinguish
        // an iron vessel carrying ore from a roof across the bent frames.
        m.girder(bottom[i], bottom[j], 0.87, IRON);
        m.girder(
            bottom[i].lerp(top[i], 0.83),
            bottom[j].lerp(top[j], 0.83),
            0.42,
            COPPER,
        );
    }
    for i in 0..9 {
        let t = i as f32 / 8.;
        for (a, b) in [(0, 1), (3, 2)] {
            m.girder(
                top[a].lerp(top[b], t),
                bottom[a].lerp(bottom[b], t),
                0.78,
                COPPER,
            );
        }
    }
    let crests = [7.2, 9.7, 10.4, 9.85, 10.22, 10.05, 9.55, 7.2];
    let section = |i: usize| {
        let x = -34.7 + i as f32 * 5.3;
        [
            v(x, 7., -4.56),
            v(x, crests[i], if i % 2 == 0 { -1.75 } else { -0.95 }),
            v(x, 7., 4.56),
        ]
    };
    for i in 0..crests.len() - 1 {
        let [near, ridge, far] = section(i);
        let [next_near, next_ridge, next_far] = section(i + 1);
        m.0.triangle([near, ridge, next_ridge], COAL);
        m.0.triangle(
            [near, next_ridge, next_near],
            if i % 3 == 0 { CHAR } else { COAL },
        );
        m.quad([ridge, far, next_far, next_ridge], CHAR);
    }
}

// A short sheet of ore pours over the seaward lip. The lower edge falls away
// from the building, standing clear of the roof even when viewed broadside.
fn ore_stream(m: &mut Works) {
    for i in 0..8 {
        let x = i as f32 - 3.5;
        let top = Vec3::new(x * 0.76, 0., 0.);
        let bottom = Vec3::new(x * 1.13 - 1.7, -1., -1. - 0.04 * (i % 3) as f32);
        m.quad(
            [top, top + Vec3::X * 0.79, bottom + Vec3::X * 1.21, bottom],
            if i % 3 == 0 { CHAR } else { COAL },
        );
    }
}

pub(crate) fn ore_fall_pose(collapse: f32) -> Transform {
    let lip = machine_pose(IndustrialYard::RedShift, collapse, 0.)
        .transform_point(Vec3::new(-30.5, 6.8, -4.75));
    let run = ((collapse - 0.12) / 0.22).clamp(0., 1.);
    let thickness = ((0.94 - collapse) / 0.18).clamp(0.02, 1.);
    Transform::from_translation(lip).with_scale(Vec3::new(
        thickness,
        run * (lip.y - 2.3).max(0.1),
        run * 5.4,
    ))
}

fn forge(m: &mut Works, stage: u8) {
    let v = Vec3::new;
    m.slab(v(0., -0.7, 0.), v(26., 2., 22.), CONCRETE);
    m.slab(v(0., 1.4, 0.), v(16., 2.6, 13.), BRICK);
    m.slab(v(0., 2.83, 0.), v(13.7, 0.48, 10.8), IRON);
    m.slab(
        v(0., 3.1, 0.),
        v(15., 0.5, 12.6),
        if stage < 2 { FURNACE } else { OXIDE },
    );
    // A tapered four-legged press. Each pair folds towards the die at a
    // different point in the descent, leaving a recognisable empty frame.
    let crowns = [(4.8, 32.), (4.8, 32.), (4.3, 29.), (3.3, 24.8)];
    let (spread, height) = crowns[usize::from(stage.min(3))];
    for x in [-1., 1.] {
        for z in [-1., 1.] {
            let shift = if stage >= 2 { x * 2. } else { 0. };
            let base = v(x * 10.7, 1., z * 6.7);
            let head = v(x * spread, height + shift, z * 3.);
            m.slab(base, v(4.3, 2.8, 4.3), ASH);
            m.girder(base, head, 2.35, STEEL);
            let flange = Vec3::Z * (z * 1.1);
            m.girder(base + flange, head + flange, 0.66, OXIDE);
            for i in 1..8 {
                let rivet = base.lerp(head, i as f32 / 8.) + flange;
                m.tube(rivet, rivet + Vec3::Z * z * 0.4, 0.30, 0.22, STRIPE);
            }
        }
        m.girder(
            v(x * spread, height + 0.1, -3.3),
            v(x * spread, height + 0.1, 3.3),
            2.2,
            IRON,
        );
    }
    m.block(
        v(0., height, 0.),
        v(2. * spread + 3.0, 2.8, 8.2),
        Quat::from_rotation_z(if stage >= 2 { 0.18 } else { 0. }),
        IRON,
    );
    if stage < 3 {
        m.stack(v(0., height + 1.0, 0.), 2.8, 3.65, 3.45, COPPER);
        m.stack(v(0., height + 3.65, 0.), 1.04, 3.68, 3.58, CHAR);
        m.stack(v(0., height - 13., 0.), 12., 2.48, 2.84, STEEL);
        for y in [-13., -9., -2.] {
            m.stack(v(0., height + y, 0.), 0.68, 2.95, 2.95, COPPER);
        }
    }
    let mouth = industrial_chimney(IndustrialYard::BellWorks);
    smokestack(m, mouth, mouth.y);
    smokestack(m, v(8.7, 23.8, 7.7), 23.8);
    // A brick boiler house feeds the press through a brass-jointed steam
    // main. Keeping machinery off the centre line leaves the die visible.
    let boiler_height = if stage < 2 { 10.8 } else { 6.5 };
    m.slab(
        v(8.25, boiler_height * 0.5, 3.1),
        v(8.15, boiler_height, 10.9),
        BRICK,
    );
    m.slab(v(8.25, boiler_height, 3.1), v(8.78, 0.55, 11.5), IRON);
    for x in [5.65, 8.2, 10.75] {
        m.slab(v(x, 4.25, -2.42), v(1.80, 4.70, 0.35), CHAR);
        if stage < 2 {
            m.slab(v(x, 4.45, -2.65), v(1.25, 3.87, 0.16), HEAT);
            m.slab(v(x, 4.72, -2.79), v(1.42, 0.28, 0.18), IRON);
        }
    }
    let riser = v(9.8, boiler_height, 0.1);
    let elbow = v(9.8, height + 0.4, 0.1);
    m.tube(riser, elbow, 0.61, 0.61, STEEL);
    if stage < 3 {
        m.tube(elbow, elbow - Vec3::X * 6.8, 0.62, 0.62, STEEL);
    }
    for t in [0.12, 0.47, 0.84] {
        m.stack(riser.lerp(elbow, t), 0.46, 0.86, 0.86, COPPER);
    }
    let walk = v(8.25, boiler_height + 0.35, -2.75);
    m.slab(walk, v(8.9, 0.33, 1.46), STEEL);
    m.rails(walk - v(4.4, -0.2, 0.61), walk + v(4.4, 0.2, -0.61), 4);
    // The engine house and its open flywheel sit clear of the striking bed.
    m.slab(v(-9.2, 2.6, -0.8), v(5.8, 5.2, 8.), OXIDE);
    let center = v(-9.2, 6., -5.);
    let ring = |angle: f32| center + v(angle.cos() * 3.4, angle.sin() * 3.4, 0.);
    for i in 0..12 {
        let angle = TAU * i as f32 / 12.;
        let rim = ring(angle);
        m.girder(rim, ring(angle + TAU / 12.), 0.56, COPPER);
        if i % 2 == 0 {
            m.girder(center, rim, 0.3, IRON);
        }
    }
}

fn hammer(m: &mut Works) {
    let v = Vec3::new;
    m.stack(v(0., 0., 0.), 2.2, 3.1, 3.6, IRON);
    m.stack(v(0., 2.2, 0.), 4., 3.45, 3.45, STEEL);
    m.stack(v(0., 5.75, 0.), 0.72, 3.68, 3.68, COPPER);
    m.stack(v(0., 6.45, 0.), 10.4, 1.45, 1.45, STRIPE);
    m.slab(v(0., 0.42, 0.), v(5.8, 0.85, 5.8), CHAR);
}

/// Glass, furnace mouths and the forging bed get their own luminous material.
/// Iron bars and brick surrounds stay dark in the same openings.
fn furnace_light(m: &mut Works, kind: IndustrialYard, stage: u8) {
    let v = Vec3::new;
    match kind {
        IndustrialYard::RedShift => {
            for (bay, falls) in MILL_BAYS.into_iter().enumerate() {
                let x = (bay as f32 - 2.5) * 9.;
                if stage >= falls {
                    continue;
                }
                for side in [-1., 1.] {
                    for col in [-1., 1.] {
                        m.slab(
                            v(x + col * 1.08, 2.62, side * 9.10),
                            v(1.82, 2.75, 0.16),
                            HEAT,
                        );
                        m.slab(
                            v(x + col * 1.08, 5.25, side * 9.10),
                            v(1.82, 1.74, 0.16),
                            FURNACE,
                        );
                    }
                    for col in [-2.42, 0., 2.42] {
                        m.slab(v(x + col, 10.72, side * 9.02), v(2.03, 2.18, 0.16), HEAT);
                    }
                }
            }
            // Visible furnace doors in the end walls when the approach is
            // alongshore and the long window rows are nearly edge-on.
            for side in [-1., 1.] {
                if stage >= MILL_BAYS[if side < 0. { 0 } else { 5 }] {
                    continue;
                }
                for z in [-4.9, 0., 4.9] {
                    m.slab(v(27.08 * side, 5.0, z), v(0.20, 5.3, 3.25), HEAT);
                }
            }
        }
        IndustrialYard::BellWorks => {
            let embers = stage >= 2;
            m.slab(
                v(0., 3.39, 0.),
                v(if embers { 7. } else { 14.7 }, 0.18, 11.9),
                if embers { OXIDE } else { HEAT },
            );
            // The long hot billet runs through the die. Its white centre
            // marks the strike, with cooler ends near the two governor bays.
            if !embers {
                m.slab(v(0., 3.51, -1.3), v(15.8, 0.18, 5.6), FURNACE);
                m.slab(v(0., 3.63, -1.3), v(6.4, 0.12, 5.4), ENAMEL);
            }
            for side in [-1., 1.] {
                for x in [-5., 0., 5.] {
                    m.slab(
                        v(x, 2.04, side * 6.55),
                        v(4.66, if embers { 0.39 } else { 1.92 }, 0.21),
                        if !embers && x == 0. { FURNACE } else { HEAT },
                    );
                }
            }
            if !embers {
                for x in [5.65, 8.2, 10.75] {
                    m.slab(v(x, 3.56, -2.78), v(1.18, 1.79, 0.13), FURNACE);
                    m.slab(v(x, 5.56, -2.78), v(1.18, 1.37, 0.13), HEAT);
                }
                m.stack(v(0., 19.19, 0.), 0.42, 3.01, 3.01, HEAT);
            }
        }
    }
}

fn governor(m: &mut Works, ruined: bool) {
    let v = Vec3::new;
    m.slab(v(0., 0.54, 0.), v(5.7, 1.08, 4.8), CONCRETE);
    if ruined {
        m.block(
            v(0.5, 1.65, -0.3),
            v(3.7, 1.18, 2.7),
            Quat::from_rotation_z(-0.23),
            CHAR,
        );
        m.girder(v(-2.05, 1., 1.5), v(-1.7, 4., 1.), 0.52, COPPER);
        return;
    }
    // The white enamel disc and black needle repeat at both yards.
    m.slab(v(0., 3.3, 0.45), v(3.96, 5.3, 2.7), IRON);
    m.tube(v(0., 5.85, 1.0), v(0., 5.85, -1.39), 2.56, 2.56, COPPER);
    m.tube(v(0., 5.85, -1.4), v(0., 5.85, -1.55), 2.19, 2.19, ENAMEL);
    m.tube(v(0., 5.85, -1.56), v(0., 5.85, -1.72), 0.29, 0.29, IRON);
    m.girder(v(0., 5.85, -1.70), v(1.08, 7.23, -1.70), 0.21, CHAR);
    for i in 0..7 {
        let angle = (i as f32 / 6. - 0.5) * PI * 1.42;
        let at = v(angle.sin(), angle.cos(), 0.);
        let center = v(0., 5.85, -1.62);
        m.girder(center + at * 1.63, center + at * 1.94, 0.11, IRON);
    }
    m.slab(v(0., 2.29, -1.), v(2.75, 0.76, 0.62), FURNACE);
}

fn fuel(m: &mut Works, ruined: bool) {
    let v = Vec3::new;
    m.stack(v(0., -0.5, 0.), 1., 6.25, 6.25, CONCRETE);
    if ruined {
        m.stack(v(0., 0.5, 0.), 0.61, 5.9, 5.78, CHAR);
        for i in 0..10 {
            let ang = i as f32 * TAU / 10.;
            let top = 1.65 + ((i * 7) % 4) as f32 * 0.5;
            m.block(
                v(ang.sin() * 5.1, top * 0.5 + 0.5, ang.cos() * 5.1),
                v(3.12, top, 0.28),
                Quat::from_rotation_y(ang) * Quat::from_rotation_x(0.22 * ((i % 3) as f32 - 1.)),
                ASH,
            );
        }
        return;
    }
    m.stack(v(0., 0.5, 0.), 9.2, 5.86, 5.86, ENAMEL);
    for (y, h, r, col) in [
        (1.2, 0.35, 5.95, IRON),
        (3.15, 2.25, 5.92, OXIDE),
        (6.72, 0.23, 5.92, STEEL),
        (9.57, 0.4, 6.05, IRON),
    ] {
        m.stack(v(0., y, 0.), h, r, r, col);
    }
    m.stack(v(0., 9.9, 0.), 1.4, 5.88, 1.22, ROOF);
    m.stack(v(0., 11.23, 0.), 0.78, 1.2, 1.12, STEEL);
    for x in [-0.54, 0.54] {
        m.girder(v(x, 0.59, -5.99), v(x, 10.66, -5.99), 0.16, IRON);
    }
    for i in 0..14 {
        let y = 0.86 + i as f32 * 0.69;
        m.girder(v(-0.54, y, -6.), v(0.54, y, -6.), 0.12, IRON);
    }
}

fn casemate(m: &mut Works) {
    let v = Vec3::new;
    // Put the portal behind the extended gun. The gun carriage can run through
    // its actual opening and the raised shield disappears behind the lintel.
    for side in [-1., 1.] {
        m.slab(v(side * 8.2, 7.1, 7.), v(3.1, 14.2, 10.6), CONCRETE);
        m.slab(v(side * 6.5, 7.2, 2.95), v(0.64, 12.5, 1.3), IRON);
    }
    m.slab(v(0., 14.6, 7.4), v(20.4, 3.3, 12.4), ASH);
    m.slab(v(0., 1.15, 6.4), v(17.7, 2.3, 11.8), CONCRETE);
    m.slab(v(0., 7.4, 12.15), v(15.7, 12., 0.7), COAL);
    for side in [-1., 1.] {
        m.slab(v(side * 6.83, 11.6, 1.48), v(0.65, 0.84, 0.64), FURNACE);
    }
}

fn shutter(m: &mut Works) {
    let v = Vec3::new;
    m.slab(v(0., 7.56, 3.0), v(13.1, 11.1, 0.77), IRON);
    for i in 0..7 {
        m.slab(
            v(0., 2.95 + i as f32 * 1.54, 2.54),
            v(12.8, 1.39, 0.46),
            if i % 2 == 0 { STEEL } else { TIN },
        );
    }
    m.slab(v(0., 2.2, 2.32), v(12.8, 0.36, 0.58), COPPER);
}

fn quay(m: &mut Works) {
    let v = Vec3::new;
    // Back the existing travelling crane shoes with concrete batters, seaward
    // stairs and a dark tide line. The central basin and its launch stay open.
    for side in [-1., 1.] {
        for (x, y, z, w, h, d, col) in [
            (5.1, -1.32, -4., 3.35, 4., 21., CONCRETE),
            (5.1, -1.8, -14., 3.35, 3.1, 3.8, ASH),
            (5.1, -2.35, -17.5, 3.35, 2.4, 3.8, ASH),
            (5.1, -2.70, -20.6, 3.10, 2.0, 2.8, ASH),
            (5.12, 0.73, -3.45, 2.88, 0.28, 19.9, CONCRETE),
        ] {
            m.slab(v(side * x, y, z), v(w, h, d), col);
        }
        for i in 0..6 {
            let z = -11.9 + i as f32 * 3.25;
            m.slab(v(side * 6.83, -0.91, z), v(0.67, 3.3, 1.29), IRON);
            m.stack(v(side * 5.9, 0.91, z), 0.62, 0.32, 0.39, CHAR);
        }
    }
}

/// The companion graving dock winds in plate with a steam capstan and one
/// shear leg, leaving the travelling portal crane alone on the skyline.
fn winch_house(m: &mut Works, state: YardState) {
    let v = Vec3::new;
    let hoist = flag_hoist(state);
    let ruined = state == YardState::Ruined;
    let top = if ruined { 1.14 } else { hoist.y.min(6.2) };
    m.slab(v(-5.12, 0.46, 0.), v(3.54, 0.79, 10.6), CONCRETE);
    m.slab(
        v(-5.12, top * 0.5, 2.0),
        v(3.28, top, 6.38),
        if ruined { CHAR } else { BRICK },
    );
    if ruined {
        m.block(
            v(-4.9, 0.94, 1.61),
            v(3.5, 0.28, 6.5),
            Quat::from_rotation_z(0.17),
            IRON,
        );
        return;
    }
    m.slab(v(-5.12, top, 2.), v(3.50, 0.25, 6.59), ROOF);
    for side in [-1., 1.] {
        for z in [0.1, 2.05, 4.] {
            m.slab(
                v(-5.12 + side * 1.67, top * 0.68, z),
                v(0.11, top * 0.24, 1.35),
                GLAZING,
            );
        }
    }
    m.slab(v(-5.12, 0.75, -3.83), v(3.58, 1.1, 3.4), IRON);
    let drum = v(-5.12, 2.05, -3.75);
    m.tube(drum - Vec3::X, drum + Vec3::X, 0.99, 0.99, BRICK);
    for side in [-1., 1.] {
        m.tube(
            drum + Vec3::X * (side * 0.93),
            drum + Vec3::X * (side * 1.35),
            1.17,
            1.17,
            COPPER,
        );
    }
    let heel = v(-5.2, 0.9, -2.34);
    m.girder(heel, hoist, 0.83, STEEL);
    m.girder(v(-6.15, top, 4.2), hoist, 0.52, OCHRE);
    let hook = hoist + v(3.2, -1.4, -3.);
    m.girder(hoist, hook, 0.57, OCHRE);
    m.girder(heel, hook, 0.35, STEEL);
    m.girder(hook, hook - Vec3::Y * 1.8, 0.15, IRON);
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::camera::primitives::MeshAabb;

    #[test]
    fn the_weak_spots_reach_their_target_roofs_and_leave_lower_ruins() {
        let top = |part| {
            industrial(IndustrialYard::RedShift, part)
                .get_aabb()
                .unwrap()
                .max()
                .y
                + 3.2
        };
        for (live, ruined, roof) in [
            (
                IndustrialPart::Governor(false),
                IndustrialPart::Governor(true),
                12.,
            ),
            (
                IndustrialPart::Fuel(false),
                IndustrialPart::Fuel(true),
                15.2,
            ),
        ] {
            assert!((top(live) - roof).abs() < 0.5);
            assert!(top(ruined) < roof - 4.);
        }
    }

    #[test]
    fn the_loaded_end_tips_into_the_mill_and_the_hammer_punches_through_the_bed() {
        let mill = |t| machine_pose(IndustrialYard::RedShift, t, 0.);
        let loaded_end = Vec3::new(-29., 0., 0.);
        assert!(mill(0.).transform_point(loaded_end).y > 17.);
        assert!(mill(1.).transform_point(loaded_end).y < 0.);

        let forge = |t, clock| {
            machine_pose(IndustrialYard::BellWorks, t, clock)
                .translation
                .y
        };
        assert!(forge(0., 0.72 * 3.4) > 14.);
        assert!((forge(0., 0.95 * 3.4) - 3.25).abs() < 0.01);
        assert!(forge(1., 0.) < 2.);
    }
}
