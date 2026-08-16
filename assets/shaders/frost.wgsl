// ASPECTUS: the frosted glass behind an open book.
//
// The first pass this game owns outright. Everything else it draws goes
// through Bevy's pipeline with our materials hung on it; this runs in the
// render graph as a pass of ours, over the whole screen, after the world is
// drawn and before it is tonemapped.
//
// WHAT IT REPLACES. Opening the book used to wake a second camera, which
// rendered the entire world again - every chunk, every tree, every villager -
// into a 480x270 image with a Gaussian depth-of-field on it, and a fullscreen
// UI quad stretched that image back over the screen. The image was small, so
// the fill cost was nothing; the SCENE was not, and it was submitted twice for
// as long as the book stayed open.
//
// Here the world is already drawn. Blurring what is on the screen costs a
// handful of taps and no geometry at all, and because it is the real screen
// rather than a 480-wide copy, the glass is sharp where it should be and the
// blur is a blur rather than a stretch.
//
// The taps are a spiral rather than a box: a box of this radius needs its
// samples squared to stop banding, and a spiral spreads a fixed handful over
// the disc so the smear reads as ground glass instead of a grid.

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

struct Frost {
    // How much of the blur is mixed over the sharp image, 0 to 1. The book
    // fades this up as it opens rather than snapping the world out of focus.
    strength: f32,
    // The blur's reach, in fractions of the screen's width.
    radius: f32,
    // The screen's shape, so a circular blur stays circular on a wide window.
    aspect: f32,
    _pad: f32,
}

@group(0) @binding(0) var screen: texture_2d<f32>;
@group(0) @binding(1) var screen_sampler: sampler;
@group(0) @binding(2) var<uniform> frost: Frost;

/// Sixteen points on a golden-angle spiral. Evenly spread over the disc by
/// construction, with no direction preferred - which is what keeps a heavy
/// blur from growing the star-shaped seams a fixed cross or box gives.
const TAPS: i32 = 16;
const GOLDEN_ANGLE: f32 = 2.39996323;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let sharp = textureSample(screen, screen_sampler, in.uv);
    if frost.strength < 0.002 || frost.radius < 0.00001 {
        return sharp;
    }

    var blurred = sharp.rgb;
    var weight = 1.0;
    for (var i = 0; i < TAPS; i = i + 1) {
        let step = f32(i) + 1.0;
        // Radius grows as the square root of the index, so the points sit at
        // equal AREA rather than equal spacing and the disc is covered evenly
        // instead of crowding the middle.
        let reach = frost.radius * sqrt(step / f32(TAPS));
        let angle = step * GOLDEN_ANGLE;
        let offset = vec2<f32>(cos(angle) * reach / frost.aspect, sin(angle) * reach);
        // Clamped to the screen: a tap that walks off the edge would otherwise
        // wrap or repeat, and either one draws a bright seam along the border
        // exactly where a book's margin sits.
        let at = clamp(in.uv + offset, vec2<f32>(0.0), vec2<f32>(1.0));
        blurred = blurred + textureSample(screen, screen_sampler, at).rgb;
        weight = weight + 1.0;
    }
    blurred = blurred / weight;

    return vec4<f32>(mix(sharp.rgb, blurred, clamp(frost.strength, 0.0, 1.0)), sharp.a);
}
