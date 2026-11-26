// Gun smoke as a light-scattering volume. Each puff is a sphere of smoke drawn on a cube
// about it. The eye's ray is marched through the sphere, where the density is a lumpy
// noise that thins toward the edge. At each step the smoke takes light from the sun,
// shadowed by the smoke between, from the sky, and from every gun flash nearby. A flash
// behind the smoke shines through it toward the eye, so its light is scattered forward.
import bevy_pbr::render::forward_io::{Vertex, decompress_vertex};
import bevy_pbr::render::mesh_functions::{get_world_from_local, get_tag};
import bevy_pbr::render::mesh_view_bindings::{view, lights, fog};
import bevy_pbr::render::view_transformations::position_world_to_clip;
@if(TONEMAP_IN_SHADER) import bevy_core_pipeline::tonemapping::tone_mapping;

const FLASHES: u32 = 16u;
const STEPS: i32 = 14;
const PI: f32 = 3.14159265;
// Extinction across one radius of the thickest smoke.
const OPACITY: f32 = 3.;
// Fresh smoke is the brown of burnt cordite, and pales to grey as it spreads.
const CORDITE: vec3f = vec3f(0.34, 0.29, 0.24);
const ASH: vec3f = vec3f(0.60, 0.60, 0.61);

struct Smoke {
    // Each flash: where it is, and how bright.
    flashes: array<vec4f, 16>,
    // The flash's colour, and how many are in use.
    glow: vec4f,
    // Seconds, for the slow churn of the smoke.
    clock: vec4f,
}
@group(3) @binding(0) var<uniform> smoke: Smoke;

struct PuffVertex {
    @builtin(position) position: vec4f,
    @location(0) world: vec3f,
    @location(1) centre: vec3f,
    @location(2) radius: f32,
    @location(3) @interpolate(flat) tag: u32,
}

@vertex
fn vertex(in: Vertex) -> PuffVertex {
    let local = decompress_vertex(in, in.instance_index);
    let model = get_world_from_local(in.instance_index);
    let world = (model * vec4f(local.position, 1.)).xyz;
    var out: PuffVertex;
    out.position = position_world_to_clip(world);
    out.world = world;
    out.centre = model[3].xyz;
    out.radius = length(model[0].xyz);
    out.tag = get_tag(in.instance_index);
    return out;
}

fn hash(p: vec3f) -> f32 {
    let q = fract(p * vec3f(0.1031, 0.1030, 0.0973));
    let r = q + dot(q, q.yxz + 33.33);
    return fract((r.x + r.y) * r.z);
}

fn noise(p: vec3f) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3. - 2. * f);
    let a = mix(hash(i), hash(i + vec3f(1., 0., 0.)), u.x);
    let b = mix(hash(i + vec3f(0., 1., 0.)), hash(i + vec3f(1., 1., 0.)), u.x);
    let c = mix(hash(i + vec3f(0., 0., 1.)), hash(i + vec3f(1., 0., 1.)), u.x);
    let d = mix(hash(i + vec3f(0., 1., 1.)), hash(i + vec3f(1., 1., 1.)), u.x);
    return mix(mix(a, b, u.y), mix(c, d, u.y), u.z);
}

// Billows: big lumps with smaller ones on them.
fn billows(p: vec3f) -> f32 {
    return 0.58 * noise(p) + 0.29 * noise(p * 2.13 + 7.1) + 0.13 * noise(p * 4.41 + 1.7);
}

// Metres from `p` inside the unit sphere to its edge along `d`.
fn to_edge(p: vec3f, d: vec3f) -> f32 {
    let b = dot(p, d);
    let c = dot(p, p) - 1.;
    return -b + sqrt(max(b * b - c, 0.));
}

// Henyey-Greenstein: how much light turned through the angle with this cosine.
fn scatter(cosine: f32, g: f32) -> f32 {
    let k = 1. + g * g - 2. * g * cosine;
    return (1. - g * g) / (4. * PI * pow(max(k, 1e-4), 1.5));
}

