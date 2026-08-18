// THE HAND'S REACH: a ring on the ground where the hand would close, and the
// smoke coming off it.
//
// One skirt of geometry does both. Its bottom edge sits on the ground and is
// the ring; going up, it thins into smoke and is gone. `uv.y` is 0 at the
// ground and 1 at the top of the skirt, `uv.x` runs once around.

#import bevy_pbr::forward_io::VertexOutput

struct ReachParams {
    // rgb the ring's color, a its overall strength.
    tint: vec4<f32>,
    // x seconds, y how far around the ring one turn of smoke is, z unused,
    // w unused.
    dials: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> reach: ReachParams;

// A cheap value noise. Nothing here is on screen long enough or large enough
// to want anything better, and a texture would be one more asset to load for
// a ring that is off by default.
fn hash(p: vec2<f32>) -> f32 {
    let h = dot(p, vec2<f32>(127.1, 311.7));
    return fract(sin(h) * 43758.5453);
}

fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    // Smoothstep the cell, or the noise reads as a grid of squares - which on
    // a ring this thin is very visible.
    let u = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(hash(i), hash(i + vec2<f32>(1.0, 0.0)), u.x),
        mix(hash(i + vec2<f32>(0.0, 1.0)), hash(i + vec2<f32>(1.0, 1.0)), u.x),
        u.y
    );
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let up = in.uv.y;
    let around = in.uv.x;
    let time = reach.dials.x;
    let turns = reach.dials.y;

    // THE RING, at the very bottom. Bright and nearly solid, and thin enough
    // that it reads as a line drawn on the ground rather than a wall standing
    // on it.
    let ring = 1.0 - smoothstep(0.0, 0.10, up);

    // THE SMOKE above it. Two layers of noise scrolling upward at different
    // rates, because one layer scrolling on its own reads as a moving
    // texture; two moving through each other read as something rising.
    let drift = vec2<f32>(around * turns, up * 2.4 - time * 0.34);
    let curl = vec2<f32>(around * turns * 1.7 + time * 0.06, up * 3.6 - time * 0.52);
    let body = noise(drift) * 0.6 + noise(curl) * 0.4;

    // Thinning as it goes up, and torn: below about a third of the way the
    // smoke is mostly there, and above that the noise decides what survives.
    let fade = pow(1.0 - up, 2.2);
    let torn = smoothstep(0.34, 0.86, body + fade * 0.7);
    let smoke = torn * fade * 0.55;

    let strength = (ring + smoke) * reach.tint.a;
    if strength < 0.004 {
        discard;
    }

    // The ring itself is brighter than white on purpose: this is an HDR
    // camera with bloom hanging off it, so anything over one blooms, and the
    // bloom is where the ETHEREAL in "ethereal circle" actually comes from.
    // The smoke stays under one and simply glows near the ring.
    let heat = 1.0 + ring * 2.6;
    return vec4<f32>(reach.tint.rgb * heat, strength);
}
