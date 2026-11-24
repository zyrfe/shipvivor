//! A ship-length tile for Breakwater. The shell and teak use separate strips of the atlas.
//! The bottom strip is plain enamel for the fittings that carry their own vertex colours.
use crate::surfaces::{grain, mipmapped, sample};
use bevy::{prelude::*, render::render_resource::TextureFormat};

const WIDTH: u32 = 1024;
const HEIGHT: u32 = 256;
pub(super) const PLAIN: [f32; 2] = [0.5, 0.98];
pub(super) fn shell_uv(p: Vec3) -> [f32; 2] {
    [(p.z + 16.) / 32., (3.3 - p.y) / 8.]
}
pub(super) fn teak_uv(p: Vec3) -> [f32; 2] {
    [(p.z + 16.) / 32., 0.80 + p.x / 68.]
}

// Distance to a repeated line, with one fine edge catching the light.
fn seam(at: f32, pitch: f32, width: f32) -> f32 {
    let d = (at + pitch * 0.5).rem_euclid(pitch) - pitch * 0.5;
    (1. - d.abs() / width).clamp(0., 1.)
}

pub(super) fn hull_material(images: &mut Assets<Image>) -> StandardMaterial {
    let mut colour = Vec::with_capacity((WIDTH * HEIGHT * 4) as usize);
    let mut surface = Vec::with_capacity(colour.capacity());
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let uv = (Vec2::new(x as f32, y as f32) + 0.5) / Vec2::new(WIDTH as f32, HEIGHT as f32);
            let fore = uv.x * 32.;
            let grain_at = |nx, ny, seed| grain(uv, IVec2::new(nx, ny), seed);
            let (rgb, roughness) = if uv.y < 0.67 {
                let height = 3.3 - uv.y * 8.;
                let band = ((height + 2.) / 0.58).floor() as i32;
                let shift = (band.rem_euclid(3) as f32) * 1.21;
                let plate = ((fore + shift) / 4.).floor() as i32;
                let metal = sample(plate.rem_euclid(8) as u32, band as u32, 37);
                let lap = seam(height + 2., 0.58, 0.043);
                let butt = seam(fore + shift, 4., 0.047);
                let bevel = seam(height + 2. - 0.046, 0.58, 0.037);
                let rivets =
                    seam(height + 2. - 0.085, 0.58, 0.029) * seam(fore + shift, 0.16, 0.032);
                let salt = grain_at(29, 18, 19);
                let rain = grain_at(264, 7, 53);
                let wet = ((0.50 + 0.25 * grain_at(41, 8, 82) - height) * 3.).clamp(0., 1.);
                // Tight rusty runs beneath the hawse, and softer rain streaks across the plates.
                let hawse = 1. - ((fore - 3.4) / 0.58).abs().min(1.);
                let runoff = hawse * (2.6 - height).clamp(0., 1.) * (0.3 + rain * 0.7);
                let value = 205. + metal * 15. + salt * 17. + rain * 9. - lap * 33. - butt * 28.
                    + bevel * 16.
                    + rivets * 15.
                    - wet * 30.;
                (
                    [
                        value + runoff * 16.,
                        value - runoff * 31.,
                        value - runoff * 75.,
                    ],
                    0.66 - wet * 0.38 + salt * 0.09,
                )
            } else if (0.685..0.92).contains(&uv.y) {
                let across = (uv.y - 0.80) * 68.;
                let row = (across / 0.20).floor() as i32;
                let stagger = sample(row as u32, 0, 93) * 3.;
                let joint = seam(across, 0.20, 0.034).max(seam(fore + stagger, 4., 0.025));
                let timber = 205. + sample(row as u32, 1, 92) * 22. + grain_at(8, 220, 85) * 22.
                    - joint * 41.;
                ([timber, timber * 0.98, timber * 0.94], 0.83)
            } else {
                ([255.; 3], 0.54)
            };
            colour.extend([rgb[0] as u8, rgb[1] as u8, rgb[2] as u8, 255]);
            surface.extend([255, (roughness * 255.) as u8, 255, 255]);
        }
    }
    StandardMaterial {
        base_color_texture: Some(images.add(mipmapped(
            WIDTH,
            HEIGHT,
            colour,
            TextureFormat::Rgba8UnormSrgb,
        ))),
        metallic_roughness_texture: Some(images.add(mipmapped(
            WIDTH,
            HEIGHT,
            surface,
            TextureFormat::Rgba8Unorm,
        ))),
        perceptual_roughness: 1.,
        metallic: 0.10,
        ..default()
    }
}
