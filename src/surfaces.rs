//! Tiled surface detail shared by ships and coastal scenery.
use crate::scene::Facets;
use bevy::{
    asset::RenderAssetUsages,
    image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};

pub(super) fn hash(mut n: u32) -> u32 {
    n ^= n >> 16;
    n = n.wrapping_mul(0x7feb352d);
    n ^= n >> 15;
    n = n.wrapping_mul(0x846ca68b);
    n ^ (n >> 16)
}

pub(super) fn sample(x: u32, y: u32, seed: u32) -> f32 {
    let n = x.wrapping_mul(0x9e3779b9) ^ y.wrapping_mul(0x85ebca6b) ^ seed;
    (hash(n) >> 8) as f32 / 16777216.
}

pub(super) fn grain(p: Vec2, cells: IVec2, seed: u32) -> f32 {
    let q = p * cells.as_vec2();
    let i = q.floor().as_ivec2();
    let f = q - q.floor();
    let s = f * f * (3. - 2. * f);
    let row = |y: i32| {
        let at = |x: i32| {
            sample(
                (i.x + x).rem_euclid(cells.x) as u32,
                (i.y + y).rem_euclid(cells.y) as u32,
                seed,
            )
        };
        at(0).lerp(at(1), s.x)
    };
    row(0).lerp(row(1), s.y)
}

pub(super) fn weathered_mesh(facets: Facets, repeat: f32) -> Mesh {
    let mut mesh = facets.finish();
    let positions = mesh
        .attribute(Mesh::ATTRIBUTE_POSITION)
        .unwrap()
        .as_float3()
        .unwrap();
    let normals = mesh
        .attribute(Mesh::ATTRIBUTE_NORMAL)
        .unwrap()
        .as_float3()
        .unwrap();
    let uv = positions
        .iter()
        .zip(normals)
        .map(|(p, n)| {
            let [x, y, z] = *p;
            let axis = Vec3::from(*n).abs();
            let st = if axis.y > axis.x.max(axis.z) {
                Vec2::new(x, z)
            } else if axis.x > axis.z {
                Vec2::new(z, -y)
            } else {
                Vec2::new(x, -y)
            };
            (st / repeat).to_array()
        })
        .collect::<Vec<_>>();
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uv);
    mesh
}

pub(super) fn tile(side: u32, pixels: Vec<u8>) -> Image {
    mipmapped(side, side, pixels, TextureFormat::Rgba8UnormSrgb)
}

pub(crate) fn mipmapped(width: u32, height: u32, pixels: Vec<u8>, format: TextureFormat) -> Image {
    let mut image = Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        format,
        RenderAssetUsages::default(),
    );
    // At patrol distances hundreds of texels fall into one screen pixel.
    let data = image.data.as_mut().unwrap();
    let mut src = 0;
    let (mut w, mut h) = (width as usize, height as usize);
    while w > 1 || h > 1 {
        let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
        let end = data.len();
        for y in 0..nh {
            for x in 0..nw {
                let base = src + 4 * (2 * y * w + 2 * x);
                let right = if w > 1 { 4 } else { 0 };
                let down = if h > 1 { w * 4 } else { 0 };
                for c in 0..4 {
                    let sum: u16 = [0, right, down, down + right]
                        .iter()
                        .map(|k| u16::from(data[base + k + c]))
                        .sum();
                    data.push((sum / 4) as u8);
                }
            }
        }
        src = end;
        (w, h) = (nw, nh);
        image.texture_descriptor.mip_level_count += 1;
    }
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        ..ImageSamplerDescriptor::linear()
    });
    image
}
