// The planet's own skin: ordinary ground, plus the fog of war laid over it
// WITHOUT the sun's help.
//
// At play height unknown ground is hidden by cloths — thin unlit sheets whose
// shader hands back its tint and that is the pixel. From orbit there is no
// cloth small enough, so the planet wears its fog in its patches instead. Paint
// the same colour into a LIT surface, though, and it does not come out the same
// colour: the sun's diffuse and its specular sheen both add to it, and the veil
// came out half again as far toward white and noticeably less blue than the
// cloths a few thousand units below. Worse, it would then shift with the light —
// one shade at midday, another at dusk — while the cloths never move.
//
// So the mix happens HERE, after the lighting, where the answer is exactly the
// cloth's colour under any sun at all. Which vertices are veiled rides in the
// vertex colour's ALPHA channel — the patch mesh is opaque, so nothing else
// wanted it.

#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
    forward_io::{VertexOutput, FragmentOutput},
}

struct VeilPaint {
    // rgb the veil's colour, a how much of it fully-veiled ground takes.
    tint: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> veil: VeilPaint;

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color =
        alpha_discard(pbr_input.material, pbr_input.material.base_color);

    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);

#ifdef VERTEX_COLORS
    // Alpha 1 is ground the village has walked; 0 is everything else.
    let veiled = (1.0 - in.color.a) * veil.tint.a;
    out.color = vec4<f32>(mix(out.color.rgb, veil.tint.rgb, veiled), out.color.a);
#endif

    return out;
}
