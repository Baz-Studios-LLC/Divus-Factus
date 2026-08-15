// The fog of the unknown world.
//
// The village's knowledge is a home circle and a scatter of pockets brought
// back by explorers - so the fog is not a texture that has to be painted and
// stored, it is a distance field evaluated per pixel from a handful of
// circles. Ground the village knows is clear; everything else takes the veil.
//
// This draws on a COPY of the terrain's own mesh. The vertex stage raises that
// exact surface only as it crosses from known ground into the unknown, making
// the veil one continuous bank instead of a flat lid plus a separate wall.
//
// The veil is an OCCLUDER, not a tint laid over readable ground. Its inner end
// begins beneath the terrain, so the ground itself hides the join while the
// solid bank rises into the unknown.

#import bevy_pbr::{
    mesh_functions,
    forward_io::{Vertex, VertexOutput},
    mesh_view_bindings::view,
}

struct FogParams {
    // rgb is the veil's colour, a is how much of the stippled shroud has risen.
    tint: vec4<f32>,
    // xyz the home ground's centre, w its radius.
    home: vec4<f32>,
    // x how many pockets are live, y how many metres the edge takes to fade,
    // z how high the bank of mist rises, w unused.
    dials: vec4<f32>,
    // xyz the planet's centre, w its radius.
    planet: vec4<f32>,
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

// Knowledge is stored as simple circles because that makes exploration cheap
// and inspectable. This stable contour turns each circle's drawn edge into a
// piece of landscape instead of a compass line. `fog.rs::frontier_wobble`
// uses the same terms when it builds the opaque wall.
fn frontier_wobble(centre: vec2<f32>, angle: f32) -> f32 {
    let tau = 6.28318530718;
    let first = sin(centre.x * 0.013 + centre.y * 0.021) * tau;
    let second = sin(centre.x * 0.029 - centre.y * 0.011) * tau;
    let third = sin(centre.x * 0.007 + centre.y * 0.037) * tau;
    return sin(angle * 2.0 + first) * 26.0
        + sin(angle * 5.0 + second) * 12.0
        + sin(angle * 9.0 + third) * 6.0;
}

// Undo the world bend and recover the map coordinates used by the simulation.
// Direction is all that matters, so this works for the terrain itself and for
// the same vertex after the veil has lifted it away from the planet.
fn flat_ground(world_position: vec3<f32>) -> vec2<f32> {
    let from_centre = world_position - fog.planet.xyz;
    let unturned = vec3<f32>(from_centre.x, -from_centre.z, from_centre.y);
    let dir = normalize(unturned);
    let lat = asin(clamp(dir.y, -1.0, 1.0));
    let lon = atan2(dir.x, dir.z);
    return vec2<f32>(lon * fog.planet.w, -lat * fog.planet.w);
}

/// How far the bank's inner end sinks beneath the terrain it copies.
///
/// The join has to happen somewhere, and under the ground is the one place
/// it cannot be seen. Small: any deeper and the bank starts its climb from
/// too far down to reach full height inside the taper.
const VEIL_SINK: f32 = 0.6;

// Positive is inside knowledge, negative is in the unknown.
fn known_at(ground: vec2<f32>) -> f32 {
    let home_delta = ground - fog.home.xz;
    var known = fog.home.w - length(home_delta);
    let live = i32(fog.dials.x);
    for (var i = 0; i < live; i = i + 1) {
        let pocket = fog.pockets[i];
        let delta = ground - pocket.xz;
        let pocket_known = pocket.w - length(delta);
        known = smax(known, pocket_known, 8.0);
    }
    return known;
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;

    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    let ground_position = mesh_functions::mesh_position_local_to_world(
        world_from_local,
        vec4(vertex.position, 1.0),
    );

    let radial = normalize(ground_position.xyz - fog.planet.xyz);

    // THE BANK STANDS UP, AND LIES DOWN AT ITS INNER EDGE.
    //
    // Brett's shape: "keep it at 16m but taper it to the ground, that way it
    // covers everything but looks seamless because it would taper down and
    // slightly clip through the ground." Height and seam are two jobs, and
    // they were being asked of one number - the bank was flattened to zero
    // so that its edge would not show, which left it a skin painted on the
    // terrain that every tree, rock and roof stood clean on top of.
    //
    // So: full height out in the unknown, tapering to nothing at the
    // boundary and finishing BELOW the ground, where the terrain hides the
    // join. `flat_ground` normalises the direction from the planet's centre,
    // so a purely radial lift does not move the flat coordinate by a
    // millimetre - the fragment's `known` is the same number whether it is
    // asked before this lift or after it, and the taper therefore lines up
    // exactly with the discard.
    let beyond = max(0.0, -known_at(flat_ground(ground_position.xyz)));
    let climb = clamp(beyond / max(fog.dials.w, 0.001), 0.0, 1.0);
    // Smoothstep, so the bank leaves the ground and reaches its ceiling
    // without a crease at either end.
    let eased = climb * climb * (3.0 - 2.0 * climb);
    let lift = fog.dials.z * eased - VEIL_SINK;
    let world_position = vec4(
        ground_position.xyz + radial * lift,
        1.0,
    );

    out.world_position = world_position;
    out.position = view.clip_from_world * world_position;
    out.world_normal = mesh_functions::mesh_normal_local_to_world(
        vertex.normal,
        vertex.instance_index,
    );
#ifdef VERTEX_UVS_A
    out.uv = vertex.uv;
#endif
#ifdef VERTEX_COLORS
    out.color = vertex.color;
#endif
#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    out.instance_index = vertex.instance_index;
#endif
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let ground = flat_ground(in.world_position.xyz);
    let known = known_at(ground);
    // Explored ground is clear; unknown ground wears the veil.
    if known > 0.0 || fog.tint.a < 0.004 {
        discard;
    }
    return vec4<f32>(fog.tint.rgb, fog.tint.a);
}
