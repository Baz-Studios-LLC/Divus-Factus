//! Miracles: the hotbar, the choosing, and the casting.
//!
//! Belief is a currency and this is where it is spent. The hotbar sits at the
//! bottom of the screen like a spellbook laid open — pick a miracle with a
//! click or its number key, then click the world to cast it there. Two exist
//! so far, and they are deliberately a moral pair:
//!
//! - **Flourish** — grace. Every bush around the settlement bears fruit.
//! - **Smite** — wrath. Lightning, called down on a point, killing what it
//!   touches.
//!
//! The same currency buys both. What the player buys is what the villagers
//! come to believe their god *is* — the bold read a smiting as power, the
//! timid read it as terror, and both readings enter their chronicles. The
//! hotbar is not a menu of effects; it is a menu of theologies.

use bevy::prelude::*;

use crate::camera::CameraSet;
use crate::creature::anim::CreatureMotion;
use crate::creature::{Corpse, Creature, Vitality};
use crate::hand::DivineHand;
use crate::scatter::FoodSource;
use crate::ui::{self, PointerContext};
use crate::villager::SettlementSite;
use crate::villager::belief::{Belief, FLOURISH_COST};
use crate::witness::{DivineEvent, DivineEventKind};

/// Belief the Smite miracle costs.
// Priced so a founding congregation (about 4-5 belief) can afford exactly
// one act of wrath from the opening pool — a taste of power, then the god
// must earn the rest one answered prayer at a time.
pub const SMITE_COST: f32 = 4.0;

/// Belief the earned tier-two miracles cost.
pub const MEND_COST: f32 = 8.0;
pub const QUAKE_COST: f32 = 8.0;

/// Belief Bounty costs. Deliberately affordable from the founding pool:
/// a new god's first act can be kindness that pays for itself, since fed
/// witnesses convert and belief flows back.
pub const BOUNTY_COST: f32 = 2.0;

/// How far Bounty's blessing reaches from the cast point.
const BOUNTY_RADIUS: f32 = 9.0;

/// How far Mend reaches from the cast point.
const MEND_RADIUS: f32 = 12.0;

/// How far Quake throws people down.
const QUAKE_RADIUS: f32 = 16.0;

/// How far the bolt's harm reaches.
const SMITE_RADIUS: f32 = 4.5;

pub struct MiraclesPlugin;

impl Plugin for MiraclesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SelectedMiracle>()
            .add_systems(Startup, spawn_hotbar)
            .add_systems(Update, dress_slot_caps)
            .add_systems(
                Update,
                (
                    reveal_earned_miracle,
                    choose_miracle,
                    cast,
                    style_hotbar,
                    update_belief_meter,
                    fade_bolts,
                    tick_glory,
                )
                    .chain()
                    .after(CameraSet),
            );
        if std::env::var("DIVUS_FACTUS_GLORY_TEST").is_ok() {
            app.add_systems(Update, glory_test_harness);
        }
    }
}

/// One aging piece of a miracle's visible glory — beam, mote, ring stone
/// or light. Everything fades on the same clock and cleans itself up.
#[derive(Component)]
struct Glory {
    age: f32,
    life: f32,
}

/// A drifting spark: carries its own velocity.
#[derive(Component)]
struct GloryMote(Vec3);

/// The central shaft of light; it thins as the glory passes.
#[derive(Component)]
struct GloryBeam;

/// The cast's light, with the intensity it peaks at.
#[derive(Component)]
struct GloryLight(f32);

