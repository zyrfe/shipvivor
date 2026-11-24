//! Breakwater's wheelhouse, bridge wings and signal rig. Each part is in ship coordinates.
use super::Facets;
use bevy::prelude::*;

fn ring(beam: f32, depth: f32, z: f32, cut: f32, y: f32) -> [Vec3; 8] {
    [
        (-beam + cut, -depth),
        (beam - cut, -depth),
        (beam, -depth + cut),
        (beam, depth - cut),
        (beam - cut, depth),
        (-beam + cut, depth),
        (-beam, depth - cut),
        (-beam, -depth + cut),
    ]
    .map(|(x, dz)| Vec3::new(x, y, z + dz))
}

fn tier(mesh: &mut Facets, bottom: [Vec3; 8], top: [Vec3; 8], wall: Color, cap: Color) {
    let center = (top[0] + top[4]) * 0.5;
    for i in 0..8 {
        let j = (i + 1) % 8;
        mesh.quad([bottom[i], top[i], top[j], bottom[j]], wall);
        mesh.triangle([center, top[j], top[i]], cap);
    }
}

pub(super) fn wheelhouse() -> (Mesh, Mesh) {
    let mut paint = Facets::default();
    let mut glass = Facets::default();
    let rgb = Color::srgb_u8;
    let steel = rgb(110, 134, 140);
    let shade = rgb(68, 87, 96);
    let enamel = rgb(181, 196, 189);
    let trim = rgb(210, 210, 176);
    let iron = rgb(54, 65, 69);
    let z = -3.10;
    tier(
        &mut paint,
        ring(1.83, 2.97, -2.1, 0.43, 1.53),
        ring(1.72, 2.86, -2.1, 0.43, 2.95),
        steel,
        shade,
    );
    tier(
        &mut paint,
        ring(1.84, 2.19, z, 0.41, 2.93),
        ring(1.84, 2.19, z, 0.41, 3.04),
        trim,
        shade,
    );
    tier(
        &mut paint,
        ring(1.38, 1.37, z, 0.39, 3.03),
        ring(1.45, 1.40, z, 0.40, 3.76),
        enamel,
        shade,
    );
    let sill = ring(1.46, 1.41, z, 0.40, 3.75);
    let brow = ring(1.57, 1.52, z, 0.44, 4.39);
    tier(&mut paint, sill, brow, shade, shade);
    for i in 0..8 {
        let j = (i + 1) % 8;
        let outward = Vec3::Y.cross(sill[j] - sill[i]).normalize();
        let panel = |t: f32, h: f32, relief: f32| {
            sill[i].lerp(sill[j], t).lerp(brow[i].lerp(brow[j], t), h) + outward * relief
        };
        let panes = match i {
            0 | 4 => 4,
            2 | 6 => 3,
            _ => 1,
        };
        for k in 0..panes {
            let t = k as f32 / panes as f32;
            let end = (k + 1) as f32 / panes as f32;
            let margin = 0.036 / (sill[j] - sill[i]).length();
            let a = panel(t + margin, 0.075, 0.014);
            let b = panel(t + margin, 0.93, 0.014);
            let c = panel(end - margin, 0.93, 0.014);
            let d = panel(end - margin, 0.075, 0.014);
            let sky = rgb(102, 150, 165);
            let deep = rgb(30, 59, 69);
            glass.triangle_colors([a, b, c], [deep, sky, sky]);
            glass.triangle_colors([a, c, d], [deep, sky, deep]);
            paint.tube(
                panel(t, 0., 0.026),
                panel(t, 1., 0.026),
                0.030,
                0.030,
                enamel,
            );
        }
    }
    tier(
        &mut paint,
        ring(1.56, 1.53, z, 0.43, 4.38),
        ring(1.76, 1.71, z, 0.49, 4.48),
        shade,
        enamel,
    );
    tier(
        &mut paint,
        ring(1.76, 1.71, z, 0.49, 4.48),
        ring(1.73, 1.68, z, 0.49, 4.57),
        enamel,
        steel,
    );
    // Wing guards, bridge ventilators, and the lifebuoy on each exposed bulkhead.
    for s in [-1., 1.] {
        let wing = |h, dz| Vec3::new(s * 1.76, h, z + dz);
        for dz in [-1.65, -0.70, 0.25, 1.20, 1.70] {
            paint.tube(wing(3.04, dz), wing(3.54, dz), 0.025, 0.023, enamel);
        }
        for h in [3.28, 3.55] {
            paint.tube(wing(h, -1.65), wing(h, 1.70), 0.026, 0.026, enamel);
        }
        for i in 0..5 {
            let h = 2.12 + 0.105 * i as f32;
            paint.tube(
                Vec3::new(1.805 * s, h, -1.64),
                Vec3::new(1.805 * s, h, -0.84),
                0.042,
                0.042,
                shade,
            );
        }
        let buoy = |i: usize| {
            let (sin, cos) = (i as f32 * std::f32::consts::TAU / 20.).sin_cos();
            Vec3::new(s * 1.85, 2.42 + 0.285 * sin, -3.16 + 0.285 * cos)
        };
        for i in 0..20 {
            paint.tube(
                buoy(i),
                buoy(i + 1),
                0.073,
                0.073,
                if i % 5 == 0 { trim } else { rgb(188, 76, 48) },
            );
        }
    }
    // Shrouds run from the hounds to deck strongpoints. Halyards hang from the yard tips.
    let mast = |h| Vec3::new(0., h, -0.85);
    paint.tube(mast(2.90), mast(6.73), 0.152, 0.096, steel);
    paint.tube(mast(6.73), mast(8.16), 0.096, 0.048, iron);
    paint.tube(mast(6.54), mast(6.72), 0.19, 0.19, trim);
    for s in [-1., 1.] {
        let end = Vec3::new(1.68 * s, 6.71, -0.85);
        paint.tube(mast(6.65), end, 0.082, 0.044, steel);
        paint.tube(mast(7.57), end, 0.019, 0.019, iron);
        paint.tube(end, Vec3::new(1.39 * s, 3.1, -1.31), 0.016, 0.016, trim);
        paint.tube(
            mast(7.73),
            Vec3::new(1.68 * s, 2.95, 0.25),
            0.024,
            0.024,
            iron,
        );
    }
    paint.tube(mast(7.81), Vec3::new(0., 4.57, -4.47), 0.023, 0.023, iron);
    paint.tube(mast(8.12), mast(8.23), 0.072, 0.056, trim);
    (paint.finish(), glass.finish())
}
