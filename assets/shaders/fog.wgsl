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
    pockets: array<vec4<f32>, 64>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> fog: FogParams;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let ground = in.world_position.xz;
    let soft = max(fog.dials.y, 0.001);

    // How far INSIDE the known world this pixel lies, in metres. Negative is
    // outside. The union of circles is the largest of these.
    var known = fog.home.w - distance(ground, fog.home.xz);
    let live = i32(fog.dials.x);
    for (var i = 0; i < live; i = i + 1) {
        let pocket = fog.pockets[i];
        known = max(known, pocket.w - distance(ground, pocket.xz));
    }

    let veil = 1.0 - smoothstep(-soft, soft, known);
    if veil < 0.004 {
        discard;
    }
    return vec4<f32>(fog.tint.rgb, fog.tint.a * veil);
}
