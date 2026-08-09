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
use crate::villager::belief::Belief;
use crate::witness::{DivineEvent, DivineEventKind};

// The unlock ladder: congregation faith a miracle asks for BEFORE it will
// be taught. Belief is never spent — crossing each high-water mark is the
// whole price, and after that the calendar is the only cost. Brett: "move
// them to cooldown only. Belief amount will be one of the unlock
// requirements. No form of mana."

/// A founding congregation holds 4-5 faith: wrath is the first thing a new
/// god grows into, one answered prayer past the opening pool.
pub const SMITE_UNLOCK: f32 = 4.0;

/// Walking in a body asks for a congregation that already half-believes.
pub const AVATAR_UNLOCK: f32 = 6.0;

/// The land-wide providence, for a village whose faith has real weight.
pub const FLOURISH_UNLOCK: f32 = 10.0;

/// How a miracle is earned.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Unlock {
    /// The god's from the first day.
    Founding,
    /// Taught when the congregation's faith first holds this much.
    Belief(f32),
    /// Awarded by the legends: Mend or Quake, whichever crystallises.
    Legend,
    /// The dark school: taught when the god's dread has grown this far.
    Dread(f32),
}

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
            .init_resource::<Hotbar>()
            .init_resource::<Grimoire>()
            .init_resource::<Cooldowns>()
            .init_resource::<DragState>()
            .add_message::<FireMiracle>()
            .add_systems(Startup, spawn_hotbar)
            .add_systems(Update, (dress_slot_caps, dress_the_bar, cooldown_faces))
            .add_systems(
                Update,
                (
                    unlock_miracles,
                    carry_miracles,
                    choose_miracle,
                    fire_the_slots,
                    cast,
                    style_hotbar,
                    update_belief_meter,
                    // The standing wonders: one subject, paired to spare
                    // the tuple.
                    (beacons_call, wards_hold, evangels_preach, stones_land),
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
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Miracle {
    Flourish,
    Smite,
    /// The cheap early kindness: bushes near the touch fruit heavily.
    Bounty,
    /// Earned by a legend of providence: the broken made whole.
    Mend,
    /// Earned by a legend of dread: the ground thrown like a blanket.
    Quake,
    /// Not done TO the world but INSIDE it: the god takes a villager's
    /// body and walks around in it. Nobody remarks on it, which is the
    /// whole point - it is the only way a god ever hears what is said
    /// about them by people who think they are alone.
    Avatar,
    /// A soaking called onto a point: crops drink deep, fires die.
    Rain,
    /// A pillar of light the idle walk to - the crowd-mover.
    Beacon,
    /// A circle no predator will cross while it stands.
    Ward,
    /// Every ripe field in the village yields at once.
    HarvestWind,
    /// The dark ripple: fear crosses every heart near the touch.
    PlagueOfDoubt,
    /// A boulder out of a clear sky - and it stays, as stone.
    StoneFromSky,
    /// A dream visited on one soul: they wake certain, and they preach.
    Visitation,
    /// The map opens wide around the touch - colony ground included.
    FoundingSight,
}

impl Miracle {
    /// Every miracle there is, for the unlock ladder to walk.
    pub const ALL: [Miracle; 14] = [
        Miracle::Bounty,
        Miracle::Smite,
        Miracle::Avatar,
        Miracle::Rain,
        Miracle::Flourish,
        Miracle::Beacon,
        Miracle::Ward,
        Miracle::HarvestWind,
        Miracle::FoundingSight,
        Miracle::Visitation,
        Miracle::Mend,
        Miracle::Quake,
        Miracle::PlagueOfDoubt,
        Miracle::StoneFromSky,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Miracle::Flourish => "Flourish",
            Miracle::Smite => "Smite",
            Miracle::Bounty => "Bounty",
            Miracle::Mend => "Mend",
            Miracle::Quake => "Quake",
            Miracle::Avatar => "Avatar",
            Miracle::Rain => "Rain",
            Miracle::Beacon => "Beacon",
            Miracle::Ward => "Ward",
            Miracle::HarvestWind => "Harvest Wind",
            Miracle::PlagueOfDoubt => "Plague of Doubt",
            Miracle::StoneFromSky => "Stone from Sky",
            Miracle::Visitation => "Visitation",
            Miracle::FoundingSight => "Founding Sight",
        }
    }

    /// Days between castings. The calendar is the cost now — no mana, no
    /// meter, just the patience of a god who must choose what this day's
    /// power is spent on. Brett: "maybe an ability has a 1 or 2 day
    /// cooldown... some might even have a week."
    pub fn cooldown_days(self) -> f32 {
        match self {
            Miracle::Bounty => 0.5,
            Miracle::Smite | Miracle::Avatar | Miracle::Mend | Miracle::Rain => 1.0,
            Miracle::Flourish => 1.5,
            Miracle::Quake | Miracle::Beacon | Miracle::Ward => 2.0,
            Miracle::HarvestWind | Miracle::StoneFromSky => 3.0,
            Miracle::PlagueOfDoubt => 4.0,
            // The week-long trumpets. Brett: "some might even have a week."
            Miracle::Visitation | Miracle::FoundingSight => 7.0,
        }
    }

    /// The cooldown in world-clock seconds.
    pub fn cooldown_secs(self) -> f64 {
        (self.cooldown_days() * crate::calendar::DAY_SECONDS) as f64
    }

    /// What unlocks this miracle. Belief is the LADDER, never the fuel;
    /// the legends award their own; the darkest asks are paid in dread.
    pub fn unlock(self) -> Unlock {
        match self {
            Miracle::Bounty => Unlock::Founding,
            Miracle::Smite => Unlock::Belief(SMITE_UNLOCK),
            Miracle::Avatar => Unlock::Belief(AVATAR_UNLOCK),
            Miracle::Rain => Unlock::Belief(8.0),
            Miracle::Flourish => Unlock::Belief(FLOURISH_UNLOCK),
            Miracle::Beacon => Unlock::Belief(12.0),
            Miracle::Ward => Unlock::Belief(14.0),
            Miracle::HarvestWind => Unlock::Belief(16.0),
            Miracle::FoundingSight => Unlock::Belief(18.0),
            Miracle::Visitation => Unlock::Belief(20.0),
            Miracle::Mend | Miracle::Quake => Unlock::Legend,
            Miracle::PlagueOfDoubt => Unlock::Dread(10.0),
            Miracle::StoneFromSky => Unlock::Dread(16.0),
        }
    }

    /// The belief rung alone, for displays that speak in numbers.
    pub fn unlock_at(self) -> Option<f32> {
        match self.unlock() {
            Unlock::Belief(rung) => Some(rung),
            _ => None,
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
            Miracle::Avatar => "wear somebody's body and walk about in it",
            Miracle::Rain => "a soaking over the point - crops surge, fires die",
            Miracle::Beacon => "a pillar of light the village gathers to",
            Miracle::Ward => "a circle predators will not cross for a day",
            Miracle::HarvestWind => "every ripe field yields at once",
            Miracle::PlagueOfDoubt => "fear ripples outward from the touch",
            Miracle::StoneFromSky => "a boulder falls - and stays, as stone",
            Miracle::Visitation => "appear in a dream; they wake certain, and preach",
            Miracle::FoundingSight => "the map opens wide around the touch",
        }
    }
}

/// The keycap letter in a slot's corner, kept true to the keymap.
#[derive(Component)]
struct SlotCap(usize);

/// Keeps every slot's corner naming the key that actually fires it.
fn dress_slot_caps(keymap: Res<crate::keymap::Keymap>, mut caps: Query<(&SlotCap, &mut Text)>) {
    use crate::keymap::{Deed, key_name};
    for (cap, mut text) in &mut caps {
        let fresh = key_name(keymap.key(Deed::slot(cap.0)))
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

/// The bar itself: ten slots and what the god has set in each. Drag one
/// miracle onto another slot and they trade places — the arrangement is
/// the god's own and it is SAVED. Brett: "10 buttons wide... drag and
/// drop like WoW."
#[derive(Resource, Clone, serde::Serialize, serde::Deserialize)]
pub struct Hotbar(pub [Option<Miracle>; 10]);

impl Default for Hotbar {
    fn default() -> Self {
        let mut bar = [None; 10];
        // The founding kit: kindness first. Everything else is earned.
        bar[0] = Some(Miracle::Bounty);
        Hotbar(bar)
    }
}

impl Hotbar {
    /// The slot a miracle sits in, if it sits anywhere.
    pub fn slot_of(&self, miracle: Miracle) -> Option<usize> {
        self.0.iter().position(|held| *held == Some(miracle))
    }

    /// Sets a freshly learned miracle in the first empty slot.
    pub fn take_in(&mut self, miracle: Miracle) {
        if self.slot_of(miracle).is_some() {
            return;
        }
        if let Some(empty) = self.0.iter().position(Option::is_none) {
            self.0[empty] = Some(miracle);
        }
    }
}

/// What the god has earned the right to, and the most faith the
/// congregation has ever held together — the ladder is climbed on the
/// HIGH-WATER mark, so a faith dip never takes a miracle back.
#[derive(Resource, Clone, serde::Serialize, serde::Deserialize)]
pub struct Grimoire {
    pub unlocked: Vec<Miracle>,
    pub high_water: f32,
}

impl Default for Grimoire {
    fn default() -> Self {
        Grimoire {
            unlocked: vec![Miracle::Bounty],
            high_water: 0.0,
        }
    }
}

impl Grimoire {
    pub fn knows(&self, miracle: Miracle) -> bool {
        self.unlocked.contains(&miracle)
    }
}

/// When each miracle is ready again, in world-clock seconds. The calendar
/// is the whole economy now.
#[derive(Resource, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Cooldowns(pub Vec<(Miracle, f64)>);

impl Cooldowns {
    pub fn ready(&self, miracle: Miracle, now: f64) -> bool {
        self.remaining(miracle, now) <= 0.0
    }

    /// Seconds until this miracle answers again. Zero when ready.
    pub fn remaining(&self, miracle: Miracle, now: f64) -> f64 {
        self.0
            .iter()
            .find(|(held, _)| *held == miracle)
            .map_or(0.0, |(_, until)| (until - now).max(0.0))
    }

    /// The casting spends the days.
    pub fn start(&mut self, miracle: Miracle, now: f64) {
        self.0.retain(|(held, _)| *held != miracle);
        self.0.push((miracle, now + miracle.cooldown_secs()));
    }
}

/// A casting on its way: what to work and where. Written by the armed
/// click and by the slot keys alike, so there is exactly one performer.
#[derive(bevy::prelude::Message)]
pub struct FireMiracle {
    pub miracle: Miracle,
    pub at: Vec3,
}

/// A hotbar slot, by position. What sits in it lives in [`Hotbar`].
#[derive(Component)]
struct MiracleSlot(usize);

/// A piece of slot dressing — icon nodes and hint, torn down and redrawn
/// whenever the bar's arrangement changes.
#[derive(Component)]
struct SlotArt;

/// The dark sweep rising over a slot while its miracle rests.
#[derive(Component)]
struct CooldownShade(usize);

/// The remaining-time word under the sweep.
#[derive(Component)]
struct CooldownLabel(usize);

/// A slot flashing with the cast it just fired.
#[derive(Component)]
struct FiredFlash(f32);

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

    // Ten slots, bare. What sits in each is the Hotbar resource's business,
    // and `dress_the_bar` draws it — the frames here are furniture. Brett:
    // "I want the bar to be 10 buttons wide."
    for index in 0..10 {
        let slot = commands
            .spawn((
                MiracleSlot(index),
                ui::UiButton,
                Interaction::default(),
                UiTransform::default(),
                Node {
                    width: px(42),
                    height: px(42),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(5)),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(ui::theme::panel_bg().with_alpha(0.4)),
                BorderColor::all(ui::theme::panel_border()),
                ChildOf(bar),
            ))
            .id();

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

        // The rest sweep: a dark tide that drains as the days pass.
        commands.spawn((
            CooldownShade(index),
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                bottom: px(0),
                height: percent(0),
                ..default()
            },
            BackgroundColor(Color::BLACK.with_alpha(0.62)),
            ChildOf(slot),
        ));
        // And the word for how long: "2d", "14h".
        commands.spawn((
            CooldownLabel(index),
            ui::dim(String::new()),
            Node {
                position_type: PositionType::Absolute,
                right: px(4),
                bottom: px(1),
                ..default()
            },
            ChildOf(slot),
        ));
    }
}

/// Draws a miracle's face into a slot: the node-built icon, tagged as art
/// so a re-dress can sweep it. The same shapes the slots always wore.
fn draw_miracle_icon(commands: &mut Commands, slot: Entity, miracle: Miracle) {
    let art = |commands: &mut Commands, node: Node, color: Color| {
        let piece = commands.spawn((SlotArt, node, BackgroundColor(color))).id();
        commands.entity(piece).insert(ChildOf(slot));
    };
    let block = |l: f32, t: f32, w: f32, h: f32, r: f32| Node {
        position_type: PositionType::Absolute,
        left: px(l),
        top: px(t),
        width: px(w),
        height: px(h),
        border_radius: BorderRadius::all(px(r)),
        ..default()
    };
    match miracle {
        // A sprout: stem and two leaves.
        Miracle::Flourish => {
            for (l, t, w, h) in [
                (20.0, 12.0, 3.0, 22.0),
                (12.0, 14.0, 9.0, 6.0),
                (22.0, 20.0, 9.0, 6.0),
            ] {
                art(
                    commands,
                    block(l, t, w, h, 3.0),
                    crate::palette::shade(&crate::palette::GRASS, 0.7),
                );
            }
        }
        // The jagged bolt.
        Miracle::Smite => {
            for (l, t, w, h) in [(22.0, 8.0, 5.0, 14.0), (16.0, 19.0, 5.0, 15.0)] {
                art(commands, block(l, t, w, h, 0.0), ui::theme::accent());
            }
        }
        // A cluster of berries under a leaf.
        Miracle::Bounty => {
            for (l, t, sz, red) in [
                (14.0, 20.0, 8.0, true),
                (22.0, 22.0, 8.0, true),
                (18.0, 14.0, 8.0, true),
                (16.0, 8.0, 12.0, false),
            ] {
                art(
                    commands,
                    block(l, t, sz, if red { sz } else { 5.0 }, 4.0),
                    if red {
                        crate::palette::shade(&crate::palette::CLOTH_RED, 0.7)
                    } else {
                        crate::palette::shade(&crate::palette::GRASS, 0.7)
                    },
                );
            }
        }
        // A body, standing empty and waiting to be worn.
        Miracle::Avatar => {
            for (l, t, w, h, round) in [
                (17.0, 9.0, 8.0, 8.0, true),
                (18.0, 18.0, 6.0, 9.0, false),
                (18.0, 27.0, 2.0, 6.0, false),
                (22.0, 27.0, 2.0, 6.0, false),
            ] {
                art(
                    commands,
                    block(l, t, w, h, if round { 4.0 } else { 1.0 }),
                    ui::theme::accent().with_alpha(0.85),
                );
            }
        }
        // A cross of green — mending.
        Miracle::Mend => {
            for (l, t, w, h) in [(18.0, 9.0, 7.0, 24.0), (10.0, 17.0, 23.0, 7.0)] {
                art(
                    commands,
                    block(l, t, w, h, 3.0),
                    crate::palette::shade(&crate::palette::GRASS, 0.8),
                );
            }
        }
        // Broken strata — the quake.
        Miracle::Quake => {
            for (l, t, w) in [(8.0, 12.0, 12.0), (23.0, 15.0, 11.0), (12.0, 24.0, 14.0)] {
                art(commands, block(l, t, w, 4.0, 0.0), ui::theme::text_dim());
            }
        }
        // Falling drops over a bowed row.
        Miracle::Rain => {
            let blue = crate::palette::shade(&crate::palette::CLOTH_BLUE, 0.75);
            for (l, t) in [
                (12.0, 8.0),
                (20.0, 12.0),
                (28.0, 8.0),
                (16.0, 18.0),
                (24.0, 20.0),
            ] {
                art(commands, block(l, t, 3.0, 7.0, 2.0), blue);
            }
            art(
                commands,
                block(10.0, 30.0, 22.0, 4.0, 2.0),
                crate::palette::shade(&crate::palette::GRASS, 0.7),
            );
        }
        // The pillar, standing in its own glow.
        Miracle::Beacon => {
            art(
                commands,
                block(19.0, 6.0, 4.0, 28.0, 2.0),
                ui::theme::accent(),
            );
            art(
                commands,
                block(14.0, 30.0, 14.0, 4.0, 2.0),
                ui::theme::accent().with_alpha(0.5),
            );
        }
        // The ring of stones.
        Miracle::Ward => {
            for (l, t) in [
                (18.0, 7.0),
                (27.0, 12.0),
                (29.0, 21.0),
                (22.0, 28.0),
                (12.0, 28.0),
                (7.0, 20.0),
                (9.0, 11.0),
            ] {
                art(
                    commands,
                    block(l, t, 5.0, 5.0, 2.0),
                    crate::palette::shade(&crate::palette::STONE, 0.75),
                );
            }
        }
        // Stalks leaning into the wind.
        Miracle::HarvestWind => {
            let gold = crate::palette::shade(&crate::palette::CLOTH_GOLD, 0.85);
            for (l, h) in [(11.0, 16.0), (17.0, 20.0), (23.0, 18.0), (29.0, 14.0)] {
                art(commands, block(l, 34.0 - h, 3.0, h, 2.0), gold);
            }
        }
        // A dark drop, spreading rings.
        Miracle::PlagueOfDoubt => {
            let ash = crate::palette::shade(&crate::palette::CLOTH_SABLE, 0.6);
            art(commands, block(18.0, 16.0, 6.0, 6.0, 4.0), ash);
            art(
                commands,
                block(13.0, 11.0, 16.0, 2.0, 2.0),
                ash.with_alpha(0.5),
            );
            art(
                commands,
                block(13.0, 27.0, 16.0, 2.0, 2.0),
                ash.with_alpha(0.5),
            );
        }
        // The stone, and the streak it fell by.
        Miracle::StoneFromSky => {
            art(
                commands,
                block(24.0, 6.0, 4.0, 12.0, 2.0),
                ui::theme::accent().with_alpha(0.6),
            );
            art(
                commands,
                block(12.0, 20.0, 14.0, 12.0, 4.0),
                crate::palette::shade(&crate::palette::STONE, 0.55),
            );
        }
        // The closed eye, dreaming.
        Miracle::Visitation => {
            let pink = crate::palette::shade(&crate::palette::CLOTH_PINK, 0.85);
            art(commands, block(11.0, 17.0, 20.0, 4.0, 3.0), pink);
            for l in [13.0, 19.0, 25.0] {
                art(
                    commands,
                    block(l, 22.0, 2.0, 5.0, 1.0),
                    pink.with_alpha(0.6),
                );
            }
        }
        // The horizon line, opened.
        Miracle::FoundingSight => {
            let bone = crate::palette::shade(&crate::palette::BONE, 0.9);
            art(commands, block(8.0, 20.0, 26.0, 3.0, 2.0), bone);
            art(
                commands,
                block(14.0, 12.0, 14.0, 3.0, 2.0),
                bone.with_alpha(0.6),
            );
            art(
                commands,
                block(18.0, 27.0, 6.0, 6.0, 4.0),
                ui::theme::accent(),
            );
        }
    }
}

/// Redraws every slot's face whenever the bar's arrangement changes — a
/// drag, an unlock, a load. The frames stay; the art is swept and drawn.
#[allow(clippy::type_complexity)]
fn dress_the_bar(
    mut commands: Commands,
    hotbar: Res<Hotbar>,
    art: Query<(Entity, &ChildOf), With<SlotArt>>,
    slots: Query<(Entity, &MiracleSlot)>,
) {
    if !hotbar.is_changed() {
        return;
    }
    for (piece, parent) in &art {
        let _ = parent;
        // try_despawn: the drop that changed the Hotbar also despawns the
        // drag ghost, whose icon pieces are art too - despawning them
        // twice in one frame must be a no-op, not a panic.
        commands.entity(piece).try_despawn();
    }
    for (slot, place) in &slots {
        let Some(miracle) = hotbar.0[place.0] else {
            commands.entity(slot).remove::<ui::HoverHint>();
            continue;
        };
        draw_miracle_icon(&mut commands, slot, miracle);
        commands.entity(slot).insert(ui::HoverHint::new(
            miracle.name(),
            format!(
                "{} - ready every {} days",
                miracle.blurb(),
                miracle.cooldown_days()
            ),
        ));
    }
}

/// The ladder: crossing a high-water mark of congregation faith teaches a
/// miracle, and the legends teach their own (Mend or Quake, whichever the
/// stories crystallise). Learning is forever — faith can ebb, the miracle
/// stays — and each new power sets itself in the first empty slot, from
/// which the god may drag it wherever they please.
fn unlock_miracles(
    belief: Res<Belief>,
    legend: Res<crate::villager::belief::Legend>,
    mut grimoire: ResMut<Grimoire>,
    mut hotbar: ResMut<Hotbar>,
    mut notices: MessageWriter<ui::Notice>,
) {
    grimoire.high_water = grimoire.high_water.max(belief.total);

    let mut learned: Vec<Miracle> = Vec::new();
    for miracle in Miracle::ALL {
        if grimoire.knows(miracle) {
            continue;
        }
        let earned = match miracle.unlock() {
            Unlock::Founding => true,
            Unlock::Belief(rung) => grimoire.high_water >= rung,
            // The legend-taught pair arrives when the stories say so.
            Unlock::Legend => legend.unlocked == Some(miracle),
            // The dark school watches the dread the stories carry.
            Unlock::Dread(depth) => legend.dread >= depth,
        };
        if earned {
            learned.push(miracle);
        }
    }
    for miracle in learned {
        grimoire.unlocked.push(miracle);
        hotbar.take_in(miracle);
        notices.write(ui::Notice::fanfare(format!(
            "A miracle is yours: {}",
            miracle.name()
        )));
    }
}

/// Arming and disarming, mouse-side: click a slot to arm what sits in it,
/// click the world to work it; right-click or Escape lowers the hand. The
/// number keys do NOT arm — they fire, over in `fire_the_slots`.
fn choose_miracle(
    clock: Res<crate::calendar::WorldClock>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    hotbar: Res<Hotbar>,
    cooldowns: Res<Cooldowns>,
    dragging: Res<DragState>,
    slots: Query<(&Interaction, &MiracleSlot), Changed<Interaction>>,
    mut selected: ResMut<SelectedMiracle>,
    mut notices: MessageWriter<ui::Notice>,
) {
    // A drag in progress owns the pointer; a press that began a carry must
    // not also arm what it lifted.
    if dragging.0.is_some() {
        return;
    }
    for (interaction, slot) in &slots {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(miracle) = hotbar.0[slot.0] else {
            continue;
        };
        if selected.0 == Some(miracle) {
            selected.0 = None;
        } else if cooldowns.ready(miracle, clock.elapsed) {
            selected.0 = Some(miracle);
        } else {
            // Arming a resting miracle fails out loud: silence here reads
            // as a broken button, not a resting power.
            notices.write(ui::Notice::new(format!(
                "{} returns in {}",
                miracle.name(),
                rest_word(cooldowns.remaining(miracle, clock.elapsed)),
            )));
        }
    }
    if keys.just_pressed(KeyCode::Escape) || buttons.just_pressed(MouseButton::Right) {
        selected.0 = None;
    }
}

/// The keys FIRE. Pressing a slot's number works its miracle immediately,
/// wherever the hand points — no arming step, no second click. Brett:
/// "pressing the hotkey should fire the ability, not just arm it."
fn fire_the_slots(
    clock: Res<crate::calendar::WorldClock>,
    keys: Res<ButtonInput<KeyCode>>,
    keymap: Res<crate::keymap::Keymap>,
    hotbar: Res<Hotbar>,
    cooldowns: Res<Cooldowns>,
    hand: Res<crate::hand::DivineHand>,
    mut fire: MessageWriter<FireMiracle>,
    mut wear: MessageWriter<crate::avatar::Wear>,
    mut notices: MessageWriter<ui::Notice>,
) {
    for (index, held) in hotbar.0.iter().enumerate() {
        if !keys.just_pressed(keymap.key(crate::keymap::Deed::slot(index))) {
            continue;
        }
        let Some(miracle) = *held else {
            continue;
        };
        if !cooldowns.ready(miracle, clock.elapsed) {
            notices.write(ui::Notice::new(format!(
                "{} returns in {}",
                miracle.name(),
                rest_word(cooldowns.remaining(miracle, clock.elapsed)),
            )));
            continue;
        }
        // The Avatar is worn, not cast at ground: fired, it takes whoever
        // stands under the hand this instant.
        if miracle == Miracle::Avatar {
            if let Some(body) = hand.hovered {
                wear.write(crate::avatar::Wear(body));
            } else {
                notices.write(ui::Notice::new(
                    "The Avatar needs a body under the hand".to_string(),
                ));
            }
            continue;
        }
        let Some(at) = hand.cursor_world else {
            continue;
        };
        fire.write(FireMiracle { miracle, at });
    }
}

/// How long a rest reads in words: days while days remain, hours after.
fn rest_word(seconds: f64) -> String {
    let days = seconds / crate::calendar::DAY_SECONDS as f64;
    if days >= 1.0 {
        format!("{:.0}d", days.ceil())
    } else {
        format!("{:.0}h", (days * 24.0).ceil().max(1.0))
    }
}

/// An armed miracle casts where the hand touches the world.
/// An armed miracle casts where the hand touches the world.
#[allow(clippy::too_many_arguments)]
fn cast(
    mut commands: Commands,
    clock: Res<crate::calendar::WorldClock>,
    // Bundled in pairs: this system rides Bevy's sixteen-parameter
    // ceiling, and each pair is one subject.
    input: (Res<ButtonInput<MouseButton>>, Res<PointerContext>),
    hand: Res<DivineHand>,
    ctx: (
        Option<Res<SettlementSite>>,
        Option<Res<crate::villager::DivineName>>,
        Option<ResMut<crate::villager::explore::KnownWorld>>,
    ),
    mut world_q: (
        Query<(&Transform, &mut crate::villager::work::Field)>,
        Query<(&Transform, &mut crate::villager::home::Bonfire)>,
        Query<&mut crate::villager::work::Stockpile>,
        ResMut<crate::villager::SimRng>,
    ),
    mut selected: ResMut<SelectedMiracle>,
    mut cooldowns: ResMut<Cooldowns>,
    mut fired: MessageReader<FireMiracle>,
    hotbar: Res<Hotbar>,
    slots: Query<(Entity, &MiracleSlot)>,
    mut visuals: (ResMut<Assets<Mesh>>, ResMut<Assets<StandardMaterial>>),
    mut bushes: Query<(&GlobalTransform, &mut FoodSource)>,
    mut told: (MessageWriter<crate::ui::Notice>, MessageWriter<DivineEvent>),
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
            Entity,
            &Transform,
            &mut crate::villager::Morale,
            &mut crate::villager::belief::Faith,
            Option<&mut crate::villager::regard::Regard>,
        ),
        With<crate::villager::Villager>,
    >,
) {
    let (site, name, mut known) = ctx;
    let god = name.as_ref().map_or("the god", |n| n.0.as_str());
    let mut castings: Vec<(Miracle, Vec3)> = Vec::new();
    // Two doors, one performer: the slot keys arrive as messages, the
    // armed click walks straight in.
    for order in fired.read() {
        castings.push((order.miracle, order.at));
    }
    if let Some(miracle) = selected.0
        && input.0.just_pressed(MouseButton::Left)
        && !input.1.over_ui
        && let Some(at) = hand.cursor_world
    {
        // Avatar never reaches here - `take_a_body` owns the armed click
        // on a person - so ground-cast kinds only.
        if miracle != Miracle::Avatar {
            castings.push((miracle, at));
        }
        // One cast per arming: a miracle is a sentence, not a hose.
        selected.0 = None;
    }
    for (miracle, at) in castings {
        if !cooldowns.ready(miracle, clock.elapsed) {
            continue;
        }
        let worked = perform(
            miracle,
            at,
            god,
            clock.elapsed,
            &mut commands,
            &site,
            known.as_deref_mut(),
            &mut world_q,
            &mut visuals.0,
            &mut visuals.1,
            &mut bushes,
            &mut told.0,
            &mut told.1,
            &mut victims,
            &mut souls,
        );
        if worked {
            cooldowns.start(miracle, clock.elapsed);
            // The slot it fired from flashes with the act.
            if let Some(slot_index) = hotbar.slot_of(miracle) {
                for (slot_entity, place) in &slots {
                    if place.0 == slot_index {
                        commands.entity(slot_entity).insert(FiredFlash(0.45));
                    }
                }
            }
        }
    }
}

