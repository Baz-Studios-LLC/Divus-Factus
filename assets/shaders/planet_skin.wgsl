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
    //
    // And the SAME LAW as the cloths, not merely the same colour. The cloths are
    // a Beer-Lambert slab: looked at squarely they take nine parts in ten, and a
    // grazing look travels further through the same sheet and comes out heavier,
    // up to six times the thickness (see `fog.wgsl`). The patches took a flat
    // nine tenths at every angle, so wherever the two met at an oblique view -
    // which is most of the time, from orbit - the near ring was measurably
    // darker than the planet beyond it. Brett: "the real veil and the global
    // veil dont match in color or maybe transparency."
    //
    // Both come to the same thing once the depth cancels out. The cloth's
    // density is 1 - exp(-extinction * travelled) with extinction
    // -ln(1 - a) / depth and travelled min(depth / facing, depth * 6), and every
    // depth in that divides out:
    //
    //     k = min(1 / facing, 6)        density = 1 - (1 - a)^k
    //
    // which needs no sheet and no thickness, only the angle. Straight on, k is 1
    // and it is nine tenths again, exactly as before.
    let facing = max(abs(dot(normalize(pbr_input.world_normal), pbr_input.V)), 0.0001);
    let thickening = min(1.0 / facing, 6.0);
    let density = 1.0 - pow(max(1.0 - veil.tint.a, 0.002), thickening);
    let veiled = (1.0 - in.color.a) * density;
    out.color = vec4<f32>(mix(out.color.rgb, veil.tint.rgb, veiled), out.color.a);
#endif

    return out;
}
