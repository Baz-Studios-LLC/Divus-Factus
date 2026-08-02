// The fog of the unknown world.
//
// The village's knowledge is a home circle and a scatter of pockets brought
// back by explorers - so the fog is not a texture that has to be painted and
// stored, it is a distance field evaluated per pixel from a handful of
// circles. Ground the village knows is clear; everything else takes the veil.
//
// This draws on a COPY of the terrain's own mesh, lifted a hand's breadth, so
// the veil follows every hill and hollow instead of slicing through them the
// way a flat plane would.

#import bevy_pbr::forward_io::VertexOutput

struct FogParams {
    // rgb is the veil's colour, a is how heavy it gets at its thickest.
    tint: vec4<f32>,
    // xyz the home ground's centre, w its radius.
    home: vec4<f32>,
    // x how many pockets are live, y how many metres the edge takes to fade.
    dials: vec4<f32>,
    // xyz each pocket's centre, w its radius.
    pockets: array<vec4<f32>, 128>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> fog: FogParams;

// A smooth union: where two circles come near, their boundaries melt
// together instead of overlapping like coins. k is the metres over which
// neighbours blend - the whole reason the known world reads as one
// coastline rather than a pattern of footfall stamps.
fn smax(a: f32, b: f32, k: f32) -> f32 {
    let h = clamp(0.5 + 0.5 * (a - b) / k, 0.0, 1.0);
    return mix(b, a, h) + k * h * (1.0 - h);
}

// Cheap value noise, for an edge that wavers like ground actually walked
// rather than ground swept by a compass.
fn wobble(at: vec2<f32>) -> f32 {
    let cell = floor(at);
    let f = fract(at);
    let u = f * f * (3.0 - 2.0 * f);
    let a = fract(sin(dot(cell, vec2<f32>(127.1, 311.7))) * 43758.547);
    let b = fract(sin(dot(cell + vec2<f32>(1.0, 0.0), vec2<f32>(127.1, 311.7))) * 43758.547);
    let c = fract(sin(dot(cell + vec2<f32>(0.0, 1.0), vec2<f32>(127.1, 311.7))) * 43758.547);
    let d = fract(sin(dot(cell + vec2<f32>(1.0, 1.0), vec2<f32>(127.1, 311.7))) * 43758.547);
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y) - 0.5;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let ground = in.world_position.xz;
    let soft = max(fog.dials.y, 0.001);

    // How far INSIDE the known world this pixel lies, in metres. Negative is
    // outside. The union of circles is smoothed, and the boundary itself
    // wavers a few strides - two scales of it, so neither reads as tiling.
    var known = fog.home.w - distance(ground, fog.home.xz);
    let live = i32(fog.dials.x);
    for (var i = 0; i < live; i = i + 1) {
        let pocket = fog.pockets[i];
        known = smax(known, pocket.w - distance(ground, pocket.xz), 18.0);
    }
    known += wobble(ground / 37.0) * 14.0 + wobble(ground / 11.0) * 5.0;

    let veil = 1.0 - smoothstep(-soft, soft, known);
    if veil < 0.004 {
        discard;
    }
    return vec4<f32>(fog.tint.rgb, fog.tint.a * veil);
}
