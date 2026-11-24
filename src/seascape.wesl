import bevy_pbr::render::forward_io::{Vertex, decompress_vertex};
import bevy_pbr::render::mesh_functions::get_world_from_local;
import bevy_pbr::render::mesh_view_bindings::view;
import bevy_pbr::render::view_transformations::position_world_to_clip;

const TAU: f32 = 6.2831853;
const TRAINS: u32 = 8u;
const RIPPLES: i32 = 13;
// Seconds after which the ripples repeat. The clock they read wraps there.
const RIPPLE_PERIOD: f32 = 512.;
const SUN: vec3f = vec3f(0.5094, 0.1898, -0.8393);

struct Waters {
    // Each swell train: its wave vector, amplitude and phase.
    trains: array<vec4f, 8>,
    // The ship, east and south, then one over the span of the wave and foam patches.
    focus: vec4f,
    // Fractions of a half-span between which a patch fades out, then the chop.
    fade: vec4f,
    // Size of the finest cell, and the distance from the ship at which cells start to grow.
    lattice: vec4f,
    // Sea state, overcast, seconds, and seconds within the ripple period.
    weather: vec4f,
    bow_flow: vec4f,
    chart: vec4f,
    sky: u32,
}
@group(3) @binding(0) var<uniform> waters: Waters;
@group(3) @binding(1) var wake_texture: texture_2d<f32>;
@group(3) @binding(2) var wake_sampler: sampler;
@group(3) @binding(3) var foam_texture: texture_2d<f32>;
@group(3) @binding(4) var foam_sampler: sampler;
@group(3) @binding(5) var shoals: texture_2d<f32>;
@group(3) @binding(6) var shore_sampler: sampler;

struct SeaVertex {
    @builtin(position) position: vec4f,
    @location(0) world: vec3f,
    // Where this water lies when the sea is calm. The surface is shaded there.
    @location(1) rest: vec2f,
}

fn hash(p: vec2<f32>) -> f32 {
    let a = fract(vec3f(p.xyx) * vec3f(0.1031, 0.1030, 0.0973));
    let b = a + dot(a, a.yzx + 33.33);
    return fract((b.x + b.y) * b.z);
}
fn noise(p: vec2f) -> f32 {
    let c = floor(p);
    let f = fract(p);
    let k = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(c), hash(c + vec2f(1,0)), k.x),
               mix(hash(c + vec2f(0,1)), hash(c + vec2f(1,1)), k.x), k.y);
}
fn cloud(p: vec2f) -> f32 {
    return 0.52*noise(p) + 0.27*noise(p*2.07 + 3.1) + 0.14*noise(p*4.31-7.6) + 0.07*noise(p*9.13);
}
// `disc` is 1 to draw the sun itself and 0 for the sky the water reflects, which adds its own glitter.
fn sky_color(d: vec3f, disc: f32) -> vec3f {
    let overcast = waters.weather.y;
    let time = waters.weather.z;
    let elevation = max(d.y, 0.0);
    let facing = max(dot(d, SUN), 0.0);
    let glow = pow(facing, 5.) * (1. - 0.85*overcast);
    let horizon = mix(mix(vec3f(0.43,0.56,0.64), vec3f(0.36,0.40,0.44), overcast), vec3f(1.42,0.82,0.39), glow);
    let zenith = mix(vec3f(0.095,0.245,0.40), vec3f(0.17,0.21,0.26), overcast);
    var color = mix(horizon, zenith, 1.-exp(-2.9*elevation));
    let uv = d.xz / (elevation + 0.16) * vec2f(1.9, 3.1) + vec2f(time*(0.004 + 0.012*overcast), 3.6);
    let mass = cloud(uv);
    let cover = smoothstep(0.41 - 0.30*overcast, 0.63 - 0.30*overcast, mass) * smoothstep(-0.02,0.12,d.y);
    let edge = clamp(0.50 + 4.8*(cloud(uv + SUN.xz*0.22) - mass), 0., 1.);
    let shade = mix(mix(vec3f(0.16,0.225,0.29), vec3f(0.13,0.15,0.18), overcast),
                    mix(vec3f(0.86,0.81,0.72), vec3f(0.34,0.37,0.40), overcast), edge);
    color = mix(color, shade + pow(facing, 12.)*vec3f(0.62,0.35,0.12)*(1.-overcast), cover*(0.88 + 0.10*overcast));
    color += (pow(facing, 55.)*0.55 + disc*smoothstep(0.99960,0.99985,facing)*3.5)
        * vec3f(1.,0.74,0.40) * (1.-cover*0.83) * (1.-0.9*overcast);
    return color;
}

