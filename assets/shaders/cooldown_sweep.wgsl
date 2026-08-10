// The radial rest sweep: a dark wedge covering what remains of a
// miracle's cooldown, its edge turning clockwise from twelve like a
// clock hand as the power recovers - the grammar every action bar since
// WoW has taught. `remaining` is the fraction of the rest still owed;
// the uncovered window grows from twelve as it falls.

#import bevy_ui::ui_vertex_output::UiVertexOutput

struct Sweep {
    remaining: f32,
    shade: vec4<f32>,
}

@group(1) @binding(0) var<uniform> sweep: Sweep;

const TAU: f32 = 6.28318530718;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let centred = in.uv - vec2<f32>(0.5, 0.5);
    // The angle from twelve o'clock, clockwise, in turns of the face.
    var angle = atan2(centred.x, -centred.y);
    if (angle < 0.0) {
        angle = angle + TAU;
    }
    let turn = angle / TAU;
    // Everything the hand has not yet swept past stays under the shade.
    if (turn < 1.0 - sweep.remaining) {
        discard;
    }
    return sweep.shade;
}
