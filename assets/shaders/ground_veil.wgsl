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

/// A soft maximum, so two overlapping pockets merge into one bay instead of
/// meeting in a crease. The same one the cloths used.
fn smax(a: f32, b: f32, softness: f32) -> f32 {
    let h = clamp(0.5 + 0.5 * (b - a) / softness, 0.0, 1.0);
    return mix(a, b, h) + softness * h * (1.0 - h);
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