// How much of a wrapped patch is live at a point: all of it near the ship, none past its edge.
fn shown(p: vec2f, per_span: f32) -> f32 {
    let out = abs(p - waters.focus.xy) * per_span * 2.;
    return 1. - smoothstep(waters.fade.x, waters.fade.y, max(out.x, out.y));
}
// Read a patch along a smooth curve through its cells. Straight lines between
// cells three quarters of a metre apart show as facets beside the ship. The
// curve weighs sixteen cells, gathered here in four reads.
fn curved(cells: texture_2d<f32>, smoothing: sampler, uv: vec2f) -> vec4f {
    let size = vec2f(textureDimensions(cells));
    let at = uv * size - 0.5;
    let base = floor(at);
    let f = at - base;
    let before = (1. - f) * (1. - f) * (1. - f) / 6.;
    let here = (3. * f * f * f - 6. * f * f + 4.) / 6.;
    let after = f * f * f / 6.;
    let first = before + here;
    let second = 1. - first;
    let a = (base - 0.5 + here / first) / size;
    let b = (base + 1.5 + after / second) / size;
    return first.y * (first.x * textureSampleLevel(cells, smoothing, a, 0.)
                      + second.x * textureSampleLevel(cells, smoothing, vec2f(b.x, a.y), 0.))
        + second.y * (first.x * textureSampleLevel(cells, smoothing, vec2f(a.x, b.y), 0.)
                      + second.x * textureSampleLevel(cells, smoothing, b, 0.));
}
// The wake at a point: its height, then its slope to the east and to the south.
fn wake(p: vec2f) -> vec3f {
    let live = shown(p, waters.focus.z);
    if live <= 0. { return vec3f(0.); }
    let uv = p * waters.focus.z + 0.5 / vec2f(textureDimensions(wake_texture));
    return curved(wake_texture, wake_sampler, uv).xyz * live;
}

@vertex
fn vertex(in: Vertex) -> SeaVertex {
    let local = decompress_vertex(in, in.instance_index);
    var world = (get_world_from_local(in.instance_index) * vec4f(local.position, 1.)).xyz;
    var out: SeaVertex;
    out.rest = world.xz;
    if waters.sky == 0u {
        // The lattice coarsens with distance. A wave only a few cells long would
        // crawl as the vertices passed under it, so it is left to the shading.
        let out_from = abs(world.xz - waters.focus.xy);
        let cell = waters.lattice.x * max(1., max(out_from.x, out_from.y) / waters.lattice.y);
        for (var i = 0u; i < TRAINS; i++) {
            let train = waters.trains[i];
            let number = length(train.xy);
            let amplitude = train.z * smoothstep(2.5 * cell, 5. * cell, TAU / number);
            let angle = dot(train.xy, out.rest) - train.w;
            let reach = train.xy / number * (waters.fade.z * amplitude * sin(angle));
            world += vec3f(-reach.x, amplitude * cos(angle), -reach.y);
        }
        world.y += wake(out.rest).x * (1. - smoothstep(0.8, 1.3, cell));
    }
    out.world = world;
    out.position = position_world_to_clip(world);
    return out;
}

// The swell under a pixel `footprint` metres across. Returns the normal,
// unnormalised, then how much of the slope was too fine to resolve. The
// normal's height is also how far the water there has been drawn together:
// it falls toward zero where a crest is about to break. `calm` is how far a
// hull's slick has flattened the shorter trains.
fn swell(p: vec2f, footprint: f32, calm: f32) -> vec4f {
    var slope = vec2f(0.);
    var squeeze = vec3f(0.);
    var lost = 0.;
    for (var i = 0u; i < TRAINS; i++) {
        let train = waters.trains[i];
        let number = length(train.xy);
        let keep = (1. - smoothstep(0.3, 0.8, footprint * number / TAU))
            * (1. - 0.85 * calm * smoothstep(0.3, 0.9, number));
        let angle = dot(train.xy, p) - train.w;
        slope += train.xy * (train.z * keep * sin(angle));
        let pull = waters.fade.z * train.z * keep * cos(angle) / number;
        squeeze += vec3f(train.x * train.x, train.y * train.y, train.x * train.y) * pull;
        lost += 0.5 * pow(train.z * number, 2.) * (1. - keep * keep);
    }
    let east = 1. - squeeze.x;
    let south = 1. - squeeze.y;
    return vec4f(slope.y * squeeze.z + slope.x * south,
                 east * south - squeeze.z * squeeze.z,
                 slope.x * squeeze.z + slope.y * east,
                 lost);
}

