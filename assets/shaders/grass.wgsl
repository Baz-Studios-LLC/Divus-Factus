// Wind for grass blades.
//
// Only the vertex stage lives here; lighting, shadows and fog stay the standard
// pipeline's, because this is an extension on StandardMaterial rather than a
// material of its own. The blades bend — everything else is stock.
//
// Each vertex carries its bend weight in uv.x: 0 at the root, 1 at the tip. The
// root of a blade never moves, which is what keeps a field of swaying grass from
// reading as a field sliding around.

#import bevy_pbr::{
    mesh_functions,
    forward_io::{Vertex, VertexOutput},
    mesh_view_bindings::view,
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

    // Two broad gusts at incommensurate headings plus a per-blade flutter. The
    // gusts travel, so wind visibly moves *across* a meadow rather than the whole
    // field rocking in unison.
    let weight = vertex.uv.x;
    let phase = vertex.uv.y * 6.28318;
    let t = grass.clock.x * grass.wind.z;

    let gust = sin(dot(world_position.xz, vec2(0.045, 0.038)) + t) * 0.7
        + sin(dot(world_position.xz, vec2(-0.021, 0.052)) + t * 0.63 + 1.7) * 0.4;
    let flutter = sin(t * 2.6 + phase) * 0.14;

    // The blade leans rather than shears: the sum is an angle about the root,
    // and each vertex swings on its own height up the blade (weight against a
    // nominal stature). sin carries it sideways while 1 - cos settles it, so
    // no gust can lengthen a blade — hard wind lays grass over instead of
    // stretching it, and the tips stop pogoing on the old abs() drop.
    let angle = clamp((gust + flutter) * grass.wind.w, -1.1, 1.1);
    let reach = weight * 0.3;
    let lean = sin(angle) * reach;

    world_position.x += grass.wind.x * lean;
    world_position.z += grass.wind.y * lean;
    world_position.y -= (1.0 - cos(angle)) * reach;

    out.world_position = world_position;
    out.position = view.clip_from_world * world_position;
    out.world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex.instance_index);
#ifdef VERTEX_UVS_A
    out.uv = vertex.uv;
#endif
#ifdef VERTEX_COLORS
    out.color = vertex.color;
#endif
#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    out.instance_index = vertex.instance_index;
#endif
    return out;
}