/// Raises the full theophany at a point: a shaft of light out of the sky,
/// a ring of sparks breaking outward along the ground, and a slow spiral
/// of motes — rising for blessings poured into someone, falling for
/// blessings poured onto the land. Entirely procedural, one material.
pub(crate) fn spawn_glory(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    at: Vec3,
    color: Color,
    rising: bool,
) {
    let glow = materials.add(StandardMaterial {
        base_color: color.with_alpha(0.85),
        emissive: color.to_linear() * 14.0,
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let life = 2.4;

    // The shaft, sky to ground.
    commands.spawn((
        Glory { age: 0.0, life },
        GloryBeam,
        Mesh3d(cube.clone()),
        MeshMaterial3d(glow.clone()),
        Transform::from_translation(at + Vec3::Y * 14.0).with_scale(Vec3::new(0.6, 28.0, 0.6)),
        bevy::light::NotShadowCaster,
    ));
    // The light of it, thrown across the ground and the faces around.
    commands.spawn((
        Glory { age: 0.0, life },
        GloryLight(28_000_000.0),
        PointLight {
            color,
            intensity: 0.0,
            range: 55.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_translation(at + Vec3::Y * 4.0),
    ));
    // The ground ring: sparks breaking outward in a circle.
    for i in 0..18 {
        let angle = i as f32 / 18.0 * std::f32::consts::TAU;
        let (sin, cos) = angle.sin_cos();
        commands.spawn((
            Glory { age: 0.0, life },
            GloryMote(Vec3::new(cos * 5.5, 0.6, sin * 5.5)),
            Mesh3d(cube.clone()),
            MeshMaterial3d(glow.clone()),
            Transform::from_translation(at + Vec3::new(cos * 0.8, 0.25, sin * 0.8))
                .with_scale(Vec3::splat(0.22)),
            bevy::light::NotShadowCaster,
        ));
    }
    // The spiral of motes. Golden-angle spread, no two alike.
    for i in 0..26 {
        let angle = i as f32 * 2.399963;
        let (sin, cos) = angle.sin_cos();
        let radius = 1.1 + (i % 5) as f32 * 0.45;
        let lift = 1.6 + (i % 3) as f32 * 0.6;
        let (start, velocity) = if rising {
            (
                at + Vec3::new(cos * radius, 0.3 + (i % 4) as f32 * 0.2, sin * radius),
                Vec3::new(-sin * 1.1, lift, cos * 1.1),
            )
        } else {
            (
                at + Vec3::new(cos * radius * 1.6, 6.0 + (i % 5) as f32, sin * radius * 1.6),
                Vec3::new(
                    -cos * 0.4 - sin * 0.7,
                    -1.4 - (i % 3) as f32 * 0.5,
                    -sin * 0.4 + cos * 0.7,
                ),
            )
        };
        commands.spawn((
            Glory { age: 0.0, life },
            GloryMote(velocity),
            Mesh3d(cube.clone()),
            MeshMaterial3d(glow.clone()),
            Transform::from_translation(start)
                .with_scale(Vec3::splat(0.14 + (i % 3) as f32 * 0.05)),
            bevy::light::NotShadowCaster,
        ));
    }
}

/// Ages every piece of glory: motes drift, the beam thins, the light
/// swells and dies, the shared material fades, and everything despawns
/// together.
#[allow(clippy::type_complexity)]
fn tick_glory(
    mut commands: Commands,
    time: Res<Time>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut parts: Query<(
        Entity,
        &mut Glory,
        &mut Transform,
        Option<&GloryMote>,
        Option<&GloryBeam>,
        Option<&GloryLight>,
        Option<&mut PointLight>,
        Option<&MeshMaterial3d<StandardMaterial>>,
    )>,
) {
    let dt = time.delta_secs();
    for (entity, mut glory, mut transform, mote, beam, peak, light, material) in &mut parts {
        glory.age += dt;
        let t = (glory.age / glory.life).min(1.0);
        if let Some(GloryMote(velocity)) = mote {
            transform.translation += *velocity * dt;
        }
        if beam.is_some() {
            let girth = 0.6 * (1.0 - t * 0.85);
            transform.scale.x = girth;
            transform.scale.z = girth;
        }
        if let (Some(GloryLight(peak)), Some(mut light)) = (peak, light) {
            // Fast attack, long decay: a struck bell of light.
            light.intensity = peak * (t * 7.0).min(1.0) * (1.0 - t);
        }
        if let Some(material) = material {
            let faded = 0.85 * (1.0 - t);
            if let Some(mut glow) = materials.get_mut(&material.0) {
                glow.base_color.set_alpha(faded);
            }
        }
        if glory.age >= glory.life {
            commands.entity(entity).despawn();
        }
    }
}

/// Raises one of each glory near the settlement, once, so a capture run
/// can look at them. Only registered under DIVUS_FACTUS_GLORY_TEST.
fn glory_test_harness(
    mut commands: Commands,
    time: Res<Time>,
    mut fired: Local<bool>,
    site: Option<Res<SettlementSite>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(site) = site else {
        return;
    };
    if !*fired && time.elapsed_secs() > 18.0 {
        *fired = true;
        info!("GLORY_TEST: raising bounty and mend glories");
        spawn_glory(
            &mut commands,
            &mut meshes,
            &mut materials,
            site.centre + Vec3::new(-7.0, 0.0, 0.0),
            crate::palette::shade(&crate::palette::GRASS, 0.9),
            false,
        );
        spawn_glory(
            &mut commands,
            &mut meshes,
            &mut materials,
            site.centre + Vec3::new(7.0, 0.0, 0.0),
            Color::srgb(1.0, 0.88, 0.55),
            true,
        );
    }
}

/// The miracles the god can work.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Miracle {
    Flourish,
    Smite,
    /// The cheap early kindness: bushes near the touch fruit heavily.
    Bounty,
    /// Earned by a legend of providence: the broken made whole.
    Mend,
    /// Earned by a legend of dread: the ground thrown like a blanket.
    Quake,
}

impl Miracle {
    pub fn name(self) -> &'static str {
        match self {
            Miracle::Flourish => "Flourish",
            Miracle::Smite => "Smite",
            Miracle::Bounty => "Bounty",
            Miracle::Mend => "Mend",
            Miracle::Quake => "Quake",
        }
    }

    pub fn cost(self) -> f32 {
        match self {
            Miracle::Flourish => FLOURISH_COST,
            Miracle::Smite => SMITE_COST,
            Miracle::Bounty => BOUNTY_COST,
            Miracle::Mend => MEND_COST,
            Miracle::Quake => QUAKE_COST,
        }
    }

    /// What the miracle does, for the hint card.
    pub fn blurb(self) -> &'static str {
        match self {
            Miracle::Flourish => "every bush around the settlement bears fruit",
            Miracle::Smite => "lightning, called down on a point",
            Miracle::Bounty => "the bushes at the touch erupt with fruit",
            Miracle::Mend => "knits the hurt whole around the touch",
            Miracle::Quake => "throws the ground and everyone on it",
        }
    }

    pub fn key(self, keymap: &crate::keymap::Keymap) -> KeyCode {
        use crate::keymap::Deed;
        keymap.key(match self {
            Miracle::Flourish => Deed::Flourish,
            Miracle::Smite => Deed::Smite,
            Miracle::Bounty => Deed::Bounty,
            // Whichever of the pair is earned takes the fourth slot.
            Miracle::Mend | Miracle::Quake => Deed::MendOrQuake,
        })
    }
}

