//! The quarried shelf, its intertidal rock and the scrub that grows above the spray line.
use super::harbour_models::Harbour;
use super::surfaces::{self, hash, sample, tile};
use crate::scene::Facets;
use bevy::{
    asset::RenderAssetUsages,
    image::ImageSampler,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use shipvivor_encounter::{ISLAND_RADIUS, LandmarkKind};
use std::f32::consts::TAU;

#[derive(Clone, Copy, Hash, Eq, PartialEq)]
pub(super) enum Coast {
    Yard(Harbour),
    Landmark(LandmarkKind),
}

impl Coast {
    fn foundry(self) -> bool {
        self == Self::Yard(Harbour::Foundry)
    }

    fn seed(self) -> u32 {
        match self {
            Self::Yard(Harbour::Cannery) => 11,
            Self::Yard(Harbour::TwinMills) => 37,
            Self::Yard(Harbour::Foundry) => 53,
            Self::Landmark(LandmarkKind::Beacon) => 73,
            Self::Landmark(LandmarkKind::Pines) => 89,
            Self::Landmark(LandmarkKind::Stacks) => 107,
            Self::Landmark(LandmarkKind::Glacier) => 139,
            Self::Landmark(LandmarkKind::Mainland) => 151,
        }
    }

    /// The level yard apron and the bluff behind it follow the two land volumes on the chart.
    fn crest(self, p: Vec2) -> f32 {
        if self == Self::Landmark(LandmarkKind::Glacier) {
            // The ice cap rises to the high ground the lookouts see across the water. On
            // one side its glacier runs down to the sea and ends in a cliff of ice.
            let tongue = (1. - (p.y.atan2(p.x) / 0.6).powi(2)).max(0.);
            let r = p.length() / (ISLAND_RADIUS * (0.74 + 0.2 * tongue));
            return 26.8 * (1. - r * r).max(0.).sqrt();
        }
        if matches!(self, Self::Landmark(_)) {
            return 0.;
        }
        let q = (p - Vec2::new(-5., 7.)) / Vec2::new(12.7, 10.2);
        let saddle = (1. - q.length_squared()).max(0.);
        let bench = 0.5 + 0.5 * (p.x * 0.83 - p.y * 0.33).sin();
        saddle * (5.05 + 1.55 * bench)
    }

    fn radius(self, a: f32) -> f32 {
        let shift = self.seed() as f32 * 0.28;
        ISLAND_RADIUS
            * (0.923
                + 0.055 * (3. * a + shift).cos()
                + 0.036 * (7. * a - shift).sin()
                + 0.021 * (13. * a + shift).cos()
                + 0.014 * (21. * a + 1.4).sin())
    }

    fn height(self, p: Vec2) -> f32 {
        let a = p.y.atan2(p.x);
        let r = p.length() / self.radius(a);
        let ledge = (0.80 + 0.052 * (4. * a + 0.9).sin()) - 0.16 * a.cos().max(0.).powi(6);
        let slope = ((r - ledge) / (1. - ledge)).max(0.);
        3.13 + self.crest(p) - 3.42 * slope.powf(0.79)
    }
}

fn noise(uv: Vec2, cells: i32, seed: u32) -> f32 {
    surfaces::grain(uv, IVec2::splat(cells), seed)
}

// One repeating mineral texture. The mesh supplies the much larger grass, lichen and tide marks.
fn granite() -> Image {
    const SIDE: u32 = 192;
    let mut pixels = Vec::with_capacity((SIDE * SIDE * 4) as usize);
    for y in 0..SIDE {
        for x in 0..SIDE {
            let uv = Vec2::new(x as f32, y as f32) / SIDE as f32;
            let warp = noise(uv, 5, 17);
            let vein = (TAU * (uv.x * 2. + uv.y * 3. + warp * 1.1)).sin().abs();
            let aggregate = 0.55 * noise(uv, 14, 7) + 0.45 * noise(uv, 39, 3);
            let quartz = ((0.19 - vein) * 5.).clamp(0., 1.);
            let fissure = ((0.075 - (vein - 0.29).abs()) * 13.).clamp(0., 1.);
            let stone =
                167. + 70. * aggregate + 20. * sample(x, y, 83) + 24. * quartz - 32. * fissure;
            pixels.extend([
                stone as u8,
                (stone * 0.995) as u8,
                (stone * 0.975) as u8,
                255,
            ]);
        }
    }
    tile(SIDE, pixels)
}

pub(super) fn material(images: &mut Assets<Image>) -> StandardMaterial {
    StandardMaterial {
        base_color_texture: Some(images.add(granite())),
        perceptual_roughness: 0.89,
        reflectance: 0.23,
        emissive: LinearRgba::rgb(0.030, 0.034, 0.032),
        ..default()
    }
}

// Edges of the foundry's dredged slot in the island frame.
const PORT_WALL: f32 = 2.7;
const STARBOARD_WALL: f32 = 9.3;
const END_WALL: f32 = 1.4;

fn inside_berth(p: Vec2) -> bool {
    (PORT_WALL..=STARBOARD_WALL).contains(&p.x) && p.y <= END_WALL
}

fn clip(poly: &[Vec3], distance: impl Fn(Vec3) -> f32) -> Vec<Vec3> {
    let mut result = Vec::with_capacity(poly.len() + 1);
    let Some(&last) = poly.last() else {
        return result;
    };
    let mut prev = last;
    let mut d0 = distance(prev);
    for &next in poly {
        let d1 = distance(next);
        if (d0 < 0.) != (d1 < 0.) {
            result.push(prev.lerp(next, d0 / (d0 - d1)));
        }
        if d1 >= 0. {
            result.push(next);
        }
        prev = next;
        d0 = d1;
    }
    result
}

struct Outcrop {
    facets: Facets,
    berth: bool,
}

impl Outcrop {
    fn face(&mut self, v: [Vec3; 3], colour: Color) {
        if !self.berth
            || v.iter().all(|p| p.x <= PORT_WALL)
            || v.iter().all(|p| p.x >= STARBOARD_WALL)
            || v.iter().all(|p| p.z >= END_WALL)
        {
            self.facets.triangle(v, colour);
            return;
        }
        if v.iter().all(|p| inside_berth(p.xz())) {
            return;
        }
        let left = clip(&v, |p| PORT_WALL - p.x);
        let right = clip(&v, |p| p.x - STARBOARD_WALL);
        let strip = clip(&clip(&v, |p| p.x - PORT_WALL), |p| STARBOARD_WALL - p.x);
        for part in [left, right, clip(&strip, |p| p.z - END_WALL)] {
            for i in 1..part.len().saturating_sub(1) {
                let [a, b, c] = [part[0], part[i], part[i + 1]];
                if (b - a).cross(c - a).length_squared() > 1e-10 {
                    self.facets.triangle([a, b, c], colour);
                }
            }
        }
    }

    /// Broad slabs with offset plates and two exposed fracture faces, bedded into the bank.
    fn slab(&mut self, foot: Vec3, size: Vec3, angle: f32, seed: u32) {
        let rotate = Quat::from_rotation_y(angle);
        let top = foot + Vec3::Y * size.y;
        let ring = |i: u32| {
            let i = i % 7;
            let a = (i as f32 + 0.22 * sample(i, 0, seed)) * TAU / 7.;
            let ray = Vec3::new(a.cos() * size.x, 0., a.sin() * size.z)
                * (0.78 + 0.28 * sample(i, 1, seed));
            let chip = 0.56 + 0.27 * sample(i, 2, seed);
            [
                foot + rotate * ray * 0.92,
                foot + rotate * ray + Vec3::Y * size.y * chip,
                top + rotate * (ray * 0.68 + Vec3::X * size.x * 0.11)
                    - Vec3::Y * size.y * (0.12 + 0.14 * a.cos()),
            ]
        };
        // Snow lies on every top above the spray. A few carry only lichen.
        let cap = |k: f32| {
            if foot.y < 0.4 {
                tint([128, 141, 135], 0.91 + 0.13 * k)
            } else if sample(seed, 9, 6) > 0.82 {
                tint([178, 176, 134], 0.91 + 0.13 * k)
            } else {
                bright(SNOW, 0.93 + 0.07 * k)
            }
        };
        for i in 0..7 {
            let a = ring(i);
            let b = ring(i + 1);
            let k = sample(i, 4, seed);
            let dark = tint([98, 105, 104], 0.80 + 0.32 * k);
            let rock = tint([159, 157, 147], 0.83 + 0.29 * k);
            self.face([a[0], a[1], b[0]], dark);
            self.face([b[0], a[1], b[1]], dark);
            self.face([a[1], a[2], b[1]], rock);
            self.face([b[1], a[2], b[2]], rock);
            self.face([a[2], top, b[2]], cap(k));
        }
    }

    fn limb(&mut self, a: Vec3, b: Vec3, base: f32, tip: f32) {
        let axis = (b - a).normalize();
        let side = axis.any_orthonormal_vector();
        let up = axis.cross(side);
        for i in 0..5 {
            let radial = |i| {
                let (s, c) = (i as f32 * TAU / 5.).sin_cos();
                side * c + up * s
            };
            let u = radial(i);
            let v = radial(i + 1);
            let col = tint([116, 98, 79], 0.78 + 0.10 * (i % 3) as f32);
            self.face([a + u * base, a + v * base, b + u * tip], col);
            self.face([a + v * base, b + v * tip, b + u * tip], col);
        }
    }

    fn pine(&mut self, root: Vec3, height: f32, seed: u32) {
        let along = |u: f32| root + height * Vec3::new(0.13 * u * u, u, -0.047 * u * u);
        self.limb(root, along(0.62), 0.042 * height, 0.024 * height);
        self.limb(along(0.62), along(1.), 0.024 * height, 0.007 * height);
        for tier in 0..6 {
            let u = 0.33 + tier as f32 * 0.113;
            for j in 0..6 {
                let a = (j as f32 + sample(tier, j, seed) * 0.62) * TAU / 6. + tier as f32 * 1.83;
                let (s, c) = a.sin_cos();
                let dir = Vec3::new(c, 0., s);
                let side = Vec3::new(-s, 0., c);
                let reach = height
                    * (1.06 - u)
                    * (0.31 + 0.16 * sample(j, tier, seed + 1))
                    * (1. + 0.17 * c);
                let base = along(u);
                let elbow = base + reach * (dir * 0.63 - Vec3::Y * 0.14);
                let tip = base + reach * dir + Vec3::Y * reach * 0.06;
                let crown = base + Vec3::Y * (0.09 * height);
                // Most boughs carry snow on their upper faces.
                let colour = if sample(j, tier, seed + 5) > 0.3 {
                    bright(SNOW, 0.85 + 0.1 * sample(tier, j, seed + 2))
                } else {
                    tint([73, 99, 72], 0.77 + 0.37 * sample(tier, j, seed + 2))
                };
                let left = elbow - side * reach * 0.33;
                let right = elbow + side * reach * 0.33;
                self.face([crown, right, tip], colour);
                self.face([crown, tip, left], colour);
                self.face([base, left, tip], tint([48, 70, 55], 0.94));
                self.face([base, tip, right], tint([48, 70, 55], 0.94));
                if tier == 0 {
                    self.limb(base, elbow, 0.018 * height, 0.007 * height);
                }
            }
        }
    }
}

pub(super) fn tint(rgb: [u8; 3], gain: f32) -> Color {
    let [r, g, b] = rgb.map(|v| (f32::from(v) * gain).min(255.) as u8);
    Color::srgb_u8(r, g, b)
}

/// Fresh snow, trodden snow and glacier ice, in linear light. The granite texture under
/// every coast takes about a third off whatever it covers, and snow lies on slopes
/// turned from the sun, so these are set well above white to come out white.
pub(super) const SNOW: [f32; 3] = [1.85, 1.9, 1.98];
const TRODDEN: [f32; 3] = [1.2, 1.25, 1.32];
const BLUE_ICE: [f32; 3] = [0.72, 1.35, 1.8];

pub(super) fn bright(rgb: [f32; 3], gain: f32) -> Color {
    Color::linear_rgb(rgb[0] * gain, rgb[1] * gain, rgb[2] * gain)
}

pub(super) fn mesh(coast: Coast) -> Mesh {
    let mut out = Outcrop {
        facets: Facets::default(),
        berth: coast.foundry(),
    };
    let seed = coast.seed();
    const N: u32 = 64;
    const RADII: [f32; 10] = [0.12, 0.26, 0.40, 0.55, 0.66, 0.76, 0.84, 0.91, 1., 1.065];
    let vertex = |ring: usize, i: u32| {
        let i = i % N;
        let a = (i as f32 + 0.57 * (sample(i, ring as u32, seed) - 0.5)) * TAU / N as f32;
        let r = coast.radius(a) * (RADII[ring] + 0.022 * (sample(ring as u32, i, seed) - 0.5));
        let p = Vec2::from_angle(a) * r;
        p.extend(coast.height(p)).xzy()
    };
    let mut ground = |verts: [Vec3; 3]| {
        let p = (verts[0] + verts[1] + verts[2]) / 3.;
        let inland = p.xz() / 27.;
        let patch = noise(inland, 4, seed);
        let face = (verts[1] - verts[0])
            .cross(verts[2] - verts[0])
            .normalize_or_zero();
        // Arctic ground: dark wet shingle, bare rock on the slopes and snow over the
        // level ground, broken by patches of tundra. The yards' snow is trodden grey.
        // A glacier's mesh is stretched across the chart, so its slopes look gentler in
        // the world than in these coordinates.
        let grain = noise(inland, 11, seed + 1);
        let (rock, snow) = (0.88 + 0.21 * grain, 0.94 + 0.08 * grain);
        let colour = if coast == Coast::Landmark(LandmarkKind::Glacier) && p.y > 3.3 {
            bright(if face.y < 0.5 { BLUE_ICE } else { SNOW }, snow)
        } else if p.y < 0.95 + 0.32 * patch {
            tint([84, 94, 98], rock)
        } else if p.y < 2.7 || face.y < 0.82 || (p.y > 3.5 && patch < 0.54) {
            tint([158, 160, 158], rock)
        } else if patch > 0.66 {
            tint([128, 124, 98], rock)
        } else if matches!(coast, Coast::Yard(_)) {
            bright(TRODDEN, snow)
        } else {
            bright(SNOW, snow)
        };
        out.face(verts, colour);
    };
    for i in 0..N {
        ground([
            Vec3::Y * coast.height(Vec2::ZERO),
            vertex(0, i + 1),
            vertex(0, i),
        ]);
        for r in 1..RADII.len() {
            let [a, b, c, d] = [
                vertex(r - 1, i),
                vertex(r - 1, i + 1),
                vertex(r, i + 1),
                vertex(r, i),
            ];
            ground([a, b, d]);
            ground([d, b, c]);
        }
    }
    // Weathered ribs of bedrock, with smaller spalls collected down-slope in patches.
    for i in 0..165 {
        let a = TAU * sample(i, 0, seed);
        let q = 0.83 + 0.20 * sample(i, 1, seed);
        let p = Vec2::from_angle(a) * coast.radius(a) * q;
        let h = coast.height(p);
        let shoal = 0.53 + 0.47 * (3. * a + seed as f32).cos();
        if sample(i, 2, seed) > 0.30 + 0.67 * shoal || h > 2.7 {
            continue;
        }
        let scale = 0.45 + 2.9 * sample(i, 3, seed).powi(2);
        out.slab(
            p.extend(h - 0.28).xzy(),
            Vec3::new(scale, (0.28 + scale * 0.52).min(3.25 - h), scale * 0.66),
            0.58 + 0.55 * sample(i, 4, seed),
            hash(seed + i),
        );
    }
    match coast {
        Coast::Yard(_) => {
            for i in 0..40 {
                let p = Vec2::new(-5., 7.)
                    + Vec2::new(
                        (sample(i, 0, seed) - 0.5) * 22.,
                        (sample(i, 1, seed) - 0.5) * 17.,
                    );
                if coast.crest(p) < 0.95 {
                    continue;
                }
                let h = coast.height(p);
                let size = 0.8 + 2.5 * sample(i, 2, seed);
                out.slab(
                    p.extend(h - 0.63).xzy(),
                    Vec3::new(size, 1.25, size * 0.62),
                    0.68,
                    hash(seed + i),
                );
            }
            for (i, (x, z, h)) in [
                (-10., 6., 6.7),
                (-11.9, 8.8, 4.6),
                (-7.2, 10., 5.4),
                (-5., 11.3, 3.8),
                (0.4, 11., 4.6),
            ]
            .into_iter()
            .enumerate()
            {
                let p = Vec2::new(x, z);
                out.pine(p.extend(coast.height(p) - 0.18).xzy(), h, seed + i as u32);
            }
        }
        Coast::Landmark(LandmarkKind::Pines) => {
            for i in 0..19 {
                let p =
                    Vec2::from_angle(sample(i, 0, seed) * TAU) * (11. * sample(i, 1, seed).sqrt());
                out.pine(
                    p.extend(3.1).xzy(),
                    4.3 + 4.65 * sample(i, 2, seed),
                    seed + i,
                );
            }
        }
        Coast::Landmark(LandmarkKind::Stacks) => {
            for (i, (x, z, h, w)) in [
                (0., 0., 19.6, 4.9),
                (-6.3, 1.7, 11.8, 3.3),
                (5., -4.8, 6.8, 3.3),
            ]
            .into_iter()
            .enumerate()
            {
                for layer in 0..4 {
                    let u = layer as f32 / 4.;
                    let foot = Vec3::new(x + 0.38 * u, 2.2 + u * h, z - 0.8 * u);
                    out.slab(
                        foot,
                        Vec3::new(
                            w * (1. - 0.56 * u),
                            h * (0.37 - 0.10 * u),
                            w * (0.87 - 0.50 * u),
                        ),
                        0.37,
                        seed + i as u32,
                    );
                }
            }
        }
        Coast::Landmark(LandmarkKind::Beacon | LandmarkKind::Glacier | LandmarkKind::Mainland) => {}
    }
    surfaces::weathered_mesh(out.facets, 5.6)
}

/// Bake the same island profiles at chart positions once per passage. The water samples this
/// elevation to break on the actual shingle bank as the swell rises and falls.
pub(crate) fn shoals(battle: &shipvivor_encounter::Battle) -> (Image, Vec4) {
    const METRES: f32 = 1.25;
    let sites: Vec<_> = battle
        .ports()
        .iter()
        .map(|p| {
            (
                Coast::Yard(Harbour::for_factories(p.factories().len())),
                p.body,
                ISLAND_RADIUS,
            )
        })
        .chain(
            battle
                .landmarks()
                .iter()
                .map(|p| (Coast::Landmark(p.kind), p.body, p.radius)),
        )
        .collect();
    let mainland = Coast::Landmark(LandmarkKind::Mainland);
    // How far out from a piece of the mainland the surf chart reaches, and how far in.
    let (offshore, inshore) = (30., 12.);
    let reach = |coast: Coast, radius: f32| {
        if coast == mainland {
            radius + offshore
        } else {
            radius * 1.55
        }
    };
    let mut lo = Vec2::splat(f32::INFINITY);
    let mut hi = Vec2::splat(f32::NEG_INFINITY);
    for &(coast, body, radius) in &sites {
        lo = lo.min(body.pos - reach(coast, radius) - METRES * 4.);
        hi = hi.max(body.pos + reach(coast, radius) + METRES * 4.);
    }
    let extent = ((hi - lo) / METRES).ceil().as_uvec2();
    let mut texels = vec![0; (extent.x * extent.y) as usize];
    let mut chart = |x: u32, z: u32, height: f32| {
        let pixel = &mut texels[(z * extent.x + x) as usize];
        *pixel = (*pixel).max(((height + 8.) * (255. / 12.)) as u8);
    };
    for (coast, body, radius) in sites {
        let center = (body.pos - lo) / METRES;
        let pad = reach(coast, radius) / METRES;
        let first = (center - pad).floor().max(Vec2::ZERO).as_uvec2();
        let end = (center + pad).ceil().as_uvec2().min(extent);
        let at = |x: u32, z: u32| lo + (Vec2::new(x as f32, z as f32) + 0.5) * METRES;
        if coast == mainland {
            // A piece of the mainland is charted only in a ring round its shore, row by
            // row, so its broad hinterland costs nothing.
            let (outer, inner) = ((radius + offshore) / METRES, (radius - inshore) / METRES);
            for z in first.y..end.y {
                let dz = z as f32 + 0.5 - center.y;
                if dz.abs() > outer {
                    continue;
                }
                let wide = (outer * outer - dz * dz).sqrt();
                let narrow = (inner * inner - dz * dz).max(0.).sqrt();
                let span = |from: f32, to: f32| {
                    let from = (center.x + from).floor().max(0.) as u32;
                    let to = ((center.x + to).ceil().max(0.) as u32).min(extent.x);
                    from..to
                };
                for x in span(-wide, -narrow).chain(span(narrow, wide)) {
                    let inside = radius - at(x, z).distance(body.pos);
                    if (-offshore..inshore).contains(&inside) {
                        chart(x, z, super::mainland::profile(radius, inside));
                    }
                }
            }
            continue;
        }
        let to_local = Mat2::from_angle(-body.heading) * (ISLAND_RADIUS / radius);
        for z in first.y..end.y {
            for x in first.x..end.x {
                let local = to_local * (at(x, z) - body.pos);
                let height = if coast.foundry() && inside_berth(local) {
                    -5.
                } else {
                    coast.height(local)
                };
                chart(x, z, height);
            }
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: extent.x,
            height: extent.y,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        texels,
        TextureFormat::R8Unorm,
        RenderAssetUsages::default(),
    );
    image.sampler = ImageSampler::linear();
    let uv_scale = (extent.as_vec2() * METRES).recip();
    (image, Vec4::new(lo.x, lo.y, uv_scale.x, uv_scale.y))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_foundry_berth_is_open_in_the_land_and_the_surf_chart() {
        let shore = mesh(Coast::Yard(Harbour::Foundry));
        let points = shore
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .unwrap()
            .as_float3()
            .unwrap();
        // Clipping splits each triangle at a wall. Every surviving face must lie outside one wall.
        for face in points.as_chunks::<3>().0 {
            assert!(
                face.iter().all(|p| p[0] <= PORT_WALL + 0.001)
                    || face.iter().all(|p| p[0] >= STARBOARD_WALL - 0.001)
                    || face.iter().all(|p| p[2] >= END_WALL - 0.001)
            );
        }

        let battle = crate::state::Encounter::default();
        let foundry = &battle.ports()[2];
        let (map, chart) = shoals(&battle);
        let depth_sample = |across: f32| {
            let front = foundry.body.forward();
            let world = foundry.body.pos + front * 6. + front.perp() * across;
            let size = map.size();
            let pixel = ((world - chart.xy()) * chart.zw() * size.as_vec2()).as_uvec2();
            map.data.as_ref().unwrap()[(pixel.y * size.x + pixel.x) as usize] as f32 * (12. / 255.)
                - 8.
        };
        assert!(depth_sample(6.) < -4.5);
        assert!(depth_sample(0.) > 2.);
    }
}
