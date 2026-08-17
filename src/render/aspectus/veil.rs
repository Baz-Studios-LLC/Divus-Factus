//! The veil, as a pass over the whole frame rather than a coat of paint on
//! each material.
//!
//! WHY THIS EXISTS. The veil began as an object - a bank of cloth lifted over
//! the ground - and became a color, mixed into the ground material after its
//! lighting. That was the right move and it fixed a night's worth of faults,
//! but it left the veil as a property of MATERIALS, and a material has to opt
//! in. So the ground opted in, and then the groves, and then the boulders, the
//! bushes, the ore seams, the rivers and the sea, each one a separate fix
//! after somebody spotted it standing lit in the dark.
//!
//! And the ones nobody had spotted yet were the ones that matter most:
//! villagers, animals, and buildings all wear plain `StandardMaterial`. Walk
//! an explorer into unwalked country and they walk about lit; a wolf out there
//! is a lit wolf.
//!
//! A pass has no such hole. It reads the DEPTH the world just wrote,
//! reconstructs where each pixel actually is, asks the same question the
//! materials asked, and tints. Everything drawn into that depth is veiled by
//! construction - people, animals, roofs, the hand, anything ever added - with
//! no material to convert and no marker to remember.
//!
//! IT BLENDS ONTO THE FRAME; IT DOES NOT PING-PONG. The first version copied
//! the screen through `post_process_write()`, the way `fullscreen_material`
//! does - and painted the whole world black. The reason took an evening of
//! instrumented screenshots to corner. The god camera is HDR, so DEPTH OF
//! FIELD and TONEMAPPING both run as real passes here, and each one flips the
//! ping-pong's source/destination parity when it records - a parity that is
//! shared by every camera stacked on the window and survives from frame to
//! frame. This pass had an ordering edge against tonemapping and NONE against
//! depth of field, so where it fell against DoF was the schedule's whim - and
//! it fell wrong: its read and write straddled somebody else's flip, its
//! output landed in a texture the chain had already consumed, and the frame
//! that reached the screen was one nothing had finished writing.
//!
//! A tint needs none of that machinery. `mix(color, tint, w)` IS alpha
//! blending, so this pass draws one triangle straight onto the current main
//! texture with `LoadOp::Load` and an alpha-blend state, touching no parity at
//! all - the way bloom composites its glow. Cheaper, too: the frame is never
//! copied, and pixels the veil does not reach are never written. And it is
//! ordered BEFORE BLOOM, which makes the whole chain deterministic without
//! naming `depth_of_field` (which is pub(crate) and cannot be named): the
//! engine already orders bloom -> depth of field -> tonemapping, so the veil
//! goes veil -> bloom -> lens -> curve, every link an explicit edge.
//!
//! THE GROUND MATERIAL KEEPS ITS OWN VEIL. Not everything writes depth: the
//! water is drawn transparent over the seabed, and a pass keyed on depth would
//! veil the bed and leave the surface lit. The two are told from the same
//! knowledge in the same breath (`fog::follow_the_known`), so they can never
//! disagree about where the village has walked - where both apply, they mix
//! toward the same tint and the second mix changes nothing.

use std::any::type_name;

use bevy::core_pipeline::fullscreen_material::fullscreen_material_system;
use bevy::core_pipeline::schedule::Core3d;
use bevy::core_pipeline::tonemapping::tonemapping;
use bevy::core_pipeline::{Core3dSystems, FullscreenShader};
use bevy::post_process::bloom::bloom;
use bevy::ecs::error::BevyError;
use bevy::ecs::system::lifetimeless::Read;
use bevy::prelude::*;
use bevy::render::camera::ExtractedCamera;
use bevy::render::extract_component::{
    ComponentUniforms, DynamicUniformIndex, ExtractComponent, ExtractComponentPlugin,
    UniformComponentPlugin,
};
use bevy::render::render_resource::binding_types::{
    texture_depth_2d_multisampled, uniform_buffer,
};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice, ViewQuery};
use bevy::render::view::{
    ExtractedView, ViewDepthTexture, ViewTarget, ViewUniform, ViewUniformOffset, ViewUniforms,
};
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};

use crate::fog::FogParams;

/// Carried by the camera the veil is drawn for, and only while there is one to
/// draw - the pass runs whenever the component is present.
///
/// The god camera must be born with a readable depth buffer for this to bind -
/// see `camera.rs`, where `depth_texture_usages` carries `TEXTURE_BINDING`
/// from the first frame. Set later, the texture already exists without it and
/// the first bind group built against it quits the application.
#[derive(Component, ExtractComponent, ShaderType, Debug, Clone, Default)]
pub struct VeilView {
    pub params: FogParams,
}