/// The keycap letter in a slot's corner, kept true to the keymap.
#[derive(Component)]
struct SlotCap(usize);

/// Keeps every slot's corner naming the key that actually arms it.
fn dress_slot_caps(keymap: Res<crate::keymap::Keymap>, mut caps: Query<(&SlotCap, &mut Text)>) {
    use crate::keymap::{Deed, key_name};
    for (cap, mut text) in &mut caps {
        let name = match cap.0 {
            0 => key_name(keymap.key(Deed::Flourish)),
            1 => key_name(keymap.key(Deed::Smite)),
            2 => key_name(keymap.key(Deed::Bounty)),
            3 => key_name(keymap.key(Deed::MendOrQuake)),
            _ => None,
        };
        let fresh = name
            .map(str::to_string)
            .unwrap_or_else(|| format!("{}", cap.0 + 1));
        if text.0 != fresh {
            *text = Text::new(fresh);
        }
    }
}

/// Which miracle is armed, if any. Armed, the next click on the world casts
/// it; the hand grabs nothing while a miracle is in it.
#[derive(Resource, Default)]
pub struct SelectedMiracle(pub Option<Miracle>);

/// A hotbar slot bound to a miracle (empty slots have none).
#[derive(Component)]
struct MiracleSlot(Option<Miracle>);

/// A bolt of divine lightning, briefly.
#[derive(Component)]
struct Bolt {
    remaining: f32,
}

/// The running count of belief the hotbar draws on.
#[derive(Component)]
struct BeliefReadout;

/// The filled portion of the belief bar.
#[derive(Component)]
struct BeliefFill;