@fragment
fn fragment(in: PuffVertex) -> @location(0) vec4f {
    let eye = view.world_position;
    let ray = normalize(in.world - eye);
    let radius = max(in.radius, 0.01);
    // Where the ray enters and leaves the sphere, in units of its radius.
    let o = (eye - in.centre) / radius;
    let b = dot(o, ray);
    let h = b * b - (dot(o, o) - 1.);
    if h <= 0. {
        discard;
    }
    let near = max(-b - sqrt(h), 0.);
    let far = -b + sqrt(h);
    if far <= near {
        discard;
    }

    let thick = f32(in.tag & 255u) / 255.;
    let seed = f32((in.tag >> 8u) & 255u);
    let fresh = f32((in.tag >> 16u) & 255u) / 255.;
    let albedo = mix(ASH, CORDITE, fresh);
    let churn = vec3f(seed * 1.37, seed * 0.71, seed * 2.03) + vec3f(0., -0.07, 0.05) * smoke.clock.x;

    var sun = vec3f(0.);
    var sun_dir = vec3f(0., 1., 0.);
    if lights.n_directional_lights > 0u {
        sun = lights.directional_lights[0].color.rgb * view.exposure;
        sun_dir = lights.directional_lights[0].direction_to_light;
    }
    let sky = lights.ambient_color.rgb * view.exposure;
    let sun_turn = scatter(dot(ray, sun_dir), 0.35) * 4. * PI;
    let count = u32(smoke.glow.w);

    let step = (far - near) / f32(STEPS);
    // A dither along the ray hides the steps.
    var t = near + step * hash(vec3f(in.position.xy, seed));
    var through = 1.;
    var seen = vec3f(0.);
    for (var i = 0; i < STEPS; i++) {
        let q = o + ray * t;
        let r = length(q);
        if r < 1. {
            // A lumpy surface well inside the sphere, its billows standing out to the edge.
            let lumps = billows(q * 2.3 + churn);
            let shape = (0.8 - r) * 2.2 + (lumps - 0.5) * 0.9;
            let density = thick * smoothstep(0., 0.2, shape) * (0.55 + 0.9 * lumps);
            if density > 0.002 {
                let extinction = density * OPACITY;
                // The sun, through the smoke between here and the edge toward it.
                let shade = exp(-to_edge(q, sun_dir) * thick * 0.5 * OPACITY);
                // The sky lights the outside of the billows more than their hearts.
                let open = 0.45 + 0.35 * q.y + 0.4 * smoothstep(0.6, 0., shape);
                var light = albedo * (sun * shade * sun_turn / PI + sky * open);
                // Each flash, through the smoke toward it, falling off past a few metres.
                let p = in.centre + q * radius;
                for (var k = 0u; k < FLASHES; k++) {
                    if k >= count {
                        break;
                    }
                    let flash = smoke.flashes[k];
                    let to = flash.xyz - p;
                    let gap = length(to);
                    let dir = to / max(gap, 1e-3);
                    let inside = min(to_edge(q, dir), gap / radius);
                    let veil = exp(-inside * thick * 0.5 * OPACITY);
                    let fall = 1. / (1. + gap * gap / 36.);
                    let turn = scatter(dot(dir, ray), 0.45) * 4. * PI;
                    light += smoke.glow.rgb * flash.w * 4. * veil * fall * turn;
                }
                let kept = exp(-extinction * step);
                seen += through * light * (1. - kept);
                through *= kept;
                if through < 0.02 {
                    break;
                }
            }
        }
        t += step;
    }
    let alpha = 1. - through;
    if alpha < 0.004 {
        discard;
    }
    var colour = vec4f(seen / alpha, alpha);
    // Linear haze, as the rest of the scene has.
    if fog.mode == 1u {
        let span = max(fog.be.y - fog.be.x, 1.);
        let clear = clamp((fog.be.y - distance(in.centre, eye)) / span, 0., 1.);
        colour = vec4f(mix(fog.base_color.rgb, colour.rgb, clear), colour.a);
    }
    @if(TONEMAP_IN_SHADER) {
        colour = tone_mapping(colour, view.color_grading);
    }
    return colour;
}
