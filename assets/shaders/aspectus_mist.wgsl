// ASPECTUS: the ground fog.
//
// Mist that lies in the low ground of damp and cold country, and burns off the
// ridges. It is drawn by marching the ray from the eye to whatever the pixel
// actually shows, adding up how much wet air it passed through on the way.
//
// WHY A MARCH AND NOT A WASH. This game DELETED its distance fog on purpose -
// "a round world hides its own distance over the horizon, which is what fog was
// faking" (render/mod.rs). So the one thing this must never become is a gray
// veil over the far away. Mist here is an OBJECT in the world with a top and a
// bottom: it fills the bottom of a valley and stops, so a ridge stands out of
// it like an island, and looking along a hollow shows more of it than looking
// across one. Only a march gives that.
//
// WHERE THE MIST IS comes from a field baked on the processor (`mist.rs`) - the
// shader cannot ask, because the climate is three octaves of seeded fbm and a
// WGSL copy would drift out of step with the ground it lies on. The field is a
// small square of world around the camera: settled mist in R, ground height in
// G.
//
// WHAT COLOR IT IS: two colors, chosen by where the sun is. Look into a low sun
// across a valley and the mist blows out warm and BRIGHTER THAN ITSELF, which
// the bloom pass then catches; turn away and it is cool and quiet. One flat
// color in every direction is what a screen filter looks like.
//
// AND NEVER THE VEIL'S BLUE. The fog of war is a cold slate blue mixed over
// unknown ground, and the player reads the frontier off that color. Weather
// that shared the hue would make the two impossible to tell apart, so the cool
// end of the mist is deliberately kept warm-neutral rather than blue.

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput
#import bevy_render::view::View

struct MistParams {
    // rgb the mist facing away from the sun, a the overall strength.
    tint: vec4<f32>,
    // rgb the mist facing into the sun, a how low the sun is.
    sunward: vec4<f32>,
    // xyz the planet's center, w its radius.
    planet: vec4<f32>,
    // xy the field's low corner in flat sim coords, z its span in meters,
    // w how deep the mist lies over its ground.
    field: vec4<f32>,
    // x how far the march reaches, y the height range the field's green
    // channel spans, z the debug view, w the most the mist may ever hide.
    dials: vec4<f32>,
    // xyz toward the sun.
    sun: vec4<f32>,
}

@group(0) @binding(0) var depth: texture_depth_multisampled_2d;
@group(0) @binding(1) var<uniform> view: View;
@group(0) @binding(2) var<uniform> mist: MistParams;
@group(0) @binding(3) var field: texture_2d<f32>;
@group(0) @binding(4) var field_sampler: sampler;

/// Nothing: fully transparent, and the blend leaves the frame alone.
const UNTOUCHED: vec4<f32> = vec4<f32>(0.0, 0.0, 0.0, 0.0);

/// How many samples along the ray. Sixteen is enough because the march is
/// DITHERED - the banding a low count would give is turned into a fine noise
/// the eye reads as the grain of the air itself.
const STEPS: i32 = 16;

/// How sharply the mist brightens as the view turns into the sun. Six is a
/// broad forward lobe - a glow across a whole valley rather than a hotspot.
const FORWARD: f32 = 6.0;

/// How much brighter than its own color the mist gets looking into a low sun.
/// This is the number that can exceed one and reach the bloom.
const GLOW: f32 = 2.2;

/// The FLAT ground a world position stands on - the bend read backwards.
/// The same function as `ground_veil.wgsl` and `aspectus_veil.wgsl`, and the
/// three must agree to the meter.
fn flat_ground(world_position: vec3<f32>) -> vec2<f32> {
    let from_center = world_position - mist.planet.xyz;
    let unturned = vec3<f32>(from_center.x, -from_center.z, from_center.y);
    let dir = normalize(unturned);
    let lat = asin(clamp(dir.y, -1.0, 1.0));
    let lon = atan2(dir.x, dir.z);
    return vec2<f32>(lon * mist.planet.w, -lat * mist.planet.w);
}