// Wind ripples, too short for the lattice. Each is drawn toward the crests of
// the longer ones before it, which bunches them the way wind does. Returns the
// slope east and south, then the squared slope too fine for the pixel.
// `calm` is how far a hull's slick has smoothed them away.
fn ripples(start: vec2f, footprint: f32, calm: f32) -> vec3f {
    let wind = normalize(waters.trains[0].xy);
    var p = start;
    var number = 2.3;
    var steep = (0.07 + 0.05 * waters.weather.x) * (1. - 0.94 * calm);
    var slope = vec2f(0.);
    var lost = 0.;
    for (var i = 0; i < RIPPLES; i++) {
        let keep = 1. - smoothstep(0.2, 0.5, footprint * number / TAU);
        if keep > 0. {
            let veer = 1.25 * sin(f32(i) * 2.4);
            let along = vec2f(wind.x * cos(veer) - wind.y * sin(veer), wind.x * sin(veer) + wind.y * cos(veer));
            // Each rate is rounded to a whole number of turns per period, so the clock can wrap.
            let rate = round(sqrt(9.81 * number) * RIPPLE_PERIOD / TAU) * TAU / RIPPLE_PERIOD;
            let angle = dot(along, p) * number - rate * waters.weather.w + f32(i) * 1.7;
            let lean = exp(sin(angle) - 1.) * cos(angle);
            slope += along * (steep * lean * keep);
            p -= along * (1.4 * lean * steep / number);
        }
        lost += 0.07 * steep * steep * (1. - keep * keep);
        number *= 1.24;
        steep *= 0.89;
    }
    return vec3f(slope, lost);
}

// Where foam lasts longest as it thins: low along a net of veins, high in the gaps between.
fn lace(p: vec2f) -> f32 {
    let time = waters.weather.z;
    let warp = vec2f(noise(p * 0.31 + 11.3), noise(p * 0.31 - 7.7)) * 2.6;
    let wide = abs(2. * noise(p * 0.52 + warp + vec2f(0.05, -0.03) * time) - 1.);
    let fine = abs(2. * noise(p * 1.9 - warp * 0.7 + vec2f(-0.04, 0.06) * time) - 1.);
    return 0.10 + 0.85 * wide + 0.5 * fine * noise(p * 5.3);
}

