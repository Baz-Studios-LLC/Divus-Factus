//! Ground fog, as a pass: mist marched through, rather than washed over.
//!
//! Where the veil asks one question per pixel - is this ground known - this
//! asks sixteen, along the ray from the eye to whatever the pixel shows, and
//! adds up the wet air between. That is the whole difference between mist that
//! is IN the world and a filter laid over it: a marched mist fills the bottom
//! of a valley and leaves the ridge standing out of it, and looking along a
//! hollow shows more of it than looking across one.
//!
//! It matters here more than in most games, because this game DELETED its
//! distance fog on purpose - a round world hides its own distance over the
//! horizon, so haze-with-distance is the one thing ground fog must not become.
//!
//! Everything structural is the veil's, and deliberately so - see `veil.rs` for
//! the evening that bought it. In particular: it blends onto the frame with
//! `LoadOp::Load` and never touches `post_process_write()`, it names every
//! neighbor it must run against, its bind group tracks every buffer it binds,
//! and its prepare systems filter on its own marker so the hand camera and the
//! capture rigs never build one.
//!
//! ORDERED BEFORE THE VEIL, which is the one piece of real thinking on top.
//! Mist drawn after the veil would sit on top of unwalked country and describe
//! the shape of land the village has never seen - a valley the player could
//! read off the fog lying in it. Drawn first, the veil paints over it, and
//! unknown country stays unknown for free.

use std::any::type_name;

use bevy::core_pipeline::fullscreen_material::fullscreen_material_system;
use bevy::core_pipeline::schedule::Core3d;
use bevy::core_pipeline::tonemapping::tonemapping;
use bevy::core_pipeline::{Core3dSystems, FullscreenShader};
use bevy::ecs::error::BevyError;
use bevy::ecs::system::lifetimeless::Read;
use bevy::post_process::bloom::bloom;
use bevy::prelude::*;
use bevy::render::camera::ExtractedCamera;
use bevy::render::extract_component::{
    ComponentUniforms, DynamicUniformIndex, ExtractComponent, ExtractComponentPlugin,
    UniformComponentPlugin,
};
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_resource::binding_types::{
    sampler, texture_2d, texture_depth_2d_multisampled, uniform_buffer,
};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice, ViewQuery};
use bevy::render::texture::GpuImage;
use bevy::render::view::{
    ExtractedView, ViewDepthTexture, ViewTarget, ViewUniform, ViewUniformOffset, ViewUniforms,
};
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};

/// What the mist pass needs to know, packed for the shader.
///
/// ITS OWN STRUCT, not a few more fields on `FogParams`. That uniform is worn
/// by the ground material as well, and it carries a hundred and twenty-eight
/// pockets - every field added to it is paid for on every acre of ground in the
/// world, to tell it something about weather it does not need to know.
#[derive(Component, ExtractComponent, ShaderType, Debug, Clone, Default)]
pub struct MistView {
    /// rgb the mist's color facing AWAY from the sun, a its overall strength.
    pub tint: Vec4,
    /// rgb the mist's color facing INTO the sun, a how low the sun is.
    ///
    /// Two colors and not one, because that difference is the whole perception
    /// of morning: look across a valley into a low sun and the mist blows out
    /// gold, turn around and it is cool and flat. One color in every direction
    /// is the signature of a screen filter rather than of weather.
    pub sunward: Vec4,
    /// xyz the planet's center, w its radius.
    pub planet: Vec4,
    /// xy the field's low corner in flat sim coords, z its span, w how deep
    /// the mist lies over its ground.
    pub field: Vec4,
    /// x how far the march reaches, y the full height range the field's green
    /// channel spans, z the debug view, w the most the mist may ever hide.
    pub dials: Vec4,
    /// xyz toward the sun, w spare.
    pub sun: Vec4,
}

/// The baked field, carried to the render world beside the uniform.
///
/// A handle rather than the image: the render world already holds every image
/// as a `GpuImage`, and asking for it by handle is how a pass borrows one
/// without owning it.
#[derive(Component, ExtractComponent, Clone)]
pub struct MistFieldImage(pub Handle<Image>);

pub struct MistPass;

