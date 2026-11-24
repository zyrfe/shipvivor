//! Tiled salt, run-off and paint loss on the exposed steel and timber of the yards.
use super::super::surfaces::{grain, tile, weathered_mesh};
use super::Works;
use bevy::prelude::*;

impl Works {
    pub(super) fn finish(self) -> Mesh {
        weathered_mesh(self.0, 5.3)
    }
}

pub(crate) fn material(images: &mut Assets<Image>) -> StandardMaterial {
    const SIDE: u32 = 128;
    let mut rgba = Vec::with_capacity((SIDE * SIDE * 4) as usize);
    for y in 0..SIDE {
        for x in 0..SIDE {
            let uv = Vec2::new(x as f32, y as f32) / SIDE as f32;
            let cloud = grain(uv, IVec2::new(6, 5), 127);
            let streak = grain(uv, IVec2::new(43, 6), 31);
            let grit = grain(uv, IVec2::new(91, 79), 509);
            let rust = ((0.52 * cloud + 0.32 * streak + 0.16 * grit - 0.58) * 4.).clamp(0., 1.);
            let value = (204. + 24. * cloud + 21. * grit + 9. * streak).min(255.);
            rgba.extend([
                (value - 25. * rust) as u8,
                (value - 62. * rust) as u8,
                (value - 79. * rust) as u8,
                255,
            ]);
        }
    }
    StandardMaterial {
        base_color_texture: Some(images.add(tile(SIDE, rgba))),
        perceptual_roughness: 0.88,
        metallic: 0.02,
        emissive: LinearRgba::rgb(0.018, 0.017, 0.014),
        ..default()
    }
}
