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
            .add_systems(
                Update,
                (
                    reveal_earned_miracle,
                    choose_miracle,
                    cast,
                    style_hotbar,
                    update_belief_meter,
                    fade_bolts,
                )
                    .chain()
                    .after(CameraSet),
            );
    }
}

/// The miracles the god can work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Miracle {
    Flourish,
    Smite,
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
            Miracle::Mend => "Mend",
            Miracle::Quake => "Quake",
        }
    }

    pub fn cost(self) -> f32 {
        match self {
            Miracle::Flourish => FLOURISH_COST,
            Miracle::Smite => SMITE_COST,
            Miracle::Mend => MEND_COST,
            Miracle::Quake => QUAKE_COST,
        }
    }

    pub fn key(self) -> KeyCode {
        match self {
            Miracle::Flourish => KeyCode::Digit1,
            Miracle::Smite => KeyCode::Digit2,
            // Whichever of the pair is earned takes the third slot.
            Miracle::Mend | Miracle::Quake => KeyCode::Digit3,
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
    let strip = ui::centered_strip(&mut commands, Val::Auto, px(ui::theme::MARGIN));
    let column = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(3),
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
        None,
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

        // The number in the corner, hotkey-style.
        let number = commands
            .spawn((
                ui::dim(format!("{}", index + 1)),
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
        Miracle::Mend,
        Miracle::Quake,
    ] {
        if keys.just_pressed(miracle.key()) && all_slots.iter().any(|s| s.0 == Some(miracle)) {
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
        Miracle::Flourish => {
            let Some(site) = site else {
                return;
            };
            if !crate::villager::belief::flourish(&mut belief, &site, &mut bushes) {
                return;
            }
            info!("{god} made the land flourish");
            notices.write(crate::ui::Notice::fanfare(format!(
                "{god} made the land flourish"
            )));
            witnessed.write(DivineEvent {
                kind: DivineEventKind::Provided,
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

            // The bolt: jagged white-hot segments from the sky to the point.
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
                    Transform::from_xyz(sway, height + segment * 0.5, sway * 0.6)
                        .with_scale(Vec3::new(0.5, segment + 1.0, 0.5)),
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

            // The harm, and the first name it fell on.
            let mut struck: Option<Entity> = None;
            for (victim, transform, mut vitality, mut motion) in &mut victims {
                if transform.translation.distance(at) > SMITE_RADIUS {
                    continue;
                }
                vitality.harm += 1.2;
                vitality.violent = true;
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
