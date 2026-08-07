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
/// TWO long, slow swells, rotated by the golden angle so their crests never line
/// up. Not an ocean simulation - a swell this game can actually show.
///
/// It was three octaves plus a noise tap at a fourteen-metre wavelength, which is
/// a real sea surface and exactly the wrong thing here. Everything is looked at
/// from a god's camera hundreds or thousands of units up, where fourteen metres
/// is a fraction of a pixel: detail that small cannot be SEEN, only aliased, and
/// every water fault of the last few hours was that same field beating against
/// the pixel grid in a different disguise - moire rings from altitude, a hard
/// line across open water where the fade cut out, a seam at the streamed edge.
///
/// A hundred-odd unit swell is many pixels wide from any height the game is
/// played at, so there is nothing left to alias and nothing to fade. It also
/// suits the world it is in: everything else here is flat-shaded and low-poly,
/// and a finely detailed sea in the middle of that reads as borrowed.
///
/// Sine waves
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

    for (var i = 0; i < 2; i = i + 1) {
        total += sin(dot(q, vec2<f32>(0.82, 0.57)) + t * (1.0 + f32(i) * 0.37)) * amp;
        norm += amp;
        amp *= 0.5;
        q = rot * q * 1.6;
    }

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
    // The early-out is a saving, not a fade, and it has to land somewhere the
    // waves are ALREADY gone or it draws a line across the sea.
    //
    // It used to cut at a fifth of full detail while the amplitude below was
    // scaled by `detail` straight - so at the cut the waves still had a fifth
    // of their height, and one pixel further they had none. `detail` falls off
    // with range, so that step is a contour of constant distance from the
    // camera: a hard edge drawn clean across open water with nothing on either
    // side of it to explain itself.
    //
    // Squared below, the amplitude at this threshold is a twenty-fifth rather
    // than a fifth, which is nothing to see, and the saving is kept.
    if detail < 0.12 {
        return vec3<f32>(0.0, 1.0, 0.0);
    }

    let e = 0.35;
    let dx = wave_height(p + vec2<f32>(e, 0.0), t) - wave_height(p - vec2<f32>(e, 0.0), t);
    let dz = wave_height(p + vec2<f32>(0.0, e), t) - wave_height(p - vec2<f32>(0.0, e), t);
    let s = strength * detail * detail;
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

    // Fade wave detail by how big a wave is ON SCREEN, not by how far away it
    // is. Same rule the planet's quadtree splits on, and for the same reason:
    // a pixel threshold is automatically right at every altitude, every field
    // of view and every window size, where a distance in metres is hand-tuned
    // for one of them and wrong for the rest.
    //
    // It was a distance, and both ways of being wrong turned up within an hour
    // of each other. Faded over nine hundred units, the sea went flat about
    // where the streamed ground ends, so from a low camera the shader appeared
    // to give up near the shore. Stretched to five thousand to fix that, waves
    // a few centimetres across on screen survived out to the horizon and beat
    // against the pixel grid - which is not ripples at all but MOIRE, and it
    // curves, so the ocean from altitude wore circular arcs nothing in the
    // world put there.
    //
    // Below a couple of pixels a wave cannot be drawn, only aliased. Above ten
    // or so it is worth every cycle it costs.
    let camera_distance = length(view.world_position.xyz - world);
    let wavelength = 6.2831853 / max(water.wave_scale, 0.0001);
    // Perspective: clip_from_view[1][1] is cot(fov/2), so this is how many
    // pixels one world unit covers at this range.
    let px_per_unit =
        view.viewport.w * view.clip_from_view[1][1] * 0.5 / max(camera_distance, 1.0);
    let detail = smoothstep(2.5, 12.0, wavelength * px_per_unit);

    // The wave field is computed flat, around +Y, and then stood up on the
    // surface it is actually lying on.
    //
    // It used to be used as it came out, which quietly assumed the world was a
    // plane with +Y for up. It very nearly is - at the point where the flat
    // scaffold touches the globe - and that is where the streamed chunks are,
    // so their sea looked right and the planet's did not. The boundary between
    // the two was the streaming edge: ripples and glitter on one side of it,
    // dead flat water on the other, with a straight diagonal join.
    //
    // The tangent is world +X flattened onto the surface, which keeps the wave
    // field lined up with the same axes `p` is sampled in, so the pattern runs
    // on unbroken across the join instead of twisting at it.
    let up = normalize(in.world_normal);
    let ripple = wave_normal(p, t, water.wave_strength, detail);
    let tangent = normalize(vec3<f32>(1.0, 0.0, 0.0) - up * up.x);
    let bitangent = cross(up, tangent);
    let normal = normalize(tangent * ripple.x + up * ripple.y + bitangent * ripple.z);
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

    // Specular sheen, and the one place the distance fade must NOT simply take
    // things away.
    //
    // The sea's whole look is specular, so flattening its normals with range -
    // which is the only way to stop them aliasing - takes the sun off the water
    // with them, and the far ocean goes dead grey. That was the flat band on
    // the horizon. Tessendorf's answer, and LEAN mapping's: waves too small to
    // draw are not GONE, they are roughness. Hand the lost detail to the width
    // of the lobe instead of subtracting it from the light.
    //
    // So the highlight broadens as the surface flattens, and the strength is
    // not scaled down at all - the same light, spread over more sea. Near to,
    // that is glitter on individual crests; far off, it is the wide soft path
    // the sun lays across water, which is what the eye actually reads at range.
    let half_vector = normalize(sun + view_dir);
    let sharpness = mix(5.0, 48.0, detail);
    let specular = pow(clamp(dot(normal, half_vector), 0.0, 1.0), sharpness);
    color += water.sky.rgb * specular * water.specular;

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

    // No whitecaps. Foam breaking off crests is an ocean's detail, and this
    // world is flat-shaded and low-poly: from any height the game is played at
    // they were not foam but a pattern, and from orbit they banded the whole
    // planet in arcs. What reads as water at this resolution is the COLOUR -
    // shallow against deep, a foam line at the shore, and the sun on it.
    // Reflection is stronger at a glancing angle — but only where there is water to
    // reflect in. Applying it regardless forced the shallows opaque at exactly the
    // low angles a god camera uses, which drew a hard line along every shore instead
    // of letting the water thin away to nothing.
    alpha = mix(alpha, 1.0, fresnel * 0.85 * submersion);
    return vec4<f32>(color, clamp(alpha, 0.0, 1.0));
}