fn spawn_hotbar(mut commands: Commands) {
    // The strip sits flush with the screen's foot; the column carries the
    // visual margin as PADDING instead, so the whole apron around the bar -
    // sides, top, and the gap beneath - still reads as interface and the
    // hand never flickers back to its world pose while crossing it.
    let strip = ui::centered_strip(&mut commands, Val::Auto, px(0));
    let column = commands
        .spawn((
            ui::Panel,
            Interaction::default(),
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(3),
                padding: UiRect::axes(px(16), px(0))
                    .with_top(px(12))
                    .with_bottom(px(ui::theme::MARGIN)),
                ..default()
            },
            ChildOf(strip),
        ))
        .id();
    // What the miracles spend, in plain sight: a bar that fills with the
    // belief available against the congregation's whole faith. Belief is the
    // mana here, and it is made of people.
    let meter = commands
        .spawn((
            Name::new("Belief Meter"),
            ui::Panel,
            Node {
                align_self: AlignSelf::Stretch,
                height: px(16),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(6)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(ui::theme::panel_bg()),
            BorderColor::all(ui::theme::panel_border()),
            ChildOf(column),
        ))
        .id();
    commands.spawn((
        BeliefFill,
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            top: px(0),
            bottom: px(0),
            width: percent(0),
            border_radius: BorderRadius::all(px(5)),
            ..default()
        },
        BackgroundColor(ui::theme::accent().with_alpha(0.45)),
        ChildOf(meter),
    ));
    let label_row = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                bottom: px(0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            ChildOf(meter),
        ))
        .id();
    commands.spawn((BeliefReadout, ui::dim("BELIEF 0"), ChildOf(label_row)));
    let bar = commands
        .spawn((
            Name::new("Miracle Hotbar"),
            ui::Panel,
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: px(6),
                padding: UiRect::all(px(5)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(6)),
                ..default()
            },
            BackgroundColor(ui::theme::panel_bg()),
            BorderColor::all(ui::theme::panel_border()),
            Interaction::default(),
            ChildOf(column),
        ))
        .id();

    for (index, miracle) in [
        Some(Miracle::Flourish),
        Some(Miracle::Smite),
        Some(Miracle::Bounty),
        None,
        None,
        None,
        None,
        None,
    ]
    .into_iter()
    .enumerate()
    {
        let slot = commands
            .spawn((
                MiracleSlot(miracle),
                ui::UiButton,
                Node {
                    width: px(42),
                    height: px(42),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(5)),
                    ..default()
                },
                BackgroundColor(ui::theme::panel_bg().with_alpha(0.4)),
                BorderColor::all(ui::theme::panel_border()),
                Interaction::default(),
                ChildOf(bar),
            ))
            .id();
        if let Some(miracle) = miracle {
            commands.entity(slot).insert(ui::HoverHint::new(
                miracle.name(),
                format!("{} - {:.0} belief", miracle.blurb(), miracle.cost()),
            ));
        }

        // The key in the corner, hotkey-style, kept true to the keymap.
        let number = commands
            .spawn((
                SlotCap(index),
                ui::dim(String::new()),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(4),
                    top: px(1),
                    ..default()
                },
            ))
            .id();
        commands.entity(number).insert(ChildOf(slot));

        // And what it costs, in the opposite corner.
        if let Some(miracle) = miracle {
            let cost = commands
                .spawn((
                    ui::dim(format!("{:.0}", miracle.cost())),
                    Node {
                        position_type: PositionType::Absolute,
                        right: px(4),
                        bottom: px(1),
                        ..default()
                    },
                ))
                .id();
            commands.entity(cost).insert(ChildOf(slot));
        }

        // Node-drawn icons: a sprout for grace, a jagged bolt for wrath.
        match miracle {
            Some(Miracle::Flourish) => {
                for (l, t, w, h) in [
                    (20.0, 12.0, 3.0, 22.0),
                    (12.0, 14.0, 9.0, 6.0),
                    (22.0, 20.0, 9.0, 6.0),
                ] {
                    let leaf = commands
                        .spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: px(l),
                                top: px(t),
                                width: px(w),
                                height: px(h),
                                border_radius: BorderRadius::all(px(3)),
                                ..default()
                            },
                            BackgroundColor(crate::palette::shade(&crate::palette::GRASS, 0.7)),
                        ))
                        .id();
                    commands.entity(leaf).insert(ChildOf(slot));
                }
            }
            Some(Miracle::Smite) => {
                for (l, t, w, h) in [(22.0, 8.0, 5.0, 14.0), (16.0, 19.0, 5.0, 15.0)] {
                    let jag = commands
                        .spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: px(l),
                                top: px(t),
                                width: px(w),
                                height: px(h),
                                ..default()
                            },
                            BackgroundColor(ui::theme::accent()),
                        ))
                        .id();
                    commands.entity(jag).insert(ChildOf(slot));
                }
            }
            // A cluster of berries under a leaf — the bounty.
            Some(Miracle::Bounty) => {
                for (l, t, s, red) in [
                    (14.0, 20.0, 8.0, true),
                    (22.0, 22.0, 8.0, true),
                    (18.0, 14.0, 8.0, true),
                    (16.0, 8.0, 12.0, false),
                ] {
                    let dot = commands
                        .spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: px(l),
                                top: px(t),
                                width: px(s),
                                height: px(if red { s } else { 5.0 }),
                                border_radius: BorderRadius::all(px(4)),
                                ..default()
                            },
                            BackgroundColor(if red {
                                crate::palette::shade(&crate::palette::CLOTH_RED, 0.7)
                            } else {
                                crate::palette::shade(&crate::palette::GRASS, 0.7)
                            }),
                        ))
                        .id();
                    commands.entity(dot).insert(ChildOf(slot));
                }
            }
            _ => {}
        }
    }
}