pub struct VeilPass;

impl Plugin for VeilPass {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            ExtractComponentPlugin::<VeilView>::default(),
            UniformComponentPlugin::<VeilView>::default(),
        ));

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            // RenderStartup, not Startup. The render app has its own schedules
            // and no `Startup` of its own to speak of - registered there this
            // never ran, the pipeline resource never existed, and every system
            // below early-returned. A pass that does nothing logs exactly as
            // clean as a pass that works.
            .add_systems(RenderStartup, init_pipeline)
            .add_systems(
                Render,
                (
                    prepare_pipelines.in_set(RenderSystems::Prepare),
                    prepare_bind_groups.in_set(RenderSystems::PrepareBindGroups),
                ),
            )
            .add_systems(
                Core3d,
                // EVERY NEIGHBOR NAMED, NOTHING LEFT TO THE SCHEDULE'S WHIM.
                // Two systems in one set run in whatever order the schedule
                // felt like that frame, and on this HDR camera the neighbors
                // (depth of field, tonemapping) flip the ping-pong parity when
                // they record - an unordered pass lands its pixels in a
                // texture the chain has already consumed. Before bloom pins
                // the whole chain: the engine orders bloom -> depth of field
                // -> tonemapping itself, so the veil becomes the world's last
                // color before the lens and the curve, which is also what it
                // is artistically - part of the world, not an effect on it.
                //
                // And before the frost: the book's glass should blur veiled
                // ground, not veil a blurred world.
                veil_pass
                    .in_set(Core3dSystems::PostProcess)
                    .before(bloom)
                    .before(tonemapping)
                    .before(fullscreen_material_system::<super::Frost>),
            );
    }
}

#[derive(Resource)]
struct VeilPipeline {
    layout: BindGroupLayoutDescriptor,
    variants: Variants<RenderPipeline, VeilSpecializer>,
}

struct VeilSpecializer;

#[derive(PartialEq, Eq, Hash, Clone, Copy, SpecializerKey)]
struct VeilKey {
    target_format: TextureFormat,
}

impl Specializer<RenderPipeline> for VeilSpecializer {
    type Key = VeilKey;

    fn specialize(
        &self,
        key: Self::Key,
        descriptor: &mut RenderPipelineDescriptor,
    ) -> Result<Canonical<Self::Key>, BevyError> {
        // Drawn straight onto the lit frame: `mix(color, tint, w)` IS alpha
        // blending, so the blend state does the mixing and the pass never has
        // to read the screen at all.
        let fragment = descriptor.fragment_mut()?;
        fragment.set_target(
            0,
            ColorTargetState {
                format: key.target_format,
                blend: Some(BlendState::ALPHA_BLENDING),
                write_mask: ColorWrites::ALL,
            },
        );
        Ok(key)
    }
}

fn init_pipeline(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    fullscreen_shader: Res<FullscreenShader>,
) {
    let layout = BindGroupLayoutDescriptor::new(
        "aspectus_veil_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                // The depth the world wrote, which is what tells us WHERE each
                // pixel is. Read raw rather than sampled - averaging two
                // depths gives a position lying on neither surface.
                //
                // MULTISAMPLED, because the game takes Bevy's default MSAA and
                // never overrides it, so the depth buffer has four samples per
                // pixel. If MSAA is ever turned off this binding stops matching
                // and says so at once, loudly, in the validation log - which is
                // the good kind of breakage.
                texture_depth_2d_multisampled(),
                // The view, for the matrix that turns clip space back into the
                // world.
                uniform_buffer::<ViewUniform>(true),
                // And what the village knows.
                uniform_buffer::<VeilView>(true),
            ),
        ),
    );

    let desc = RenderPipelineDescriptor {
        label: Some(format!("aspectus_veil_pipeline<{}>", type_name::<VeilView>()).into()),
        layout: vec![layout.clone()],
        vertex: fullscreen_shader.to_vertex_state(),
        fragment: Some(FragmentState {
            shader: asset_server.load("shaders/aspectus_veil.wgsl"),
            targets: vec![Some(ColorTargetState {
                format: TextureFormat::Rgba8UnormSrgb,
                blend: Some(BlendState::ALPHA_BLENDING),
                write_mask: ColorWrites::ALL,
            })],
            ..default()
        }),
        ..default()
    };

    commands.insert_resource(VeilPipeline {
        layout,
        variants: Variants::new(VeilSpecializer, desc),
    });
}

#[derive(Component)]
struct VeilPipelineId(CachedRenderPipelineId);

