// Stylised water.
//
// Everything here is fragment-only: the surface stays two triangles and all the
// motion comes from perturbing the normal. From a god's-eye camera that is
// indistinguishable from displaced geometry, and it costs no vertices, no
// tessellation and no mesh updates as the sea follows the player.
//
// Lighting is hand-rolled rather than PBR. The rest of the world is flat-shaded
// low-poly, and a physically correct sea in the middle of it looks borrowed.

#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::{globals, view}
#import bevy_pbr::prepass_utils::prepass_depth

struct WaterParams {
    shallow: vec4<f32>,
    deep: vec4<f32>,
    sky: vec4<f32>,
    // xyz is the direction toward the sun.
    sun: vec4<f32>,
    wave_scale: f32,
    wave_speed: f32,
    wave_strength: f32,
    specular: f32,
    /// Depth over which water goes from clear to fully opaque.
    depth_fade: f32,
    /// Width of the foam band at the shoreline.
    foam_width: f32,
    _pad: vec2<f32>,
}

// Bevy substitutes the material bind group index; hard-coding it fails validation.
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> water: WaterParams;

fn hash21(p: vec2<f32>) -> f32 {
    var h = fract(p * vec2<f32>(0.1031, 0.1030));
    h += dot(h, h.yx + 33.33);
    return fract((h.x + h.y) * h.x);
}

/// Smooth value noise. Breaks the residual periodicity that pure sines cannot avoid.
fn value_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash21(i);
    let b = hash21(i + vec2<f32>(1.0, 0.0));
    let c = hash21(i + vec2<f32>(0.0, 1.0));
    let d = hash21(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

/// Height of the wave field at a world-space point.
///
/// Three octaves, each rotated by the golden angle relative to the last. Sine waves
/// at similar headings reinforce into parallel stripes — invisible up close,
/// unmistakable from altitude — and rotating by an angle with no rational
/// relationship to a full turn means the crests never line up.
///
/// The single noise tap outside the loop is what kills the residual regularity.
/// It used to be one tap *per octave*, which with the four-tap normal below meant
/// twenty noise lookups for every pixel of a sea that fills half the screen. That
/// alone took the frame rate to eight.
fn wave_height(p: vec2<f32>, t: f32) -> f32 {
    var total = 0.0;
    var amp = 1.0;
    var norm = 0.0;
    var q = p;

    let a = 2.39996;
    let rot = mat2x2<f32>(cos(a), -sin(a), sin(a), cos(a));

    for (var i = 0; i < 3; i = i + 1) {
        total += sin(dot(q, vec2<f32>(0.82, 0.57)) + t * (1.0 + f32(i) * 0.37)) * amp;
        norm += amp;
        amp *= 0.55;
        q = rot * q * 1.9;
    }

    total += (value_noise(p * 0.65 + vec2<f32>(t * 0.13, t * -0.09)) - 0.5) * 1.4;
    return total / norm;
}

/// Surface normal of the wave field, from finite differences.
///
/// `detail` fades the perturbation with distance. Waves a kilometre away occupy less
/// than a pixel, and left at full strength they alias into a shimmering moiré — the
/// same reason distant water in reality reads as a flat sheet.
///
/// Below a threshold the field is skipped entirely rather than computed and then
/// faded out. Most of the sea on screen at any time is far away, so this early-out
/// removes the wave cost from the majority of pixels.
fn wave_normal(p: vec2<f32>, t: f32, strength: f32, detail: f32) -> vec3<f32> {
    if detail < 0.2 {
        return vec3<f32>(0.0, 1.0, 0.0);
    }

    let e = 0.35;
    let dx = wave_height(p + vec2<f32>(e, 0.0), t) - wave_height(p - vec2<f32>(e, 0.0), t);
    let dz = wave_height(p + vec2<f32>(0.0, e), t) - wave_height(p - vec2<f32>(0.0, e), t);
    let s = strength * detail;
    return normalize(vec3<f32>(-dx * s, 1.0, -dz * s));
}

/// Converts a reverse-Z depth buffer value to a linear view-space distance.
fn linear_depth(raw: f32) -> f32 {
    // Bevy uses reverse-Z infinite perspective, where this single term inverts it.
    return view.clip_from_view[3][2] / max(raw, 1e-9);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let world = in.world_position.xyz;
    let t = globals.time * water.wave_speed;
    let p = world.xz * water.wave_scale;

    let view_dir = normalize(view.world_position.xyz - world);

    // Fade wave detail with distance, so the far sea settles instead of shimmering.
    let camera_distance = length(view.world_position.xyz - world);
    let detail = clamp(1.0 - (camera_distance - 180.0) / 900.0, 0.12, 1.0);

    let normal = wave_normal(p, t, water.wave_strength, detail);
    let sun = normalize(water.sun.xyz);

    // Fresnel: water is nearly opaque underfoot and nearly a mirror at the horizon.
    // This one term does most of the work of making it read as water.
    let facing = clamp(dot(normal, view_dir), 0.0, 1.0);
    let fresnel = pow(1.0 - facing, 4.0);

    // Looking straight down you see into the water, so that is the clearest colour;
    // the body darkens as the view flattens and the path through it lengthens.
    var color = mix(water.deep.rgb, water.shallow.rgb, clamp(facing * 1.25, 0.0, 1.0));

    // Sky reflection at grazing angles.
    color = mix(color, water.sky.rgb, fresnel * 0.85);

    // Diffuse ripple shading, so the wave field is visible even out of the sun.
    let lambert = clamp(dot(normal, sun), 0.0, 1.0);
    color *= 0.82 + lambert * 0.28;

    // Broad specular sheen. Deliberately wide: a tight highlight on a surface this
    // large collapses into one blown-out blob.
    let half_vector = normalize(sun + view_dir);
    let specular = pow(clamp(dot(normal, half_vector), 0.0, 1.0), 48.0);
    color += water.sky.rgb * specular * water.specular * detail;

    // How much water sits between this fragment and whatever is behind it. This is
    // what makes it read as a liquid rather than a painted surface: shallows go
    // clear enough to show the sand, depths go opaque, and the shoreline gets a band
    // of foam where the two meet.
    let scene = linear_depth(prepass_depth(in.position, 0u));
    let surface = linear_depth(in.position.z);
    let thickness = max(scene - surface, 0.0);

    let submersion = clamp(thickness / max(water.depth_fade, 0.001), 0.0, 1.0);

    // Shallow water keeps the seabed's colour; deep water replaces it.
    var alpha = mix(0.0, water.shallow.a, submersion);

    // Foam where the water thins to nothing against the shore.
    let foam = 1.0 - clamp(thickness / max(water.foam_width, 0.001), 0.0, 1.0);
    let foam_band = smoothstep(0.35, 1.0, foam) * smoothstep(0.02, 0.25, submersion);
    color = mix(color, water.sky.rgb * 1.35, foam_band * 0.8);
    alpha = max(alpha, foam_band * 0.9);

    // Reflection is stronger at a glancing angle — but only where there is water to
    // reflect in. Applying it regardless forced the shallows opaque at exactly the
    // low angles a god camera uses, which drew a hard line along every shore instead
    // of letting the water thin away to nothing.
    alpha = mix(alpha, 1.0, fresnel * 0.85 * submersion);
    return vec4<f32>(color, clamp(alpha, 0.0, 1.0));
}