impl Plugin for MistPass {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            ExtractComponentPlugin::<MistView>::default(),
            ExtractComponentPlugin::<MistFieldImage>::default(),
            UniformComponentPlugin::<MistView>::default(),
        ));

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
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
                // BEFORE THE VEIL - the one ordering decision that is about the
                // game rather than about the renderer. See the header.
                //
                // And before bloom, which pins the rest of the chain: the
                // engine orders bloom -> depth of field -> tonemapping itself,
                // so mist is part of the world's color, taken by the lens and
                // the curve along with everything else.
                mist_pass
                    .in_set(Core3dSystems::PostProcess)
                    .before(super::veil::veil_pass)
                    .before(bloom)
                    .before(tonemapping)
                    .before(fullscreen_material_system::<super::Frost>),
            );
    }
}

/// PREMULTIPLIED, not straight alpha, and this is an art decision wearing a
/// blend state.
///
/// Straight `SrcAlpha/OneMinusSrcAlpha` is exactly `mix(scene, mist, a)`, which
/// means the very brightest a mist pixel can ever be is the mist's own color -
/// a constant. But mist near a low sun IS brighter than mist away from it; that
/// difference is what makes a morning read as a morning, and a blend that caps
/// it deletes the effect no matter what the shader computes. Premultiplied lets
/// the shader hand over light that is brighter than its own tint, which the
/// bloom pass downstream then catches as glow.
fn mist_blend(format: TextureFormat) -> ColorTargetState {
    ColorTargetState {
        format,
        blend: Some(BlendState {
            color: BlendComponent {
                src_factor: BlendFactor::One,
                dst_factor: BlendFactor::OneMinusSrcAlpha,
                operation: BlendOperation::Add,
            },
            alpha: BlendComponent {
                src_factor: BlendFactor::One,
                dst_factor: BlendFactor::OneMinusSrcAlpha,
                operation: BlendOperation::Add,
            },
        }),
        write_mask: ColorWrites::ALL,
    }
}

#[derive(Resource)]
struct MistPipeline {
    layout: BindGroupLayoutDescriptor,
    field_sampler: Sampler,
    variants: Variants<RenderPipeline, MistSpecializer>,
}

struct MistSpecializer;

#[derive(PartialEq, Eq, Hash, Clone, Copy, SpecializerKey)]
struct MistKey {
    target_format: TextureFormat,
}

impl Specializer<RenderPipeline> for MistSpecializer {
    type Key = MistKey;

    fn specialize(
        &self,
        key: Self::Key,
        descriptor: &mut RenderPipelineDescriptor,
    ) -> Result<Canonical<Self::Key>, BevyError> {
        let fragment = descriptor.fragment_mut()?;
        fragment.set_target(0, mist_blend(key.target_format));
        Ok(key)
    }
}

fn init_pipeline(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    asset_server: Res<AssetServer>,
    fullscreen_shader: Res<FullscreenShader>,
) {
    let layout = BindGroupLayoutDescriptor::new(
        "aspectus_mist_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_depth_2d_multisampled(),
                uniform_buffer::<ViewUniform>(true),
                uniform_buffer::<MistView>(true),
                // The baked field, and a filtering sampler for it: the mist is
                // soft and its field is coarse, so the interpolation between
                // cells is doing real work rather than hiding sloppiness.
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
            ),
        ),
    );

    let desc = RenderPipelineDescriptor {
        label: Some(format!("aspectus_mist_pipeline<{}>", type_name::<MistView>()).into()),
        layout: vec![layout.clone()],
        vertex: fullscreen_shader.to_vertex_state(),
        fragment: Some(FragmentState {
            shader: asset_server.load("shaders/aspectus_mist.wgsl"),
            targets: vec![Some(mist_blend(TextureFormat::Rgba8UnormSrgb))],
            ..default()
        }),
        ..default()
    };

    commands.insert_resource(MistPipeline {
        layout,
        field_sampler: render_device.create_sampler(&SamplerDescriptor {
            label: Some("aspectus_mist_field_sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..default()
        }),
        variants: Variants::new(MistSpecializer, desc),
    });
}

