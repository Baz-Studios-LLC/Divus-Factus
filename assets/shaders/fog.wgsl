// The fog of the unknown world.
//
// The village's knowledge is a home circle and a scatter of pockets brought
// back by explorers - so the fog is not a texture that has to be painted and
// stored, it is a distance field evaluated per pixel from a handful of
// circles. Ground the village knows is clear; everything else takes the veil.
//
// This draws on a COPY of the terrain's own mesh, lifted over the treetops, so
// the veil follows every hill and hollow instead of slicing through them the
// way a flat plane would.
//
// ONE sheet, and its weight comes from how far the view ray travels through the
// bank rather than from how many sheets it crosses. It used to be a stack of
// six, each thin, and looking along them from a low camera you could count
// them: six pale contour lines lying across the distance. On the flat world the
// DISTANCE FOG hid that, by fading everything far away into the horizon before
// the sheets could be resolved; the round world hides its own distance over the
// horizon instead, the fog went with it, and the stack was left standing there
// to be counted.
//
// A slab is the honest model of a bank of mist, and it is one draw rather than
// six. Looking down through it the ray crosses the bank's own height; looking
// along it the ray crosses a great deal more, so the far edge of the veil
// thickens into a wall by itself — which is the thing the stack was built to
// fake in the first place.

#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::view

struct FogParams {
    // rgb is the veil's colour, a is how heavy it gets at its thickest.
    tint: vec4<f32>,
    // xyz the home ground's centre, w its radius.
    home: vec4<f32>,
    // x how many pockets are live, y how many metres the edge takes to fade,
    // z how deep the bank of mist stands, w unused.
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

    // How much bank this pixel's ray actually goes through. The sheet is the
    // TOP of a slab standing `dials.z` deep on the ground, so a ray meeting it
    // squarely crosses that depth and a ray meeting it at a graze crosses
    // depth / cos(angle) — unbounded in the limit, which is why it is clamped
    // to a few times the depth rather than allowed to run away at the horizon.
    let to_eye = normalize(view.world_position.xyz - in.world_position.xyz);
    let facing = max(abs(dot(normalize(in.world_normal), to_eye)), 0.0001);
    let depth = max(fog.dials.z, 0.001);
    let travelled = min(depth / facing, depth * 6.0);

    // Beer-Lambert. `tint.a` is the weight of one depth of bank looked at
    // squarely, so the extinction that reproduces it is -ln(1 - a) / depth, and
    // every other angle follows from the same law instead of being tuned.
    let extinction = -log(max(1.0 - fog.tint.a, 0.002)) / depth;
    let density = 1.0 - exp(-extinction * travelled);

    return vec4<f32>(fog.tint.rgb, density * veil);
}
