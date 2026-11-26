// Work drawn up in the yard and not yet built is drawn on the ship as a blueprint: deep
// blue stock ruled with a pale grid, its edges picked out where they turn from the eye.
// When the refit is built a sweep runs from bow to stern, and behind a bright band the
// blueprint gives way to the ship as she is painted.
import bevy_pbr::render::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
    pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT,
};

struct Draft {
    // Stretches of the ship, as world z from and to, that are drawn as blueprint.
    spans: array<vec4f, 8>,
    // How many spans are in use, the z the reveal has reached, 1 while it runs, and
    // the glow of its band.
    sweep: vec4f,
}
@group(3) @binding(100) var<uniform> draft: Draft;

const PAPER: vec3f = vec3f(0.018, 0.075, 0.20);
const INK: vec3f = vec3f(0.42, 0.88, 1.15);
const BAND: vec3f = vec3f(1.6, 2.4, 2.6);

// How strongly a point lies on a ruled line `pitch` metres apart, smoothed over a pixel.
fn ruled(uv: vec2f, pitch: f32, width: f32) -> f32 {
    let g = uv / pitch;
    let d = abs(fract(g - 0.5) - 0.5) / max(fwidth(g), vec2f(1e-4));
    return 1.0 - clamp(min(d.x, d.y) - width, 0.0, 1.0);
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color = alpha_discard(
        pbr_input.material.flags,
        pbr_input.material.alpha_cutoff,
        pbr_input.material.base_color
    );
    var lit: vec4f;
    if (pbr_input.material.flags & STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        lit = apply_pbr_lighting(pbr_input);
    } else {
        lit = pbr_input.material.base_color;
    }

    let p = in.world_position.xyz;
    var drafted = 0.0;
    let count = u32(draft.sweep.x);
    for (var i = 0u; i < 8u; i++) {
        let s = draft.spans[i];
        let inside = i < count && p.z >= s.x && p.z <= s.y;
        drafted = max(drafted, select(0.0, 1.0, inside));
    }

    // The grid lies on whichever plane the surface most nearly faces.
    let n = abs(pbr_input.world_normal);
    var uv = p.xy;
    if n.y >= n.x && n.y >= n.z {
        uv = p.xz;
    } else if n.x >= n.z {
        uv = p.zy;
    }
    let fine = ruled(uv, 0.5, 0.0) * 0.35;
    let bold = ruled(uv, 2.5, 0.6);
    let rim = pow(1.0 - abs(dot(pbr_input.N, pbr_input.V)), 2.2);
    let ink = mix(PAPER, INK, clamp(max(max(fine, bold), rim * 0.85), 0.0, 1.0));

    // Behind the sweep the ship is built. The band glows where it passes.
    let revealing = draft.sweep.z;
    let built = revealing * smoothstep(draft.sweep.y + 0.35, draft.sweep.y - 0.35, p.z);
    let band = revealing * draft.sweep.w * exp(-abs(p.z - draft.sweep.y) * 2.2);
    let shown = drafted * (1.0 - built);
    var color = vec4f(mix(lit.rgb, ink, shown) + BAND * band * drafted, lit.a);

    var out: FragmentOutput;
    out.color = main_pass_post_lighting_processing(pbr_input, color);
    return out;
}