@fragment
fn fragment(in: SeaVertex) -> @location(0) vec4f {
    let toward = in.world - view.world_position;
    let dist = length(toward);
    let d = toward / dist;
    if waters.sky != 0u { return vec4f(sky_color(d, 1.), 1.); }
    let state = waters.weather.x;
    let overcast = waters.weather.y;
    let daylight = 1. - 0.5 * overcast;
    let p = in.rest;
    // Filter what a pixel cannot resolve instead of letting it alias toward the horizon.
    let footprint = max(length(dpdx(p)), length(dpdy(p)));
    let near = 1. - smoothstep(0.75, 2.5, footprint);

    // What ships have left on the water: foam, and the slick where their wash has smoothed the ripples away.
    let foam_uv = p * waters.focus.w + 0.5 / vec2f(textureDimensions(foam_texture));
    var left = vec2f(0.);
    let lain = shown(p, waters.focus.w);
    if lain > 0. { left = curved(foam_texture, foam_sampler, foam_uv).rg * lain; }
    let trail = left.x * 1.5;
    let slick = left.y;

    let rolling = swell(p, footprint, slick);
    let fine = ripples(p, footprint, slick);
    let stirred = wake(p) * (near * (1. - 0.7 * slick));
    // The wash still wells up in slow boils inside the slick.
    let churn = p * 0.33 + vec2f(0.11, -0.07) * waters.weather.z;
    let level = noise(churn);
    let boils = vec2f(noise(churn + vec2f(0.4, 0.)) - level, noise(churn + vec2f(0., 0.4)) - level) * (0.16 * slick * near);
    let n = normalize(vec3f(rolling.x - fine.x - stirred.y - boils.x, rolling.y, rolling.z - fine.y - stirred.z - boils.y));
    let lost = rolling.w + fine.z;

    // Unresolved slopes still turn part of the distant water toward the eye,
    // which keeps the horizon from becoming a mirror.
    let facing = max(dot(n, -d), 0.9 * sqrt(lost));
    let fresnel = 0.02 + 0.98 * pow(1. - min(facing, 1.), 5.);
    var up = reflect(d, n);
    up = normalize(vec3f(up.x, abs(up.y) + 0.6 * sqrt(lost), up.z));
    let mirrored = sky_color(up, 0.);

    // Light scattered back out of the water: more through a thin crest, and more looking up-sun.
    let crest = clamp(0.5 + in.world.y / (0.5 + 2.2 * state), 0., 1.);
    let through = pow(max(dot(d.xz, SUN.xz), 0.), 3.) * (1. - overcast);
    let depths = mix(vec3f(0.008,0.038,0.066), vec3f(0.014,0.034,0.042), overcast);
    let shallows = mix(vec3f(0.030,0.175,0.190), vec3f(0.040,0.105,0.105), overcast);
    var body = mix(depths, shallows, clamp(0.10 + 0.42 * crest + 0.45 * through * crest, 0., 1.));
    // Air beaten into a wake pales the water long after the white has gone.
    body = mix(body, vec3f(0.085,0.30,0.30), min(0.55 * trail + 0.42 * slick, 0.62)) * daylight;
    var sea = mix(body, mirrored, fresnel);

    // Sunlight thrown off facets tilted the right way. Rougher water spreads it.
    let halfway = normalize(SUN - d);
    let spread = 0.004 + 2. * lost;
    let lobe = spread / (3.1416 * pow(pow(max(dot(n, halfway), 0.), 2.) * (spread - 1.) + 1., 2.));
    let glance = 0.02 + 0.98 * pow(1. - max(dot(halfway, -d), 0.), 5.);
    sea += vec3f(1.,0.80,0.52) * min(lobe * glance * 1.4, 9.) * (1. - overcast) * step(0., up.y);

    // Whitecaps where the swell has drawn the water together, and the foam ships have left.
    // Only some crests break: the wind comes in gusts.
    let gust = smoothstep(0.35, 0.65, noise(p * 0.035 + vec2f(0.05, 0.02) * waters.weather.z));
    let breaking = smoothstep(0.18, 0.31, 1. - rolling.y) * gust * (1. - slick);
    // Waves run up the bank and drain back through the rocks in ragged patches.
    var surf = 0.;
    if waters.chart.z > 0. {
        let chart_uv = (in.world.xz - waters.chart.xy) * waters.chart.zw;
        let bed = textureSampleLevel(shoals, shore_sampler, chart_uv, 0.).r * 12. - 8.;
        let depth = in.world.y - bed;
        if depth < 4.6 {
            let wind = normalize(waters.trains[0].xy);
            let pulse = 0.5 + 0.5 * sin(dot(p, wind) * 0.32 - waters.weather.z * 1.32);
            let runup = noise(p * 0.17 - 0.14 * wind * waters.weather.z);
            let wash = 1. - smoothstep(-0.38, 0.8 + state + 0.85 * pulse, depth);
            surf = wash * (0.28 + 0.55 * runup + 0.25 * pulse + 0.09 * gust);
            sea = mix(sea, vec3f(0.075,0.255,0.23)*daylight, 0.19*(1.-smoothstep(0.,4.6,depth)));
        }
    }
    // The stem shears a short sheet of white water into two widening, broken shoulders.
    var bow_white = 0.;
    var bow_bubbles = 0.;
    let offset = in.world.xz - waters.bow_flow.xy;
    if waters.bow_flow.w > 1. && dot(offset, offset) < 245. {
        let fore = normalize(waters.bow_flow.xy - waters.focus.xy);
        let aft = -dot(offset, fore);
        let transverse = dot(offset, vec2f(-fore.y, fore.x));
        let side = abs(transverse);
        let run = clamp(aft + 0.85, 0., 15.);
        let spread = waters.bow_flow.z * 0.20 * pow(run, 0.88);
        let teeth = noise(p * 2.1 + fore * waters.weather.z * 0.42) - 0.5;
        let edge = abs(side - spread - teeth * 0.45);
        let sheet = 1. - smoothstep(0.08, 0.65 + run * 0.065, edge);
        let lengthwise = smoothstep(-1.15, 0., aft) * (1. - smoothstep(5., 14., aft));
        let speed = smoothstep(1., 8., waters.bow_flow.w);
        let flow = vec2f(transverse * 4.8, aft * 1.9 - waters.weather.z * 2.7);
        let film = abs(noise(flow + teeth * 2.6) * 2. - 1.);
        let pores = noise(flow * 3.7) * 0.4 + film;
        bow_bubbles = speed * lengthwise * sheet;
        bow_white = bow_bubbles * (0.61 + 0.42 * smoothstep(0., 0.65, stirred.x))
            * (0.42 + 0.74 * smoothstep(0.16, 0.72, pores));
        sea = mix(sea, vec3f(0.11,0.34,0.35) * daylight, 0.43 * bow_white);
    }
    let amount = max(bow_white, max(surf, max(trail, breaking)));
    let net = lace(p);
    let veins = smoothstep(0., 0.28, amount - net);
    let cover = mix(0.75 * smoothstep(0.15, 1.1, amount), veins, near);
    let white = vec3f(0.86,0.90,0.90) * (0.86 + 0.14 * net + 0.12 * bow_bubbles)
        * (mix(0.52, 0.50, overcast) + 0.46 * max(dot(n, SUN), 0.) * (1. - overcast));
    sea = mix(sea, white, cover * 0.94);

    let haze = 0.88 * (1. - exp(-pow(dist / 2680., 1.6)));
    return vec4f(mix(sea, sky_color(normalize(vec3f(d.x, 0.004, d.z)), 0.), haze), 1.);
}