/// When a legend crystallises a new miracle, it materialises in the empty
/// third slot, icon and all.
fn reveal_earned_miracle(
    mut commands: Commands,
    legend: Res<crate::villager::belief::Legend>,
    mut slots: Query<(Entity, &mut MiracleSlot)>,
) {
    let Some(earned) = legend.unlocked else {
        return;
    };
    if slots.iter().any(|(_, slot)| slot.0 == Some(earned)) {
        return;
    }
    let Some((slot_entity, mut slot)) = slots.iter_mut().find(|(_, slot)| slot.0.is_none()) else {
        return;
    };
    slot.0 = Some(earned);
    commands.entity(slot_entity).insert(ui::HoverHint::new(
        earned.name(),
        format!("{} - {:.0} belief", earned.blurb(), earned.cost()),
    ));

    match earned {
        // A cross of green — mending.
        Miracle::Mend => {
            for (l, t, w, h) in [(18.0, 9.0, 7.0, 24.0), (10.0, 17.0, 23.0, 7.0)] {
                let bar = commands
                    .spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(l),
                            top: px(t),
                            width: px(w),
                            height: px(h),
                            border_radius: BorderRadius::all(px(3)),
                            ..default()
                        },
                        BackgroundColor(crate::palette::shade(&crate::palette::GRASS, 0.8)),
                    ))
                    .id();
                commands.entity(bar).insert(ChildOf(slot_entity));
            }
        }
        // Broken strata — the quake.
        Miracle::Quake => {
            for (l, t, w) in [(8.0, 12.0, 12.0), (23.0, 15.0, 11.0), (12.0, 24.0, 14.0)] {
                let crack = commands
                    .spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(l),
                            top: px(t),
                            width: px(w),
                            height: px(4),
                            ..default()
                        },
                        BackgroundColor(ui::theme::text_dim()),
                    ))
                    .id();
                commands.entity(crack).insert(ChildOf(slot_entity));
            }
        }
        _ => {}
    }
}