#[derive(Component)]
pub(super) struct MistPipelineId(CachedRenderPipelineId);

fn prepare_pipelines(
    mut commands: Commands,
    pipeline_cache: Res<PipelineCache>,
    pipeline: Option<ResMut<MistPipeline>>,
    views: Query<(Entity, &ExtractedView), (With<ExtractedCamera>, With<MistView>)>,
) -> Result<(), BevyError> {
    let Some(mut pipeline) = pipeline else {
        return Ok(());
    };
    for (entity, view) in &views {
        let id = pipeline.variants.specialize(
            &pipeline_cache,
            MistKey {
                target_format: view.target_format,
            },
        )?;
        commands.entity(entity).insert(MistPipelineId(id));
    }
    Ok(())
}

/// Tracks every resource it binds - the depth texture, both uniform buffers,
/// and the field image, which is replaced wholesale each time a bake lands.
#[derive(Component)]
pub(super) struct MistBindGroup {
    depth: TextureViewId,
    view_buffer: BufferId,
    mist_buffer: BufferId,
    field: TextureViewId,
    bind_group: BindGroup,
}

fn prepare_bind_groups(
    mut commands: Commands,
    mut views: Query<
        (
            Entity,
            &ViewDepthTexture,
            &MistFieldImage,
            Option<&mut MistBindGroup>,
        ),
        With<MistView>,
    >,
    pipeline: Option<Res<MistPipeline>>,
    pipeline_cache: Res<PipelineCache>,
    view_uniforms: Res<ViewUniforms>,
    mist_uniforms: Res<ComponentUniforms<MistView>>,
    images: Res<RenderAssets<GpuImage>>,
    render_device: Res<RenderDevice>,
) {
    let (Some(pipeline), Some(view_binding), Some(mist_binding)) = (
        pipeline,
        view_uniforms.uniforms.binding(),
        mist_uniforms.uniforms().binding(),
    ) else {
        return;
    };
    let (Some(view_buffer), Some(mist_buffer)) = (
        view_uniforms.uniforms.buffer().map(|buffer| buffer.id()),
        mist_uniforms.uniforms().buffer().map(|buffer| buffer.id()),
    ) else {
        return;
    };

    for (entity, depth, wanted, existing) in &mut views {
        // The field may not have reached the render world yet on the first
        // frames. No field, no pass - rather than a bind group against a
        // fallback image that would paint mist over the whole world.
        let Some(field) = images.get(&wanted.0) else {
            continue;
        };
        if let Some(group) = &existing
            && group.depth == depth.view().id()
            && group.view_buffer == view_buffer
            && group.mist_buffer == mist_buffer
            && group.field == field.texture_view.id()
        {
            continue;
        }
        let bind_group = render_device.create_bind_group(
            "aspectus_mist_bind_group",
            &pipeline_cache.get_bind_group_layout(&pipeline.layout),
            &BindGroupEntries::sequential((
                depth.view(),
                view_binding.clone(),
                mist_binding.clone(),
                &field.texture_view,
                &pipeline.field_sampler,
            )),
        );
        let made = MistBindGroup {
            depth: depth.view().id(),
            view_buffer,
            mist_buffer,
            field: field.texture_view.id(),
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

pub(super) fn mist_pass(
    view: ViewQuery<(
        Read<ViewTarget>,
        Read<ViewUniformOffset>,
        Read<DynamicUniformIndex<MistView>>,
        Read<MistBindGroup>,
        Read<MistPipelineId>,
    )>,
    pipeline_cache: Res<PipelineCache>,
    mut ctx: RenderContext,
) {
    let (target, view_offset, mist_index, group, pipeline_id) = view.into_inner();
    let Some(pipeline) = pipeline_cache.get_render_pipeline(pipeline_id.0) else {
        return;
    };

    let mut pass = ctx
        .command_encoder()
        .begin_render_pass(&RenderPassDescriptor {
            label: Some("aspectus_mist"),
            color_attachments: &[Some(target.get_unsampled_color_attachment())],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(
        0,
        &group.bind_group,
        &[view_offset.offset, mist_index.index()],
    );
    pass.draw(0..3, 0..1);
}