fn prepare_pipelines(
    mut commands: Commands,
    pipeline_cache: Res<PipelineCache>,
    pipeline: Option<ResMut<VeilPipeline>>,
    views: Query<(Entity, &ExtractedView), (With<ExtractedCamera>, With<VeilView>)>,
) -> Result<(), BevyError> {
    let Some(mut pipeline) = pipeline else {
        return Ok(());
    };
    for (entity, view) in &views {
        let id = pipeline.variants.specialize(
            &pipeline_cache,
            VeilKey {
                target_format: view.target_format,
            },
        )?;
        commands.entity(entity).insert(VeilPipelineId(id));
    }
    Ok(())
}

/// The one bind group. No ping-pong: the pass blends onto the frame and never
/// reads it, so there is no source texture to track. What it DOES track is
/// every resource it binds: the depth texture is replaced on a resize, and
/// both uniform buffers are REALLOCATED whenever they grow - the view buffer
/// every time a camera is added (the portrait studio and the capture rigs
/// come and go). A bind group holding the old buffer while the offsets index
/// the new one overruns it, and the validation error quits the game.
#[derive(Component)]
struct VeilBindGroup {
    depth: TextureViewId,
    view_buffer: BufferId,
    veil_buffer: BufferId,
    bind_group: BindGroup,
}

fn prepare_bind_groups(
    mut commands: Commands,
    // FILTERED BY THE PASS'S OWN MARKER, and this is load-bearing. "Every view
    // gets the pass" is true of the drawing and false of the resources: an
    // unfiltered query reaches the hand camera, the portrait studio and the
    // capture rigs, none of which asked for readable depth, and the first bind
    // group built against an unreadable depth texture quits the application.
    mut views: Query<
        (Entity, &ViewDepthTexture, Option<&mut VeilBindGroup>),
        With<VeilView>,
    >,
    pipeline: Option<Res<VeilPipeline>>,
    pipeline_cache: Res<PipelineCache>,
    view_uniforms: Res<ViewUniforms>,
    veil_uniforms: Res<ComponentUniforms<VeilView>>,
    render_device: Res<RenderDevice>,
) {
    let (Some(pipeline), Some(view_binding), Some(veil_binding)) = (
        pipeline,
        view_uniforms.uniforms.binding(),
        veil_uniforms.uniforms().binding(),
    ) else {
        return;
    };
    let (Some(view_buffer), Some(veil_buffer)) = (
        view_uniforms.uniforms.buffer().map(|buffer| buffer.id()),
        veil_uniforms.uniforms().buffer().map(|buffer| buffer.id()),
    ) else {
        return;
    };

    for (entity, depth, existing) in &mut views {
        if let Some(group) = &existing
            && group.depth == depth.view().id()
            && group.view_buffer == view_buffer
            && group.veil_buffer == veil_buffer
        {
            continue;
        }
        let bind_group = render_device.create_bind_group(
            "aspectus_veil_bind_group",
            &pipeline_cache.get_bind_group_layout(&pipeline.layout),
            &BindGroupEntries::sequential((
                depth.view(),
                view_binding.clone(),
                veil_binding.clone(),
            )),
        );
        let made = VeilBindGroup {
            depth: depth.view().id(),
            view_buffer,
            veil_buffer,
            bind_group,
        };
        match existing {
            Some(mut group) => *group = made,
            None => {
                commands.entity(entity).insert(made);
            }
        }
    }
}

fn veil_pass(
    view: ViewQuery<(
        Read<ViewTarget>,
        Read<ViewUniformOffset>,
        Read<DynamicUniformIndex<VeilView>>,
        Read<VeilBindGroup>,
        Read<VeilPipelineId>,
    )>,
    pipeline_cache: Res<PipelineCache>,
    mut ctx: RenderContext,
) {
    let (target, view_offset, veil_index, group, pipeline_id) = view.into_inner();
    let Some(pipeline) = pipeline_cache.get_render_pipeline(pipeline_id.0) else {
        return;
    };

    // Straight onto the frame as it stands - loaded, blended into, and left
    // where the next pass expects it. UNSAMPLED, deliberately: with MSAA on,
    // the multisampled texture no longer matches the resolved frame once
    // later passes composite into the latter, and drawing through a resolve
    // would wipe them back out. Bloom composites through this same unsampled
    // attachment.
    let mut pass = ctx
        .command_encoder()
        .begin_render_pass(&RenderPassDescriptor {
            label: Some("aspectus_veil"),
            color_attachments: &[Some(target.get_unsampled_color_attachment())],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, &group.bind_group, &[view_offset.offset, veil_index.index()]);
    pass.draw(0..3, 0..1);
}
