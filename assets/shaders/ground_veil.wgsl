// The ground's own veil: the fog of war mixed INTO the lit ground and
// everything standing on it, rather than draped over the top in a sheet.
//
// The veil used to be an occluder - a copy of the terrain lifted into a bank
// that hid what was under it. A bank has to be tall enough to bury a wood,
// which makes it a solid object standing in the world, and a solid object has
// a SIDE and an EDGE. Every veil bug of one long evening was one of those: the
// bank floating over ground the planet drew at a coarser height, its edge
// ending in a cliff where the chunks ran out, trees standing clean on top of
// it, and the ground beneath it keeping its own daylight colours so that the
// shroud read as a sheet laid over a readable world. Brett, looking at the
// last of them: "this chunk is veiled but the land and trees underneith it
// arent painted."
//
// So nothing is hidden. Unknown ground is simply not the colour of known
// ground, and neither are its trees. There is no bank to see under, no edge to
// meet the planet's own paint at, and no height for the two to disagree about:
// the veil is exactly as tall as the world it covers. Brett: "what about
// rendered land that has the veil over it we paint the ground the veil color".
//
// This is what the planet's patches have always done (`planet_skin.wgsl`) -
// they carry the mark in a vertex colour, because a patch is rebuilt when it
// changes anyway. Here it is read PER PIXEL from the same uniform the cloths
// used, because the known circle grows every time somebody walks, and a baked
// vertex mark would mean rebuilding every chunk in the world on every step.
//
// The mix happens AFTER the lighting, for the reason the planet's skin gives:
// paint a colour into a surface before the sun reaches it and the sun changes
// it - half again toward white at midday, something else at dusk - while the
// veil must be one colour under every sky.

#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
    forward_io::{VertexOutput, FragmentOutput},
}

struct FogParams {
    // rgb the veil's colour, a its weight at full veil.
    tint: vec4<f32>,
    // xyz the home ground's centre, w its radius.
    home: vec4<f32>,
    // x how many pockets are live, y how many metres the edge takes to fade.
    dials: vec4<f32>,
    // xyz the planet's centre, w its radius.
    planet: vec4<f32>,
    // Each known pocket: xyz its centre, w its radius.
    pockets: array<vec4<f32>, 128>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> fog: FogParams;

/// How many metres two neighbouring pockets melt into one another.
///
/// Generous on purpose. Knowledge is STORED as circles because that makes
/// exploration cheap to compute and easy to inspect, but a circle is not a
/// thing anybody has ever seen at the edge of a wood. Blend them hard enough
/// and a line of footsteps reads as one coastline rather than a row of coins.
const EDGE_MELT: f32 = 26.0;

/// A smooth union, so two overlapping pockets merge into one bay instead of
/// meeting in a crease.
fn smax(a: f32, b: f32, softness: f32) -> f32 {
    let h = clamp(0.5 + 0.5 * (b - a) / softness, 0.0, 1.0);
    return mix(a, b, h) + softness * h * (1.0 - h);
}

/// How far a circle's edge wanders in and out as you walk around it, as a
/// FRACTION of that circle's own radius.
///
/// Three harmonics, seeded from the circle's centre so the same ground always
/// wavers the same way - it must not shimmer as the camera moves, and it must
/// survive a save. Proportional rather than absolute so a scout's little
/// pocket and a town's whole home circle wear the same kind of coastline at
/// their own scale; a fixed amplitude turned small pockets inside out.
///
/// Brett: "Can we have the border of the veil smooth better instead of looking
/// like punched circles?" This was in the shader all along as
/// `frontier_wobble`, defined and never called - the fork left the function
/// standing and stopped using it, so the edge went back to being a compass
/// line.
fn frontier_wobble(centre: vec2<f32>, angle: f32) -> f32 {
    let tau = 6.28318530718;
    let first = sin(centre.x * 0.013 + centre.y * 0.021) * tau;
    let second = sin(centre.x * 0.029 - centre.y * 0.011) * tau;
    let third = sin(centre.x * 0.007 + centre.y * 0.037) * tau;
    return sin(angle * 2.0 + first) * 0.15
        + sin(angle * 5.0 + second) * 0.07
        + sin(angle * 9.0 + third) * 0.035;
}

/// One circle's reach at this bearing: its radius, wandered.
fn frontier(centre: vec2<f32>, radius: f32, ground: vec2<f32>) -> f32 {
    let delta = ground - centre;
    let angle = atan2(delta.y, delta.x);
    return radius * (1.0 + frontier_wobble(centre, angle)) - length(delta);
}

/// The FLAT ground this pixel stands on, which is not where it is drawn.
///
/// The world is simulated flat and bent onto a sphere for drawing, and what
/// the village knows is written in the flat coordinates. Undoing the bend is
/// the mapping read backwards - see `terrain::direction_at`, `globe::bend_frame`
/// and the identical function in `fog.wgsl`. It normalises the direction from
/// the planet's centre, so height above the ground does not move the answer: a
/// treetop asks the same question as the soil under it.
fn flat_ground(world_position: vec3<f32>) -> vec2<f32> {
    let from_centre = world_position - fog.planet.xyz;
    let unturned = vec3<f32>(from_centre.x, -from_centre.z, from_centre.y);
    let dir = normalize(unturned);
    let lat = asin(clamp(dir.y, -1.0, 1.0));
    let lon = atan2(dir.x, dir.z);
    return vec2<f32>(lon * fog.planet.w, -lat * fog.planet.w);
}

// Positive is inside knowledge, negative is in the unknown.
fn known_at(ground: vec2<f32>) -> f32 {
    var known = frontier(fog.home.xz, fog.home.w, ground);
    let live = i32(fog.dials.x);
    for (var i = 0; i < live; i = i + 1) {
        let pocket = fog.pockets[i];
        known = smax(known, frontier(pocket.xz, pocket.w, ground), EDGE_MELT);
    }
    return known;
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color =
        alpha_discard(pbr_input.material, pbr_input.material.base_color);

    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);

    let known = known_at(flat_ground(in.world_position.xyz));
    // Over the edge, not at it: the same handful of metres the cloths faded
    // across, so the boundary of what the village has walked is a shore rather
    // than a cut line.
    let beyond = clamp(-known / max(fog.dials.y, 0.001), 0.0, 1.0);
    let veiled = beyond * fog.tint.a;
    out.color = vec4<f32>(mix(out.color.rgb, fog.tint.rgb, veiled), out.color.a);

    return out;
}