/// Arming and disarming: click a slot or press its number; right-click or
/// Escape lowers the hand empty.
fn choose_miracle(
    keys: Res<ButtonInput<KeyCode>>,
    keymap: Res<crate::keymap::Keymap>,
    buttons: Res<ButtonInput<MouseButton>>,
    slots: Query<(&Interaction, &MiracleSlot), Changed<Interaction>>,
    all_slots: Query<&MiracleSlot>,
    belief: Res<Belief>,
    mut selected: ResMut<SelectedMiracle>,
    mut notices: MessageWriter<ui::Notice>,
) {
    // Arming an unaffordable miracle fails out loud: silence here reads as a
    // broken button, not an empty purse.
    let mut arm = |miracle: Miracle, selected: &mut SelectedMiracle| {
        if selected.0 == Some(miracle) {
            selected.0 = None;
        } else if belief.available() >= miracle.cost() {
            selected.0 = Some(miracle);
        } else {
            notices.write(ui::Notice::new(format!(
                "{} needs {:.0} belief - the people hold {:.0}",
                miracle.name(),
                miracle.cost(),
                belief.available(),
            )));
        }
    };
    for (interaction, slot) in &slots {
        if *interaction == Interaction::Pressed
            && let Some(miracle) = slot.0
        {
            arm(miracle, &mut selected);
        }
    }
    for miracle in [
        Miracle::Flourish,
        Miracle::Smite,
        Miracle::Bounty,
        Miracle::Mend,
        Miracle::Quake,
    ] {
        if keys.just_pressed(miracle.key(&keymap)) && all_slots.iter().any(|s| s.0 == Some(miracle))
        {
            arm(miracle, &mut selected);
        }
    }
    if keys.just_pressed(KeyCode::Escape) || buttons.just_pressed(MouseButton::Right) {
        selected.0 = None;
    }
}

/// An armed miracle casts where the hand touches the world.
fn cast(
    mut commands: Commands,
    buttons: Res<ButtonInput<MouseButton>>,
    pointer: Res<PointerContext>,
    hand: Res<DivineHand>,
    site: Option<Res<SettlementSite>>,
    name: Option<Res<crate::villager::DivineName>>,
    mut selected: ResMut<SelectedMiracle>,
    mut belief: ResMut<Belief>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut bushes: Query<(&GlobalTransform, &mut FoodSource)>,
    mut notices: MessageWriter<crate::ui::Notice>,
    mut witnessed: MessageWriter<DivineEvent>,
    mut victims: Query<
        (Entity, &Transform, &mut Vitality, &mut CreatureMotion),
        (
            With<Creature>,
            Without<Corpse>,
            Without<crate::creature::Held>,
        ),
    >,
    mut souls: Query<
        (
            &mut crate::villager::Morale,
            &mut crate::villager::belief::Faith,
        ),
        With<crate::villager::Villager>,
    >,
) {
    let Some(miracle) = selected.0 else {
        return;
    };
    if !buttons.just_pressed(MouseButton::Left) || pointer.over_ui {
        return;
    }
    let Some(at) = hand.cursor_world else {
        return;
    };
    let god = name.as_ref().map_or("the god", |n| n.0.as_str());

    match miracle {
        Miracle::Bounty => {
            if belief.available() < BOUNTY_COST {
                return;
            }
            // The blessing needs something living to land on.
            let mut blessed = 0;
            for (bush_at, mut bush) in &mut bushes {
                if bush_at.translation().distance(at) <= BOUNTY_RADIUS {
                    bush.amount = FoodSource::CAPACITY;
                    blessed += 1;
                }
            }
            if blessed == 0 {
                notices.write(crate::ui::Notice::new(
                    "Nothing grows there for Bounty to bless".to_string(),
                ));
                return;
            }
            belief.spent += BOUNTY_COST;
            spawn_glory(
                &mut commands,
                &mut meshes,
                &mut materials,
                at,
                crate::palette::shade(&crate::palette::GRASS, 0.9),
                false,
            );
            info!("{god} blessed the bushes with plenty");
            notices.write(crate::ui::Notice::fanfare(format!(
                "{god} blessed the bushes with plenty"
            )));
            witnessed.write(DivineEvent {
                kind: DivineEventKind::Provided,
                position: at,
                subject: None,
                intensity: 0.8,
            });
        }
        Miracle::Flourish => {
            let Some(site) = site else {
                return;
            };
            if !crate::villager::belief::flourish(&mut belief, &site, &mut bushes) {
                return;
            }
            spawn_glory(
                &mut commands,
                &mut meshes,
                &mut materials,
                site.centre,
                crate::palette::shade(&crate::palette::GRASS, 0.85),
                true,
            );
            info!("{god} made the land flourish");
            notices.write(crate::ui::Notice::fanfare(format!(
                "{god} made the land flourish"
            )));
            // Flourished, not Provided. Both wrote Provided, which
            // flattened two theologically different acts into one story:
            // "food was set down before us" and "the land itself was made
            // to give more" are not the same claim about what the god is,
            // and doctrine will one day be spun from exactly that
            // difference. It also stranded the four `event:flourished`
            // lines already in the corpus - written for abundance, and
            // unreachable by the abundance miracle.
            witnessed.write(DivineEvent {
                kind: DivineEventKind::Flourished,
                position: site.centre,
                subject: None,
                intensity: 0.9,
            });
        }
        Miracle::Smite => {
            if belief.available() < SMITE_COST {
                return;
            }
            belief.spent += SMITE_COST;

            lightning_bolt(&mut commands, &mut meshes, &mut materials, at);

            // The harm, and the first name it fell on.
            let mut struck: Option<Entity> = None;
            for (victim, transform, mut vitality, mut motion) in &mut victims {
                if transform.translation.distance(at) > SMITE_RADIUS {
                    continue;
                }
                vitality.harm += 1.2;
                vitality.violent = true;
                vitality.undoing = crate::creature::Undoing::Lightning;
                motion.flail = 1.0;
                struck.get_or_insert(victim);
            }

            info!("{god} called down lightning");
            notices.write(crate::ui::Notice::fanfare(format!(
                "{god} called down lightning"
            )));
            witnessed.write(DivineEvent {
                kind: DivineEventKind::Smote,
                position: at,
                subject: struck,
                intensity: 1.0,
            });
        }
        Miracle::Mend | Miracle::Quake => {
            if belief.available() < miracle.cost() {
                return;
            }
            // Mending arrives as warm rising light; the quake speaks for
            // itself in thrown bodies.
            if miracle == Miracle::Mend {
                spawn_glory(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    at,
                    Color::srgb(1.0, 0.88, 0.55),
                    true,
                );
            }
            cast_earned(
                miracle,
                at,
                god,
                &mut belief,
                &mut notices,
                &mut witnessed,
                &mut victims,
                &mut souls,
            );
        }
    }

    // One cast per arming: a miracle is a sentence, not a hose.
    selected.0 = None;
}

