// The sky dome: horizon-to-zenith gradient, drifting clouds, and the sun's glow.
//
// The one hard requirement is the bottom edge: it must be exactly the fog
// colour, because fully-fogged terrain and the sky meet at the horizon and any
// difference draws a line there. Everything above that line is free.

#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::view

struct SkyParams {
    // What the fog fades to; the dome's colour at the horizon.
    horizon: vec4<f32>,
    // Overhead.
    zenith: vec4<f32>,
    // Sunlit cloud colour.
    cloud: vec4<f32>,
    // Toward the sun.
    sun_dir: vec4<f32>,
    // x: time, y: cloudiness 0..1, z: daylight 0..1, w: unused.
    misc: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> sky: SkyParams;

fn hash21(p: vec2<f32>) -> f32 {
    var q = fract(p * vec2<f32>(123.34, 456.21));
    q += dot(q, q + 45.32);
    return fract(q.x * q.y);
}

fn vnoise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash21(i);
    let b = hash21(i + vec2<f32>(1.0, 0.0));
    let c = hash21(i + vec2<f32>(0.0, 1.0));
    let d = hash21(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn fbm(p: vec2<f32>) -> f32 {
    var total = 0.0;
    var amplitude = 0.55;
    var q = p;
    for (var i = 0; i < 4; i++) {
        total += vnoise(q) * amplitude;
        q = q * 2.13 + vec2<f32>(17.0, 9.0);
        amplitude *= 0.5;
    }
    return total;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let dir = normalize(in.world_position.xyz - view.world_position);
    let elevation = dir.y;

    // The base gradient: fog colour in a thin band at the rim, real sky
    // shortly above it. Blue must arrive within a few degrees of the horizon,
    // because at god-camera angles a few degrees is all the sky that shows.
    var color = mix(
        sky.horizon.rgb,
        sky.zenith.rgb,
        smoothstep(0.015, 0.16, elevation),
    );

    // The sun: a tight disc inside a broad glow, faded out at night.
    let toward_sun = max(dot(dir, normalize(sky.sun_dir.xyz)), 0.0);
    let daylight = sky.misc.z;
    color += sky.cloud.rgb * pow(toward_sun, 900.0) * 4.0 * daylight;
    color += sky.horizon.rgb * pow(toward_sun, 7.0) * 0.20 * daylight;

    // The night: stars prick through as the daylight drains, and the moon
    // rides opposite the sun, so it rises as the sun sets.
    let night = 1.0 - smoothstep(0.05, 0.35, daylight);
    if (night > 0.0 && elevation > 0.02) {
        // Stars: a hash over a coarse celestial grid; only the brightest
        // cells light, so the field is sparse and steady. Twinkle is subtle.
        let cell = floor(dir.xz / (elevation + 0.15) * 42.0);
        let h = fract(sin(dot(cell, vec2<f32>(127.1, 311.7))) * 43758.5453);
        let sparkle = 0.85 + 0.15 * sin(sky.misc.x * (1.5 + h * 3.0) + h * 40.0);
        let star = smoothstep(0.985, 0.999, h) * sparkle;
        color += vec3<f32>(0.9, 0.93, 1.0) * star * night * 0.85;

        // The moon: a pale disc with a soft halo, anti-sunward.
        let moon_dir = normalize(vec3<f32>(-sky.sun_dir.x, abs(sky.sun_dir.y), -sky.sun_dir.z));
        let toward_moon = max(dot(dir, moon_dir), 0.0);
        let disc = smoothstep(0.9996, 0.99985, toward_moon);
        let halo = pow(toward_moon, 220.0) * 0.18;
        color += (vec3<f32>(0.86, 0.88, 0.92) * disc + vec3<f32>(0.6, 0.65, 0.78) * halo)
            * night;
    }

    // Clouds, projected onto a plane overhead so they flatten toward the
    // horizon the way real cloud decks do, drifting with time.
    if (elevation > 0.015) {
        let t = sky.misc.x;
        let uv = dir.xz / (elevation + 0.22) * 0.85
            + vec2<f32>(t * 0.0045, t * 0.0016);
        let density = fbm(uv);

        let cloudiness = sky.misc.y;
        let cover = smoothstep(1.05 - cloudiness, 1.28 - cloudiness, density);

        // Strongest in the mid-sky band; thinning near the rim and overhead.
        let band = smoothstep(0.02, 0.06, elevation)
            * (0.5 + 0.5 * (1.0 - smoothstep(0.35, 0.9, elevation)));

        // Sunlit tops against self-shadowed bellies, with a lift toward the sun.
        let belly = sky.cloud.rgb * 0.68;
        let cloud_color = mix(belly, sky.cloud.rgb, smoothstep(0.35, 0.8, density))
            + sky.cloud.rgb * pow(toward_sun, 4.0) * 0.18 * daylight;

        color = mix(color, cloud_color, cover * band * 0.92);
    }

    return vec4<f32>(color, 1.0);
}