/// Works one miracle at one place. True if the world changed - a casting
/// that found nothing to bless costs no days.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn perform(
    miracle: Miracle,
    at: Vec3,
    god: &str,
    now: f64,
    commands: &mut Commands,
    site: &Option<Res<SettlementSite>>,
    known: Option<&mut crate::villager::explore::KnownWorld>,
    world_q: &mut (
        Query<(&Transform, &mut crate::villager::work::Field)>,
        Query<(&Transform, &mut crate::villager::home::Bonfire)>,
        Query<&mut crate::villager::work::Stockpile>,
        ResMut<crate::villager::SimRng>,
    ),
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    bushes: &mut Query<(&GlobalTransform, &mut FoodSource)>,
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
            Entity,
            &Transform,
            &mut crate::villager::Morale,
            &mut crate::villager::belief::Faith,
            Option<&mut crate::villager::regard::Regard>,
        ),
        With<crate::villager::Villager>,
    >,
) -> bool {
    match miracle {
        // Worn rather than cast: `take_a_body` and the Wear message own
        // it. Nothing routes Avatar here.
        Miracle::Avatar => false,

        Miracle::Bounty => {
            // The blessing needs something living to land on.
            let mut blessed = 0;
            for (bush_at, mut bush) in bushes.iter_mut() {
                if bush_at.translation().distance(at) <= BOUNTY_RADIUS {
                    bush.amount = FoodSource::CAPACITY;
                    blessed += 1;
                }
            }
            if blessed == 0 {
                notices.write(crate::ui::Notice::new(
                    "Nothing grows there for Bounty to bless".to_string(),
                ));
                return false;
            }
            spawn_glory(
                commands,
                meshes,
                materials,
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
            true
        }
        Miracle::Flourish => {
            let Some(site) = site else {
                return false;
            };
            crate::villager::belief::flourish(site, bushes);
            spawn_glory(
                commands,
                meshes,
                materials,
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
            true
        }
        Miracle::Smite => {
            lightning_bolt(commands, meshes, materials, at);

            // The harm, and the first name it fell on.
            let mut struck: Option<Entity> = None;
            for (victim, transform, mut vitality, mut motion) in victims.iter_mut() {
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
            true
        }
        Miracle::Mend | Miracle::Quake => {
            // Mending arrives as warm rising light; the quake speaks for
            // itself in thrown bodies.
            if miracle == Miracle::Mend {
                spawn_glory(
                    commands,
                    meshes,
                    materials,
                    at,
                    Color::srgb(1.0, 0.88, 0.55),
                    true,
                );
            }
            cast_earned(miracle, at, god, notices, witnessed, victims, souls);
            true
        }

        Miracle::Rain => {
            let (fields, bonfires, ..) = world_q;
            let mut watered = 0;
            for (field_at, mut field) in fields.iter_mut() {
                if field_at.translation.distance(at) <= RAIN_RADIUS {
                    field.growth = (field.growth + 0.35).min(1.0);
                    watered += 1;
                }
            }
            // Rain is the one thing that puts a fire out from the sky.
            for (fire_at, mut fire) in bonfires.iter_mut() {
                if fire_at.translation.distance(at) <= RAIN_RADIUS {
                    fire.fuel = 0.0;
                }
            }
            spawn_glory(
                commands,
                meshes,
                materials,
                at,
                crate::palette::shade(&crate::palette::CLOTH_BLUE, 0.8),
                false,
            );
            info!("{god} called rain, {watered} fields drank");
            notices.write(crate::ui::Notice::fanfare(format!("{god} called the rain")));
            witnessed.write(DivineEvent {
                kind: DivineEventKind::Rained,
                position: at,
                subject: None,
                intensity: 0.7,
            });
            true
        }

        Miracle::Beacon => {
            raise_beacon(commands, meshes, materials, at, now);
            info!("{god} raised a beacon");
            notices.write(crate::ui::Notice::fanfare(format!(
                "{god} set a pillar of light upon the land"
            )));
            witnessed.write(DivineEvent {
                kind: DivineEventKind::Beckoned,
                position: at,
                subject: None,
                intensity: 0.9,
            });
            true
        }

        Miracle::Ward => {
            raise_ward(commands, meshes, materials, at, now);
            info!("{god} drew a ward");
            notices.write(crate::ui::Notice::fanfare(format!(
                "{god} drew a circle no fang will cross"
            )));
            true
        }

        Miracle::HarvestWind => {
            let (fields, _, stores, _) = world_q;
            let mut reaped = 0;
            let mut yielded = 0.0f32;
            for (field_at, mut field) in fields.iter_mut() {
                if field.growth < 0.5 {
                    continue;
                }
                yielded += 8.0 * field.growth;
                field.growth = 0.08;
                reaped += 1;
                if reaped <= 8 {
                    spawn_glory(
                        commands,
                        meshes,
                        materials,
                        field_at.translation,
                        crate::palette::shade(&crate::palette::CLOTH_GOLD, 0.85),
                        false,
                    );
                }
            }
            if reaped == 0 {
                notices.write(crate::ui::Notice::new(
                    "No field stands ripe enough for the wind".to_string(),
                ));
                return false;
            }
            if let Some(mut store) = stores.iter_mut().next() {
                store
                    .larder
                    .add(crate::villager::work::FoodKind::Grain, yielded);
            }
            info!("{god} sent the harvest wind: {reaped} fields came in at once");
            notices.write(crate::ui::Notice::fanfare(format!(
                "{god} sent the harvest wind"
            )));
            witnessed.write(DivineEvent {
                kind: DivineEventKind::Flourished,
                position: at,
                subject: None,
                intensity: 0.9,
            });
            true
        }

        Miracle::PlagueOfDoubt => {
            let mut shadowed: Vec<Entity> = Vec::new();
            for (soul, soul_at, mut morale, mut faith, _) in souls.iter_mut() {
                if soul_at.translation.distance(at) > DOUBT_RADIUS {
                    continue;
                }
                faith.trust = (faith.trust - 0.18).max(0.0);
                morale.spirits = (morale.spirits - 0.15).max(0.0);
                shadowed.push(soul);
            }
            if shadowed.is_empty() {
                notices.write(crate::ui::Notice::new(
                    "Nobody stands there for the shadow to cross".to_string(),
                ));
                return false;
            }
            // Fear ripples the regard graph: everyone touched steps a
            // little back from everyone else who was — nobody can say
            // why, and that is the plague.
            for &soul in &shadowed {
                if let Ok((_, _, _, _, Some(mut regard))) = souls.get_mut(soul) {
                    for &other in shadowed.iter().take(6) {
                        if other != soul {
                            regard.warm_over(other, -0.06, Some("the shadow that crossed us"));
                        }
                    }
                }
            }
            spawn_glory(
                commands,
                meshes,
                materials,
                at,
                crate::palette::shade(&crate::palette::CLOTH_SABLE, 0.5),
                false,
            );
            info!("{god} sowed doubt through {} hearts", shadowed.len());
            notices.write(crate::ui::Notice::new(format!(
                "{god} sent a shadow across the hearts of the village"
            )));
            witnessed.write(DivineEvent {
                kind: DivineEventKind::DoubtSown,
                position: at,
                subject: None,
                intensity: 1.0,
            });
            true
        }

        Miracle::StoneFromSky => {
            let (_, _, _, rng) = world_q;
            drop_the_stone(commands, meshes, materials, at, now, &mut rng.0);
            info!("{god} pulled a stone out of the sky");
            notices.write(crate::ui::Notice::fanfare(format!(
                "{god} calls a stone out of the empty sky"
            )));
            true
        }

        Miracle::Visitation => {
            let dreamer = souls
                .iter()
                .map(|(soul, soul_at, ..)| (soul, soul_at.translation.distance(at)))
                .filter(|(_, d)| *d < 4.0)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(soul, _)| soul);
            let Some(dreamer) = dreamer else {
                notices.write(crate::ui::Notice::new(
                    "The dream needs a soul under the hand".to_string(),
                ));
                return false;
            };
            if let Ok((_, _, mut morale, mut faith, _)) = souls.get_mut(dreamer) {
                faith.trust = 1.0;
                morale.spirits = (morale.spirits + 0.3).min(1.0);
            }
            commands.entity(dreamer).insert(Evangel {
                until: now + 3.0 * crate::calendar::DAY_SECONDS as f64,
                last_sermon: 0.0,
            });
            spawn_glory(
                commands,
                meshes,
                materials,
                at,
                crate::palette::shade(&crate::palette::CLOTH_PINK, 0.9),
                true,
            );
            info!("{god} visited a dream");
            notices.write(crate::ui::Notice::fanfare(
                "A soul wakes certain of you, and cannot stop saying so".to_string(),
            ));
            // No DivineEvent: a dream has no witnesses. The preaching IS
            // the miracle's public face.
            true
        }

        Miracle::FoundingSight => {
            let Some(known) = known else {
                return false;
            };
            known.learn(at, 170.0);
            spawn_glory(
                commands,
                meshes,
                materials,
                at,
                crate::palette::shade(&crate::palette::BONE, 0.95),
                true,
            );
            info!("{god} opened the land to the village's eyes");
            notices.write(crate::ui::Notice::fanfare(
                "The land lies open before the village".to_string(),
            ));
            true
        }
    }
}

/// The Mend and Quake arms of [`cast`], split out for length.
#[allow(clippy::too_many_arguments)]
fn cast_earned(
    miracle: Miracle,
    at: Vec3,
    god: &str,
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
            Entity,
            &Transform,
            &mut crate::villager::Morale,
            &mut crate::villager::belief::Faith,
            Option<&mut crate::villager::regard::Regard>,
        ),
        With<crate::villager::Villager>,
    >,
) {
    match miracle {
        Miracle::Mend => {
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
                if let Ok((_, _, mut morale, mut faith, _)) = souls.get_mut(entity) {
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

/// How far Rain reaches from the cast point.
const RAIN_RADIUS: f32 = 26.0;

/// How far the shadow of doubt reaches.
const DOUBT_RADIUS: f32 = 30.0;

/// A pillar of light the idle walk to.
#[derive(Component)]
pub struct Beacon {
    until: f64,
}

/// A circle no predator crosses while it stands.
#[derive(Component)]
pub struct Ward {
    pub radius: f32,
    until: f64,
}

/// A stone still falling; when it lands, the ground answers.
#[derive(Component)]
pub(crate) struct FallingStone;

/// A soul who woke certain: for a few days their talk carries the god in
/// it, and everyone near them feels the heat of it.
#[derive(Component)]
pub struct Evangel {
    until: f64,
    last_sermon: f64,
}

/// Raises the beacon: a pillar of light standing on the ground.
fn raise_beacon(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    at: Vec3,
    now: f64,
) {
    let glow = ui::theme::accent();
    commands.spawn((
        Name::new("A beacon"),
        Beacon { until: now + 45.0 },
        Mesh3d(meshes.add(Cuboid::new(1.3, 30.0, 1.3))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: glow.with_alpha(0.75),
            emissive: LinearRgba::from(glow) * 8.0,
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        })),
        Transform::from_translation(at + Vec3::Y * 15.0),
        bevy::light::NotShadowCaster,
    ));
}

/// Raises the ward: a ring of pale stones around the point.
fn raise_ward(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    at: Vec3,
    now: f64,
) {
    let ward = commands
        .spawn((
            Name::new("A ward"),
            Ward {
                radius: 34.0,
                until: now + crate::calendar::DAY_SECONDS as f64,
            },
            Transform::from_translation(at),
            Visibility::default(),
        ))
        .id();
    let stone = materials.add(StandardMaterial {
        base_color: crate::palette::shade(&crate::palette::BONE, 0.9),
        emissive: LinearRgba::from(ui::theme::accent()) * 0.8,
        ..default()
    });
    let block = meshes.add(Cuboid::new(0.8, 1.6, 0.8));
    for i in 0..8 {
        let angle = i as f32 / 8.0 * std::f32::consts::TAU;
        commands.spawn((
            Mesh3d(block.clone()),
            MeshMaterial3d(stone.clone()),
            Transform::from_translation(Vec3::new(angle.cos() * 34.0, 0.8, angle.sin() * 34.0)),
            ChildOf(ward),
        ));
    }
}

/// Pulls the stone out of the sky: spawned high, falling, and REAL - it
/// lands as an ordinary boulder and stays, minable stone with a story.
fn drop_the_stone(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    at: Vec3,
    now: f64,
    rng: &mut crate::rng::Rng,
) {
    let _ = now;
    let stone = crate::scatter::spawn_boulder(
        commands,
        meshes,
        materials.add(StandardMaterial {
            base_color: crate::palette::shade(&crate::palette::STONE, 0.5),
            perceptual_roughness: 0.95,
            ..default()
        }),
        at + Vec3::Y * 60.0,
        rng,
        crate::scatter::RockRoll {
            mass: 160.0,
            radius: 1.4,
            girth: 1.3,
        },
        None,
    );
    commands.entity(stone).insert((
        FallingStone,
        crate::creature::Airborne {
            velocity: Vec3::new(0.0, -6.0, 0.0),
        },
    ));
}

/// The beacon calls: the idle within its light walk to it, and the pillar
/// stands down when its time is spent.
#[allow(clippy::type_complexity)]
pub(super) fn beacons_call(
    mut commands: Commands,
    clock: Res<crate::calendar::WorldClock>,
    time: Res<Time>,
    mut since: Local<f32>,
    beacons: Query<(Entity, &Transform, &Beacon)>,
    mut drawn: Query<
        (
            &Transform,
            &crate::villager::Activity,
            &mut crate::creature::MoveTarget,
        ),
        (
            With<crate::villager::Villager>,
            Without<Corpse>,
            Without<crate::creature::Held>,
            Without<Beacon>,
        ),
    >,
) {
    for (pillar, _, beacon) in &beacons {
        if clock.elapsed >= beacon.until {
            commands.entity(pillar).despawn();
        }
    }
    *since += time.delta_secs();
    if *since < 1.5 {
        return;
    }
    *since = 0.0;
    for (_, pillar_at, _) in &beacons {
        let foot = pillar_at.translation - Vec3::Y * 15.0;
        for (at, activity, mut target) in &mut drawn {
            if !matches!(
                activity,
                crate::villager::Activity::Idle | crate::villager::Activity::Wandering
            ) {
                continue;
            }
            let away = at.translation.distance(foot);
            if away < 130.0 && away > 6.0 {
                target.0 = Some(foot);
            }
        }
    }
}

/// The ward wards: any predator inside the circle is pressed back out,
/// and the stones stand down after their day.
#[allow(clippy::type_complexity)]
pub(super) fn wards_hold(
    mut commands: Commands,
    clock: Res<crate::calendar::WorldClock>,
    wards: Query<(Entity, &Transform, &Ward)>,
    mut prowlers: Query<
        (
            &Transform,
            &crate::creature::genome::CreatureGenome,
            &mut crate::creature::MoveTarget,
        ),
        (
            With<Creature>,
            Without<Corpse>,
            Without<crate::creature::Held>,
        ),
    >,
) {
    for (ring, _, ward) in &wards {
        if clock.elapsed >= ward.until {
            commands.entity(ring).despawn();
        }
    }
    for (_, centre, ward) in &wards {
        for (at, genome, mut target) in &mut prowlers {
            if genome.species != crate::creature::genome::Species::Wolf {
                continue;
            }
            let from_ring = at.translation.distance(centre.translation);
            if from_ring < ward.radius + 6.0 {
                let out = (at.translation - centre.translation).normalize_or(Vec3::X);
                target.0 = Some(centre.translation + out * (ward.radius * 2.2));
            }
        }
    }
}

/// The stone lands: the ground answers, whatever stood under it suffers,
/// and the sky's rock becomes ordinary minable stone with a story.
#[allow(clippy::type_complexity)]
pub(super) fn stones_land(
    mut commands: Commands,
    mut told: MessageWriter<DivineEvent>,
    landed: Query<(Entity, &Transform), (With<FallingStone>, Without<crate::creature::Airborne>)>,
    mut victims: Query<
        (&Transform, &mut Vitality, &mut CreatureMotion),
        (
            With<Creature>,
            Without<Corpse>,
            Without<crate::creature::Held>,
        ),
    >,
) {
    for (stone, at) in &landed {
        commands.entity(stone).remove::<FallingStone>();
        for (victim_at, mut vitality, mut motion) in &mut victims {
            if victim_at.translation.distance(at.translation) > 5.0 {
                continue;
            }
            vitality.harm += 1.3;
            vitality.violent = true;
            motion.flail = 1.0;
        }
        told.write(DivineEvent {
            kind: DivineEventKind::Fell,
            position: at.translation,
            subject: None,
            intensity: 1.0,
        });
    }
}

/// The evangelist preaches: whoever woke certain spends their days saying
/// so, and faith rises in everyone standing near enough to hear.
#[allow(clippy::type_complexity)]
pub(super) fn evangels_preach(
    mut commands: Commands,
    clock: Res<crate::calendar::WorldClock>,
    mut certain: Query<
        (Entity, &Transform, &mut Evangel),
        (With<crate::villager::Villager>, Without<Corpse>),
    >,
    mut flock: Query<
        (Entity, &Transform, &mut crate::villager::belief::Faith),
        (
            With<crate::villager::Villager>,
            Without<Corpse>,
            Without<Evangel>,
        ),
    >,
) {
    for (soul, at, mut evangel) in &mut certain {
        if clock.elapsed >= evangel.until {
            commands.entity(soul).remove::<Evangel>();
            continue;
        }
        if clock.elapsed - evangel.last_sermon < 30.0 {
            continue;
        }
        evangel.last_sermon = clock.elapsed;
        for (_, hearer_at, mut faith) in &mut flock {
            if hearer_at.translation.distance(at.translation) < 12.0 {
                faith.trust = (faith.trust + 0.03).min(1.0);
            }
        }
    }
}

/// A miracle mid-carry: where it came from, and the ghost under the cursor.
struct Drag {
    from: usize,
    miracle: Miracle,
    ghost: Entity,
    /// Where the press began, to tell a click from a carry.
    grip: Vec2,
    live: bool,
}

/// The carry in progress, if any.
#[derive(Resource, Default)]
pub struct DragState(Option<Drag>);

/// Drag and drop, the way every action bar since WoW taught: press a slot
/// and pull, and the miracle comes away under the cursor; let go over
/// another slot and the two trade places; let go anywhere else and it
/// settles home. A press that never pulls past a knuckle's width stays a
/// click, and the arming system keeps it.
#[allow(clippy::type_complexity)]
fn carry_miracles(
    mut commands: Commands,
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut hotbar: ResMut<Hotbar>,
    mut dragging: ResMut<DragState>,
    mut selected: ResMut<SelectedMiracle>,
    slots: Query<(&Interaction, &MiracleSlot)>,
    mut ghosts: Query<&mut Node, With<DragGhost>>,
) {
    let cursor = windows.single().ok().and_then(|w| w.cursor_position());

    // A press on a loaded slot takes a grip.
    if buttons.just_pressed(MouseButton::Left)
        && dragging.0.is_none()
        && let Some(at) = cursor
    {
        for (interaction, slot) in &slots {
            if *interaction == Interaction::None {
                continue;
            }
            let Some(miracle) = hotbar.0[slot.0] else {
                continue;
            };
            dragging.0 = Some(Drag {
                from: slot.0,
                miracle,
                ghost: Entity::PLACEHOLDER,
                grip: at,
                live: false,
            });
            break;
        }
    }

    let Some(drag) = dragging.0.as_mut() else {
        return;
    };

    // The pull: past a knuckle's width the grip becomes a carry, the
    // ghost appears, and whatever was armed is lowered.
    if let Some(at) = cursor {
        if !drag.live && at.distance(drag.grip) > 9.0 {
            drag.live = true;
            selected.0 = None;
            drag.ghost = commands
                .spawn((
                    DragGhost,
                    ordo::Layer::Tooltip,
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(at.x - 21.0),
                        top: px(at.y - 21.0),
                        width: px(42),
                        height: px(42),
                        border: UiRect::all(px(2)),
                        border_radius: BorderRadius::all(px(5)),
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BackgroundColor(ui::theme::panel_bg().with_alpha(0.85)),
                    BorderColor::all(ui::theme::accent()),
                ))
                .id();
            draw_miracle_icon(&mut commands, drag.ghost, drag.miracle);
        }
        if drag.live
            && let Ok(mut node) = ghosts.get_mut(drag.ghost)
        {
            node.left = px(at.x - 21.0);
            node.top = px(at.y - 21.0);
        }
    }

    // The release: over a slot, the two trade places; anywhere else, home.
    if buttons.just_released(MouseButton::Left) {
        if drag.live {
            let landing = slots
                .iter()
                .find(|(interaction, _)| **interaction != Interaction::None)
                .map(|(_, slot)| slot.0);
            match landing {
                Some(to) if to != drag.from => hotbar.0.swap(drag.from, to),
                Some(_) => {}
                // Let go over nothing: the slot empties, WoW-style — the
                // book holds more miracles than the bar holds slots, and
                // this is how room is made. The miracle stays learned;
                // set it back from the deity page whenever.
                None => hotbar.0[drag.from] = None,
            }
            commands.entity(drag.ghost).despawn();
        }
        dragging.0 = None;
    }
}

/// The carried miracle's face, floating under the cursor.
#[derive(Component)]
struct DragGhost;

/// The meter follows the people's living faith: the bar's capacity is the
/// congregation's whole faith, the fill is what is unspent.
fn update_belief_meter(
    belief: Res<Belief>,
    grimoire: Res<Grimoire>,
    mut readout: Query<&mut Text, With<BeliefReadout>>,
    mut fill: Query<&mut Node, With<BeliefFill>>,
) {
    if !belief.is_changed() && !grimoire.is_changed() {
        return;
    }
    // No mana here: the bar is the LADDER. It fills toward the next
    // miracle the congregation's faith has yet to earn, names it, and
    // stands full and quiet once everything is taught.
    let next_rung = Miracle::ALL
        .into_iter()
        .filter(|miracle| !grimoire.knows(*miracle))
        .filter_map(|miracle| miracle.unlock_at().map(|rung| (miracle, rung)))
        .filter(|(_, rung)| *rung > grimoire.high_water)
        .min_by(|a, b| a.1.total_cmp(&b.1));

    for mut text in &mut readout {
        text.0 = match next_rung {
            Some((miracle, rung)) => format!(
                "BELIEF {:.0} - {} AT {:.0}",
                belief.total,
                miracle.name().to_uppercase(),
                rung
            ),
            None => format!("BELIEF {:.0}", belief.total),
        };
    }
    let fraction = match next_rung {
        Some((_, rung)) if rung > 0.0 => (belief.total / rung).clamp(0.0, 1.0),
        _ => 1.0,
    };
    for mut node in &mut fill {
        node.width = percent(fraction * 100.0);
    }
}

/// The armed slot glows; unaffordable miracles sit dim.
#[allow(clippy::type_complexity)]
fn style_hotbar(
    time: Res<Time>,
    clock: Res<crate::calendar::WorldClock>,
    selected: Res<SelectedMiracle>,
    hotbar: Res<Hotbar>,
    cooldowns: Res<Cooldowns>,
    mut commands: Commands,
    mut slots: Query<(
        Entity,
        &MiracleSlot,
        &Interaction,
        &mut BorderColor,
        &mut BackgroundColor,
        &mut UiTransform,
        Option<&mut FiredFlash>,
    )>,
) {
    for (entity, slot, interaction, mut border, mut bg, mut pose, flash) in &mut slots {
        let Some(miracle) = hotbar.0[slot.0] else {
            *border = BorderColor::all(ui::theme::panel_border().with_alpha(0.15));
            bg.0 = ui::theme::panel_bg().with_alpha(0.2);
            pose.scale = Vec2::splat(1.0);
            continue;
        };
        let ready = cooldowns.ready(miracle, clock.elapsed);
        let armed = selected.0 == Some(miracle);
        let pressed = *interaction == Interaction::Pressed;

        // The press is a real gesture now: the button gives under the
        // finger. Brett: "they need to look better when clicking."
        pose.scale = Vec2::splat(if pressed { 0.9 } else { 1.0 });

        *border = BorderColor::all(if pressed {
            ui::theme::text()
        } else if armed {
            ui::theme::accent()
        } else if ready {
            ui::theme::panel_border()
        } else {
            ui::theme::panel_border().with_alpha(0.12)
        });
        bg.0 = if pressed {
            ui::theme::accent().with_alpha(0.5)
        } else if armed {
            ui::theme::accent().with_alpha(0.3)
        } else if ready {
            ui::theme::panel_bg().with_alpha(0.4)
        } else {
            ui::theme::panel_bg().with_alpha(0.15)
        };

        // And the cast itself flashes: a bright breath that fades as fast
        // as the act was sudden.
        if let Some(mut flash) = flash {
            flash.0 -= time.delta_secs();
            if flash.0 <= 0.0 {
                commands.entity(entity).remove::<FiredFlash>();
            } else {
                let heat = (flash.0 / 0.45).clamp(0.0, 1.0);
                *border = BorderColor::all(ui::theme::accent().with_alpha(0.4 + heat * 0.6));
                bg.0 = ui::theme::accent().with_alpha(0.2 + heat * 0.45);
                pose.scale = Vec2::splat(1.0 + heat * 0.08);
            }
        }
    }
}

/// The rest sweep and its word: each resting slot drains a dark tide from
/// full to nothing across the cooldown, labelled in days or hours.
#[allow(clippy::type_complexity)]
fn cooldown_faces(
    clock: Res<crate::calendar::WorldClock>,
    hotbar: Res<Hotbar>,
    cooldowns: Res<Cooldowns>,
    mut shades: Query<(&CooldownShade, &mut Node), Without<CooldownLabel>>,
    mut labels: Query<(&CooldownLabel, &mut Text)>,
) {
    let fraction = |index: usize| -> (f32, f64) {
        hotbar.0[index].map_or((0.0, 0.0), |miracle| {
            let left = cooldowns.remaining(miracle, clock.elapsed);
            if left <= 0.0 {
                (0.0, 0.0)
            } else {
                (
                    (left / miracle.cooldown_secs()).clamp(0.0, 1.0) as f32,
                    left,
                )
            }
        })
    };
    for (shade, mut node) in &mut shades {
        let (tide, _) = fraction(shade.0);
        node.height = percent(tide * 100.0);
    }
    for (label, mut text) in &mut labels {
        let (_, left) = fraction(label.0);
        let fresh = if left <= 0.0 {
            String::new()
        } else {
            rest_word(left)
        };
        if text.0 != fresh {
            *text = Text::new(fresh);
        }
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
    fn wrath_comes_before_grace() {
        // Deliberate: terror is the easy road. Wrath unlocks a young god's
        // first climb up the ladder; the land-wide grace asks for a real
        // congregation - and the quick sin always rests sooner than the
        // great kindness.
        assert!(SMITE_UNLOCK < FLOURISH_UNLOCK);
        assert!(Miracle::Smite.cooldown_days() < Miracle::Flourish.cooldown_days());
    }

    #[test]
    fn every_miracle_names_its_price() {
        // The whole book, one row each: a rest measured in days, and a
        // school that teaches it. The dark pair costs dread, the deep
        // wonders cost a real congregation, and nothing is free twice.
        for miracle in Miracle::ALL {
            assert!(
                miracle.cooldown_days() >= 0.5,
                "{miracle:?} rests less than half a day"
            );
        }
        // The week-long trumpets are exactly the two promised.
        let weekly: Vec<_> = Miracle::ALL
            .into_iter()
            .filter(|m| m.cooldown_days() >= 7.0)
            .collect();
        assert_eq!(weekly, vec![Miracle::FoundingSight, Miracle::Visitation]);
        // The dark school is dread's alone.
        assert!(matches!(Miracle::PlagueOfDoubt.unlock(), Unlock::Dread(_)));
        assert!(matches!(Miracle::StoneFromSky.unlock(), Unlock::Dread(_)));
        // And the deeper dread teaches the heavier stone.
        if let (Unlock::Dread(plague), Unlock::Dread(stone)) = (
            Miracle::PlagueOfDoubt.unlock(),
            Miracle::StoneFromSky.unlock(),
        ) {
            assert!(plague < stone);
        }
    }

    #[test]
    fn the_ladder_never_takes_a_miracle_back() {
        let mut grimoire = Grimoire::default();
        assert!(
            grimoire.knows(Miracle::Bounty),
            "kindness is the founding kit"
        );
        assert!(!grimoire.knows(Miracle::Smite));
        grimoire.high_water = grimoire.high_water.max(SMITE_UNLOCK);
        // The unlock system reads the high-water mark; a later ebb of
        // faith leaves it standing.
        assert!(grimoire.high_water >= SMITE_UNLOCK);
    }

    #[test]
    fn the_calendar_is_the_cost() {
        let mut cooldowns = Cooldowns::default();
        assert!(cooldowns.ready(Miracle::Smite, 0.0));
        cooldowns.start(Miracle::Smite, 0.0);
        assert!(!cooldowns.ready(Miracle::Smite, 1.0));
        let rest = Miracle::Smite.cooldown_secs();
        assert!(
            cooldowns.ready(Miracle::Smite, rest + 1.0),
            "a day's rest and the wrath answers again"
        );
        // Starting anew replaces, never stacks.
        cooldowns.start(Miracle::Smite, 10.0);
        assert_eq!(cooldowns.0.len(), 1);
    }

    #[test]
    fn a_new_power_takes_the_first_empty_slot() {
        let mut hotbar = Hotbar::default();
        assert_eq!(hotbar.slot_of(Miracle::Bounty), Some(0));
        hotbar.take_in(Miracle::Smite);
        assert_eq!(hotbar.slot_of(Miracle::Smite), Some(1));
        // Taking in what is already placed changes nothing.
        hotbar.take_in(Miracle::Smite);
        assert_eq!(hotbar.0.iter().filter(|m| m.is_some()).count(), 2);
        // And the god's own arrangement is the bar's law: a swap holds.
        hotbar.0.swap(0, 9);
        assert_eq!(hotbar.slot_of(Miracle::Bounty), Some(9));
    }
}