/// The Mend and Quake arms of [`cast`], split out for length.
#[allow(clippy::too_many_arguments)]
fn cast_earned(
    miracle: Miracle,
    at: Vec3,
    god: &str,
    belief: &mut Belief,
    notices: &mut MessageWriter<crate::ui::Notice>,
    witnessed: &mut MessageWriter<DivineEvent>,
    victims: &mut Query<
        (Entity, &Transform, &mut Vitality, &mut CreatureMotion),
        (
            With<Creature>,
            Without<Corpse>,
            Without<crate::creature::Held>,
        ),
    >,
    souls: &mut Query<
        (
            &mut crate::villager::Morale,
            &mut crate::villager::belief::Faith,
        ),
        With<crate::villager::Villager>,
    >,
) {
    match miracle {
        Miracle::Mend => {
            belief.spent += MEND_COST;
            let mut first: Option<Entity> = None;
            for (entity, transform, mut vitality, mut motion) in victims.iter_mut() {
                if transform.translation.distance(at) > MEND_RADIUS {
                    continue;
                }
                if vitality.harm <= 0.0 {
                    continue;
                }
                vitality.harm = 0.0;
                vitality.violent = false;
                motion.flail = 0.0;
                first.get_or_insert(entity);
                if let Ok((mut morale, mut faith)) = souls.get_mut(entity) {
                    morale.spirits = (morale.spirits + 0.35).min(1.0);
                    faith.trust = (faith.trust + 0.25).min(1.0);
                }
            }
            info!("{god} made the broken whole");
            notices.write(crate::ui::Notice::fanfare(format!(
                "{god} made the broken whole"
            )));
            witnessed.write(DivineEvent {
                kind: DivineEventKind::Mended,
                position: at,
                subject: first,
                intensity: 0.8,
            });
        }
        Miracle::Quake => {
            belief.spent += QUAKE_COST;
            let mut first: Option<Entity> = None;
            for (entity, transform, _, mut motion) in victims.iter_mut() {
                let offset = transform.translation - at;
                if offset.length() > QUAKE_RADIUS {
                    continue;
                }
                motion.flail = 1.0;
                first.get_or_insert(entity);
            }
            let _ = first;
            info!("{god} shook the earth");
            notices.write(crate::ui::Notice::fanfare(format!("{god} shook the earth")));
            witnessed.write(DivineEvent {
                kind: DivineEventKind::Quaked,
                position: at,
                subject: None,
                intensity: 1.0,
            });
        }
        _ => {}
    }
}

