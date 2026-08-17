// ASPECTUS: the veil, as a pass.
//
// The world is drawn. Its depth says where every pixel of it actually is. So
// this walks the frame once, turns each pixel's depth back into a position in
// the world, asks whether the village has walked there, and tints what it has
// not - the ground, the trees, the roofs, the people, the wolves, and anything
// anyone ever adds, because all of them wrote depth.
//
// That is the whole reason it is a pass. As a material the veil had to be
// opted into, one material at a time - and villagers, animals and buildings
// never were, so an explorer walking into unwalked country walked about lit.
//
// IT NEVER READS THE FRAME. The veil is a tint mixed over the lit world, and
// `mix(color, tint, w)` is exactly what alpha blending does - so this shader
// outputs the tint with `w` in its alpha and lets the blend state do the mix.
// The pass draws straight onto the frame with no copy, no ping-pong, and no
// claim on the screen texture; see `veil.rs` for the night the ping-pong
// version cost.
//
// The math is the bend read backwards, twice over. Clip space back to the
// world by the view's own inverse, and then the world back to the FLAT ground
// the simulation runs on, because that is the only space the village's
// knowledge is written in.

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput
#import bevy_render::view::View

struct FogParams {
    tint: vec4<f32>,
    home: vec4<f32>,
    // x how many pockets are live, y how many meters the edge takes,
    // z turns on the debug view (DIVUS_FACTUS_VEIL_DEBUG=1).
    dials: vec4<f32>,
    planet: vec4<f32>,
    pockets: array<vec4<f32>, 128>,
}

@group(0) @binding(0) var depth: texture_depth_multisampled_2d;
@group(0) @binding(1) var<uniform> view: View;
@group(0) @binding(2) var<uniform> fog: FogParams;

const EDGE_MELT: f32 = 26.0;

/// Nothing: fully transparent, and the blend leaves the frame alone.
const UNTOUCHED: vec4<f32> = vec4<f32>(0.0, 0.0, 0.0, 0.0);

fn smax(a: f32, b: f32, softness: f32) -> f32 {
    let h = clamp(0.5 + 0.5 * (b - a) / softness, 0.0, 1.0);
    return mix(a, b, h) + softness * h * (1.0 - h);
}

/// How far a circle's edge wanders, as a fraction of its own radius. Seeded
/// from the circle's center so the same ground always wavers the same way -
/// it must not shimmer as the camera moves. The same three harmonics the
/// ground material uses, so the pass and the paint agree to the meter.
fn frontier_wobble(center: vec2<f32>, angle: f32) -> f32 {
    let tau = 6.28318530718;
    let first = sin(center.x * 0.013 + center.y * 0.021) * tau;
    let second = sin(center.x * 0.029 - center.y * 0.011) * tau;
    let third = sin(center.x * 0.007 + center.y * 0.037) * tau;
    return sin(angle * 2.0 + first) * 0.15
        + sin(angle * 5.0 + second) * 0.07
        + sin(angle * 9.0 + third) * 0.035;
}

fn frontier(center: vec2<f32>, radius: f32, ground: vec2<f32>) -> f32 {
    let delta = ground - center;
    let angle = atan2(delta.y, delta.x);
    return radius * (1.0 + frontier_wobble(center, angle)) - length(delta);
}

/// The FLAT ground a world position stands on. Normalizes the direction from
/// the planet's center, so height above the ground does not move the answer:
/// a villager's head asks the same question as the soil under their boots.
/// The mapping and its inverse live in `globe.rs`; the same function is in
/// `ground_veil.wgsl`, and the two must agree to the meter.
fn flat_ground(world_position: vec3<f32>) -> vec2<f32> {
    let from_center = world_position - fog.planet.xyz;
    let unturned = vec3<f32>(from_center.x, -from_center.z, from_center.y);
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
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    if fog.tint.a < 0.002 && fog.dials.z < 0.5 {
        return UNTOUCHED;
    }

    // Raw, unfiltered, at the exact texel: averaging two depths would give a
    // position lying on neither surface, which along every silhouette is a
    // pixel that belongs to nothing and gets veiled or spared at random.
    let size = textureDimensions(depth);
    let texel = vec2<i32>(in.uv * vec2<f32>(size));
    // Sample zero of the four: any one of them is a real surface at this
    // pixel, and the veil's edge is meters wide, so which one is beneath
    // noticing.
    let z = textureLoad(depth, texel, 0);

    // Nothing was drawn here - the sky. The sky is not ground and cannot be
    // unknown; veiling it would put a slate lid over the world.
    if z <= 0.0 {
        return UNTOUCHED;
    }

    // Clip space back to the world. Bevy's depth is reversed-z, and
    // `world_from_clip` already accounts for it, so the depth goes in exactly
    // as it was read. The perspective divide is the step that is easy to
    // forget and impossible to miss: without it everything past a few meters
    // reports as standing at the camera.
    let ndc = vec3<f32>(in.uv.x * 2.0 - 1.0, (1.0 - in.uv.y) * 2.0 - 1.0, z);
    let world = view.world_from_clip * vec4<f32>(ndc, 1.0);
    let at = world.xyz / world.w;

    let ground = flat_ground(at);
    let known = known_at(ground);

    // THE DEBUG VIEW (DIVUS_FACTUS_VEIL_DEBUG=1): the pass's own working,
    // painted over the world. Green is known, red is unknown, and the checker
    // is the flat ground in hundred-meter squares - a steady grid means the
    // reconstruction is sane, and noise means it is not. One screenshot
    // answers the question a night of reading code could not.
    if fog.dials.z > 0.5 {
        let checker =
            f32((i32(floor(ground.x / 100.0)) + i32(floor(ground.y / 100.0))) & 1) * 0.25;
        if known > 0.0 {
            return vec4<f32>(0.1 + checker, 0.7, 0.2, 1.0);
        }
        return vec4<f32>(0.7, 0.1 + checker, 0.2, 1.0);
    }

    // A shore rather than a cut line, over the same handful of meters the
    // ground material fades across. The tint carries the weight in its alpha
    // and the blend state performs the mix.
    let beyond = clamp(-known / max(fog.dials.y, 0.001), 0.0, 1.0);
    return vec4<f32>(fog.tint.rgb, beyond * fog.tint.a);
}