/// How much wet air is at a point, given the flat ground it stands over.
///
/// The flat coordinate is passed in rather than worked out here: `asin` and
/// `atan2` are multi-instruction sequences on this hardware and running them at
/// every one of sixteen steps was most of the cost of the pass. Along a few
/// hundred meters the flat coordinate moves almost exactly linearly, so the
/// march interpolates it between the ends and is right to well under a cell.
fn density_at(at: vec3<f32>, ground: vec2<f32>) -> f32 {
    let uv = (ground - mist.field.xy) / mist.field.z;
    // Outside the baked square there is no answer. FADED rather than cut: the
    // sampler clamps, so every edge cell would otherwise smear its value out
    // to the horizon and hang a straight-edged wall of mist in the distance.
    if uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 {
        return 0.0;
    }
    let from_edge = min(min(uv.x, 1.0 - uv.x), min(uv.y, 1.0 - uv.y));
    let inside = smoothstep(0.0, 0.08, from_edge);
    if inside <= 0.0 {
        return 0.0;
    }

    let baked = textureSampleLevel(field, field_sampler, uv, 0.0);
    let weight = baked.r;
    if weight <= 0.0 {
        return 0.0;
    }

    // How high this point stands above the land under it.
    let ground_height = baked.g * mist.dials.y;
    let altitude = length(at - mist.planet.xyz) - mist.planet.w - ground_height;

    // A REAL TOP, not just a fading-out. An exponential falloff alone has no
    // surface anywhere in it, and what makes ground fog memorable is precisely
    // a surface: a flat white sea filling a valley with the ridges standing out
    // of it as islands. So the mist thins upward AND stops - softly, over a few
    // meters, but it stops, and the depth it stops at is decided by how much
    // mist this ground carries. The wettest hollows are deep in it; a thin
    // shoulder wears a few meters of haze.
    let deep = mist.field.w * (0.35 + 0.65 * weight);
    let above = max(altitude, 0.0);
    let thinning = exp(-above / max(deep * 0.55, 0.5));
    let lid = 1.0 - smoothstep(deep * 0.75, deep * 1.35, above);

    return weight * thinning * lid * inside;
}

/// How high a point stands over the land under it, for bracketing the march.
///
/// The same reckoning `density_at` does, without the density: it is used to find
/// where along a ray the fog can possibly be, so the sixteen expensive samples
/// can all be spent inside that stretch. Outside the bake there is no ground to
/// be above, and a very large number is the honest answer - it reads as "no fog
/// here", which is what the density function says there too.
fn altitude_of(at: vec3<f32>, ground: vec2<f32>) -> f32 {
    let uv = (ground - mist.field.xy) / mist.field.z;
    if uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 {
        return 1.0e6;
    }
    let baked = textureSampleLevel(field, field_sampler, uv, 0.0);
    return length(at - mist.planet.xyz) - mist.planet.w - baked.g * mist.dials.y;
}