/// The meter follows the people's living faith: the bar's capacity is the
/// congregation's whole faith, the fill is what is unspent.
fn update_belief_meter(
    belief: Res<Belief>,
    mut readout: Query<&mut Text, With<BeliefReadout>>,
    mut fill: Query<&mut Node, With<BeliefFill>>,
) {
    if !belief.is_changed() {
        return;
    }
    for mut text in &mut readout {
        text.0 = format!("BELIEF {:.0} / {:.0}", belief.available(), belief.total);
    }
    let fraction = if belief.total > 0.0 {
        (belief.available() / belief.total).clamp(0.0, 1.0)
    } else {
        0.0
    };
    for mut node in &mut fill {
        node.width = percent(fraction * 100.0);
    }
}

/// The armed slot glows; unaffordable miracles sit dim.
fn style_hotbar(
    selected: Res<SelectedMiracle>,
    belief: Res<Belief>,
    mut slots: Query<(&MiracleSlot, &mut BorderColor, &mut BackgroundColor)>,
) {
    for (slot, mut border, mut bg) in &mut slots {
        let Some(miracle) = slot.0 else {
            *border = BorderColor::all(ui::theme::panel_border().with_alpha(0.15));
            bg.0 = ui::theme::panel_bg().with_alpha(0.2);
            continue;
        };
        let affordable = belief.available() >= miracle.cost();
        let armed = selected.0 == Some(miracle);
        *border = BorderColor::all(if armed {
            ui::theme::accent()
        } else if affordable {
            ui::theme::panel_border()
        } else {
            ui::theme::panel_border().with_alpha(0.12)
        });
        bg.0 = if armed {
            ui::theme::accent().with_alpha(0.3)
        } else if affordable {
            ui::theme::panel_bg().with_alpha(0.4)
        } else {
            ui::theme::panel_bg().with_alpha(0.15)
        };
    }
}

/// The bolt: jagged white-hot segments from the sky to a point. Shared by
/// the Smite miracle and the storm - deliberately indistinguishable,
/// because the witnesses cannot tell heaven's wrath from the sky's.
pub(crate) fn lightning_bolt(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    at: Vec3,
) {
    let flash = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        emissive: LinearRgba::WHITE * 24.0,
        ..default()
    });
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let bolt = commands
        .spawn((
            Bolt { remaining: 0.4 },
            Transform::from_translation(at),
            Visibility::default(),
        ))
        .id();
    let mut height = 0.0;
    let mut sway = 0.0f32;
    while height < 60.0 {
        let segment = 7.0;
        commands.spawn((
            Mesh3d(cube.clone()),
            MeshMaterial3d(flash.clone()),
            Transform::from_xyz(sway, height + segment * 0.5, sway * 0.6).with_scale(Vec3::new(
                0.5,
                segment + 1.0,
                0.5,
            )),
            bevy::light::NotShadowCaster,
            ChildOf(bolt),
        ));
        height += segment;
        sway = if sway > 0.0 { -1.1 } else { 1.3 };
    }
    commands.spawn((
        PointLight {
            color: Color::WHITE,
            intensity: 40_000_000.0,
            range: 90.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(0.0, 6.0, 0.0),
        ChildOf(bolt),
    ));
}

/// Lightning does not linger.
fn fade_bolts(
    mut commands: Commands,
    time: Res<Time>,
    mut bolts: Query<(Entity, &mut Bolt, &mut Transform)>,
) {
    for (entity, mut bolt, mut transform) in &mut bolts {
        bolt.remaining -= time.delta_secs();
        // It thins as it dies.
        transform.scale.x = (bolt.remaining / 0.4).max(0.05);
        transform.scale.z = transform.scale.x;
        if bolt.remaining <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrath_is_cheaper_than_grace() {
        // Deliberate: terror is the easy road. The interesting choice is the
        // expensive one, and the cheap one should always be tempting.
        assert!(SMITE_COST < FLOURISH_COST);
    }
}
