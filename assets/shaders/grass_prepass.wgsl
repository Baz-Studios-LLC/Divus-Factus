// Prepass counterpart of the grass wind.
//
// The depth prepass renders every mesh, and it renders with its own vertex stage.
// When the main pass bends a blade that the prepass left straight, their depths
// disagree — and every mismatched pixel becomes a blade-shaped hole showing the
// sky through the meadow. The wind below is a byte-for-byte copy of the main
// pass's: the two stages must agree exactly, forever.

#import bevy_pbr::{
    mesh_functions,
    prepass_io::{Vertex, VertexOutput},
    view_transformations::position_world_to_clip,
}

struct GrassParams {
    // xy: wind direction. z: speed. w: strength.
    wind: vec4<f32>,
    // x: time. The prepass has no global clock, so the wind carries its own —
    // which also guarantees both passes bend from the identical instant.
    clock: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> grass: GrassParams;

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;

    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    var world_position =
        mesh_functions::mesh_position_local_to_world(world_from_local, vec4(vertex.position, 1.0));

    // Identical to the main pass. Any drift reopens the holes.
    let weight = vertex.uv.x;
    let phase = vertex.uv.y * 6.28318;
    let t = grass.clock.x * grass.wind.z;

    let gust = sin(dot(world_position.xz, vec2(0.045, 0.038)) + t) * 0.7
        + sin(dot(world_position.xz, vec2(-0.021, 0.052)) + t * 0.63 + 1.7) * 0.4;
    let flutter = sin(t * 2.6 + phase) * 0.22;
    let bend = (gust + flutter) * grass.wind.w * weight;

    world_position.x += grass.wind.x * bend;
    world_position.z += grass.wind.y * bend;
    world_position.y -= abs(bend) * 0.35 * weight;

    out.world_position = world_position;
    out.position = position_world_to_clip(world_position.xyz);

#ifdef VERTEX_UVS_A
    out.uv = vertex.uv;
#endif
#ifdef NORMAL_PREPASS_OR_DEFERRED_PREPASS
#ifdef VERTEX_NORMALS
    out.world_normal =
        mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex.instance_index);
#endif
#endif
#ifdef VERTEX_COLORS
    out.color = vertex.color;
#endif
    return out;
}