/// A per-pixel dither, so sixteen steps do not read as sixteen bands.
fn shuffle(pixel: vec2<f32>) -> f32 {
    return fract(sin(dot(pixel, vec2<f32>(12.9898, 78.233))) * 43758.5453);
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    if mist.tint.a < 0.002 {
        return UNTOUCHED;
    }

    let size = textureDimensions(depth);
    let texel = vec2<i32>(in.uv * vec2<f32>(size));
    let z = textureLoad(depth, texel, 0);

    let eye = view.world_position;
    let ndc_xy = vec2<f32>(in.uv.x * 2.0 - 1.0, (1.0 - in.uv.y) * 2.0 - 1.0);

    // Where this pixel's ray ends.
    //
    // ON SKY THE MARCH STILL RUNS, which matters more than it sounds. Cutting
    // the mist off wherever the depth buffer is empty would delete exactly the
    // thing worth having - a bank spilling over a crest and hanging against a
    // pale dawn sky - and replace it with a hard line along every ridge in the
    // world, following the terrain silhouette exactly.
    var reach = mist.dials.x;
    var toward: vec3<f32>;
    if z > 0.0 {
        let world = view.world_from_clip * vec4<f32>(ndc_xy, z, 1.0);
        let at = world.xyz / world.w;
        let span = length(at - eye);
        toward = (at - eye) / max(span, 0.0001);
        reach = min(span, mist.dials.x);
    } else {
        let far = view.world_from_clip * vec4<f32>(ndc_xy, 0.000001, 1.0);
        toward = normalize(far.xyz / far.w - eye);
    }

    if reach <= 0.0 {
        return UNTOUCHED;
    }

    // The flat coordinate at both ends, interpolated between - see `density_at`.
    let near_ground = flat_ground(eye + toward * 0.001);
    let far_ground = flat_ground(eye + toward * reach);

    // SPEND THE SAMPLES WHERE THE FOG IS. Sixteen steps across the whole reach
    // is up to fifty meters a step, and the fog slab is ten deep - so at any
    // distance most rays sampled it ONCE, or not at all, and the dither below
    // decided which. A coin flip per pixel is exactly what salt-and-pepper looks
    // like. Brett: "at a distance at night the fog has this dirty look", and
    // "the fog up close looks perfect, it just has this heavy noise at a
    // distance" - up close is precisely where the steps were already short
    // enough to land inside the layer more than once.
    //
    // So the stretch of ray that CAN hold fog is bracketed first, with five
    // cheap altitude probes, and the sixteen density samples are spent inside
    // it. Five texture reads against sixteen saved from empty air, and a ray
    // that never meets the fog at all now costs five and stops.
    //
    // Bracketed by PROBES rather than by solving the crossing, because a hill
    // between the ends rises into a ray that a straight line says is clear, and
    // a solve would clip the fog off its shoulder. The bracket keeps the
    // neighbouring probe on each side, so the stretch is always wider than the
    // fog rather than narrower.
    let ceiling = mist.field.w * 1.35;
    var lowest = 5;
    var highest = -1;
    for (var probe = 0; probe <= 4; probe = probe + 1) {
        let part = f32(probe) / 4.0;
        let along = reach * part;
        let high = altitude_of(eye + toward * along, mix(near_ground, far_ground, part));
        if high <= ceiling {
            lowest = min(lowest, probe);
            highest = max(highest, probe);
        }
    }
    if highest < 0 {
        // The whole ray runs above the fog: nothing to gather, and the cheapest
        // frames in the game are the ones spent on sky.
        return UNTOUCHED;
    }
    // `from` and `to` are RESERVED WORDS in WGSL, which the shader compiler
    // says plainly and only at runtime - there is no cargo check for this.
    let begins = reach * max(f32(lowest - 1), 0.0) / 4.0;
    let ends = reach * min(f32(highest + 1), 4.0) / 4.0;

    let step = (ends - begins) / f32(STEPS);
    let jitter = shuffle(in.uv * vec2<f32>(size)) * step;
    var gathered = 0.0;
    for (var i = 0; i < STEPS; i = i + 1) {
        let along = begins + jitter + f32(i) * step;
        if along > ends {
            break;
        }
        let part = along / reach;
        gathered += density_at(eye + toward * along, mix(near_ground, far_ground, part)) * step;
    }

    if gathered <= 0.0 {
        return UNTOUCHED;
    }

    // Beer's law: air does not add up, it lets less and less through. This is
    // what keeps a long look down a valley from going to solid white the way a
    // plain sum would.
    let through = exp(-gathered * mist.tint.a);
    let hidden = (1.0 - through) * mist.dials.w;

    // Which way the sun is, from here. A broad forward lobe: the mist between
    // the eye and a low sun catches the light and throws it back.
    let facing = max(dot(toward, mist.sun.xyz), 0.0);
    let forward = pow(facing, FORWARD) * mist.sunward.a;
    let color = mix(mist.tint.rgb, mist.sunward.rgb, forward) * (1.0 + forward * GLOW);

    if mist.dials.z > 0.5 {
        // THE DEBUG VIEW (DIVUS_FACTUS_MIST_DEBUG=1): how much air this ray
        // crossed, solid, with none of the world showing through.
        return vec4<f32>(vec3<f32>(0.1, hidden, 0.35), 1.0);
    }

    // PREMULTIPLIED: the color is already scaled by its own coverage, which is
    // what lets it be brighter than itself where the sun is in it.
    return vec4<f32>(color * hidden, hidden);
}
