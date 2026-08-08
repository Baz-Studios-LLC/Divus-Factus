// The weather deck: one shell of cloud wrapped round the whole planet.
//
// There is a single cloud layer in this world and both views of it are the same
// object. From the ground you are INSIDE the shell looking up at its underside,
// and it reads as an overcast drifting past. From orbit you are outside it
// looking down, and it reads as weather lying over the continents. Nothing is
// duplicated, nothing has to be kept in step, and a cloud you watched go over
// the village is the same cloud you find on the ball when you pull back.
//
// The field is 3D value noise sampled on the DIRECTION from the planet's centre,
// exactly the way the terrain's own field is built — which is the only
// construction with no seam anywhere and no crowding at the poles. Wind is a
// slow rotation of that sample direction about the planet's axis, so the deck
// travels the world instead of sliding across a projection of it.

#import bevy_pbr::forward_io::VertexOutput

struct CloudParams {
    // rgb the lit cloud colour, a the deck's greatest opacity.
    tint: vec4<f32>,
    // rgb the colour of cloud the sun has left, a unused.
    shade: vec4<f32>,
    // xyz the direction toward the sun, w how much daylight there is.
    sun: vec4<f32>,
    // xyz the planet's axis, w how far the deck has turned on it.
    wind: vec4<f32>,
    // x coverage 0..1, y the noise scale, z evolution clock, w edge softness.
    dials: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> cloud: CloudParams;

fn hash3(p: vec3<f32>) -> f32 {
    // Integer-lattice hash. Cheap, and stable enough that the deck does not
    // crawl with floating-point noise between frames.
    let q = fract(p * 0.3183099 + vec3<f32>(0.1, 0.2, 0.3));
    let r = q * 17.0;
    return fract(r.x * r.y * r.z * (r.x + r.y + r.z));
}

fn value3(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = p - i;
    // Smoothstep, so the field has no creases.
    let u = f * f * (3.0 - 2.0 * f);

    let c000 = hash3(i + vec3<f32>(0.0, 0.0, 0.0));
    let c100 = hash3(i + vec3<f32>(1.0, 0.0, 0.0));
    let c010 = hash3(i + vec3<f32>(0.0, 1.0, 0.0));
    let c110 = hash3(i + vec3<f32>(1.0, 1.0, 0.0));
    let c001 = hash3(i + vec3<f32>(0.0, 0.0, 1.0));
    let c101 = hash3(i + vec3<f32>(1.0, 0.0, 1.0));
    let c011 = hash3(i + vec3<f32>(0.0, 1.0, 1.0));
    let c111 = hash3(i + vec3<f32>(1.0, 1.0, 1.0));

    let near = mix(mix(c000, c100, u.x), mix(c010, c110, u.x), u.y);
    let far = mix(mix(c001, c101, u.x), mix(c011, c111, u.x), u.y);
    return mix(near, far, u.z);
}

fn fbm3(p: vec3<f32>) -> f32 {
    var sum = 0.0;
    var amp = 0.5;
    var at = p;
    for (var i = 0; i < 5; i = i + 1) {
        sum = sum + value3(at) * amp;
        at = at * 2.03;
        amp = amp * 0.5;
    }
    return sum;
}

/// Rotates `v` about `axis` by `angle`, so the deck can travel the planet.
fn spin(v: vec3<f32>, axis: vec3<f32>, angle: f32) -> vec3<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return v * c + cross(axis, v) * s + axis * dot(axis, v) * (1.0 - c);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // A deck the god has climbed above is faded out entirely, and a fully
    // transparent shell is still a full-screen pass of two five-octave fields
    // if it is allowed to get that far. Out here, before any of it. See
    // `clouds::deck_opacity`.
    if (cloud.tint.a <= 0.002) {
        discard;
    }

    // The shell's own outward normal IS the direction from the planet's centre,
    // which is the only coordinate this field needs. Seen from inside, that
    // normal points away from the eye; the direction is the same either way.
    let dir = normalize(in.world_normal);

    // Carried round the world by the wind, and slowly reshaped in place, so a
    // deck that has travelled is not the same deck it was.
    let carried = spin(dir, normalize(cloud.wind.xyz), cloud.wind.w);
    let scale = cloud.dials.y;
    let drift = vec3<f32>(0.0, cloud.dials.z * 0.03, cloud.dials.z * 0.017);
    var field = fbm3(carried * scale + drift);

    // A second, far larger field decides WHERE there is weather at all. It has
    // to GATE the fine one and not merely tilt it: multiplied in as a weighting,
    // every part of the sky still had some cloud in it and the planet came out
    // under an even white mottle from pole to pole. Gated, there are fronts —
    // stretches of thick weather and stretches of open sky between them, which
    // is what a world looks like from up there.
    // The gate opens as the weather thickens: on a fair day it leaves most of
    // the sky clear and gives the cloud somewhere to be, and in a storm it
    // stands wide so the deck can close over the world entirely — which a fixed
    // gate cannot, since it holds some of the sky empty by construction.
    let cover = cloud.dials.x;
    let banks = fbm3(carried * (scale * 0.18) + vec3<f32>(11.3, 4.7, 8.1));
    field = field * smoothstep(mix(0.42, 0.02, cover), mix(0.66, 0.20, cover), banks);

    // Coverage cuts the field: the threshold falls as the weather thickens, so
    // fair days keep scattered cloud and a storm closes over completely.
    //
    // The range is MEASURED, not dialled. Sampling this exact field over four
    // thousand directions: five octaves sit at 0.48, the gate takes the median
    // to 0.27, and the fraction of sky above a cut runs 0.39 → 30%, 0.46 → 18%,
    // 0.55 → 7%. Guessing at it put the whole planet under an unbroken white
    // mottle twice over.
    let cut = mix(0.50, 0.14, cover);
    let soft = cloud.dials.w;
    var density = smoothstep(cut, cut + soft, field);
    if (density <= 0.001) {
        discard;
    }

    // Lit by the real sun, from where it really is: the deck's day side is
    // bright, its night side is the colour of cloud with no sun on it, and at
    // dawn and dusk the terminator runs across the weather as well as the
    // ground.
    //
    // The lit side saturates WELL before the sub-solar point. Ramping it across
    // the whole hemisphere, the way a diffuse surface would, left most of the
    // day side part-shaded and the clouds read as grey — but cloud is not a
    // matte ball, it is a scattering medium, and a cloud in sunlight is white
    // whether the sun is overhead or off to one side. Only the terminator and
    // beyond it are allowed to darken.
    let facing = dot(dir, normalize(cloud.sun.xyz));
    let day = clamp((facing + 0.12) / 0.34, 0.0, 1.0);
    let color = mix(cloud.shade.rgb, cloud.tint.rgb, day);

    return vec4<f32>(color, density * cloud.tint.a);
}
