//! Belief: prayer, providence, and faith with receipts.
//!
//! This is the game's thesis made mechanical. A villager who cannot feed
//! themself kneels and asks their god — by name — for food. The prayer is a
//! window: answer it with the hand (food, set down beside them, still warm
//! from your grip) and their faith deepens; let it lapse and doubt takes the
//! difference. Witnesses to an answered prayer believe a little more for
//! having seen it, and the story travels by gossip to people who saw nothing.
//!
//! Faith never moves silently. Every change writes a line in the person's own
//! chronicle first — "prayed, and food came", "prayed, and no answer came" —
//! so the inspector can always explain a believer the way Dwarf Fortress can
//! explain a tantrum. The number is internal; the *reasons* are the interface.
//!
//! The sum of every living villager's faith is the god's strength. The first
//! thing it buys: the Flourish miracle, which fills every bush around the
//! settlement — a bigger answer, purchased with the credibility earned from
//! small ones.

use bevy::prelude::*;

use super::{Activity, Chronicle, DivineName, MemberOf, Needs, Person, SettlementSite, Villager};
use crate::creature::{Airborne, Corpse, Held, MoveTarget};
use crate::hand::DivinelyPlaced;
use crate::scatter::FoodSource;
use crate::witness::{DivineEvent, DivineEventKind};

/// Hunger past this, with nothing to eat anywhere, sends a person to their knees.
const DESPERATE_HUNGER: f32 = 0.65;

/// How long a prayer stays open before it curdles into doubt.
const PRAYER_PATIENCE: f32 = 75.0;

/// How near the answer must land to be *their* answer.
const ANSWER_RADIUS: f32 = 9.0;

/// Belief the Flourish miracle costs.
pub const FLOURISH_COST: f32 = 10.0;

/// One person's faith in the god, 0 to 1.
///
/// Newly made people start believing a little — the god was, after all, built
/// from their hope. Everything after that is earned or lost.
#[derive(Component, Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct Faith {
    pub trust: f32,
}

impl Default for Faith {
    fn default() -> Self {
        Faith { trust: 0.35 }
    }
}

impl Faith {
    /// The faith, in the person's terms.
    pub fn describe(&self) -> &'static str {
        match self.trust {
            t if t > 0.75 => "sure of you",
            t if t > 0.5 => "believes",
            t if t > 0.25 => "wavering",
            _ => "doubts you",
        }
    }

    /// Above this, a person counts as a believer: the ledger, conversion
    /// notices, and ascension all read the same line.
    pub const BELIEVER: f32 = 0.45;

    pub fn is_believer(&self) -> bool {
        self.trust > Self::BELIEVER
    }

    fn shift(&mut self, amount: f32) {
        self.trust = (self.trust + amount).clamp(0.0, 1.0);
    }
}

/// An open prayer: what they are asking for, and how long hope lasts.
#[derive(Component, Debug)]
pub struct Prayer {
    pub remaining: f32,
}

/// The visible mote of a prayer, hanging over the praying — the player's cue
/// that someone, somewhere, is asking.
#[derive(Component)]
pub struct PrayerMote;

/// The god's pooled strength: the sum of every living believer's faith, less
/// what has been spent on miracles.
#[derive(Resource, Default, serde::Serialize, serde::Deserialize)]
pub struct Belief {
    pub total: f32,
    pub spent: f32,
}

impl Belief {
    pub fn available(&self) -> f32 {
        (self.total - self.spent).max(0.0)
    }

    /// Spent belief returns as the faithful keep believing: a congregation
    /// with a faith-sum of 6 restores 6 belief in about two minutes. Without
    /// this, `spent` accrues forever and a small village is one Smite away
    /// from a god locked out of miracles for life.
    pub fn regenerate(&mut self, faith_sum: f32, dt: f32) {
        self.spent = (self.spent - faith_sum * dt / 120.0).max(0.0);
    }
}

/// The desperate kneel and pray.
///
/// Desperation means real absence: hungry, an empty store, and no fruiting
/// bush within reach. A player who steals a village's bushes *causes* prayer —
/// famine is a lever, and so is generosity.
pub(super) fn kneel(
    mut commands: Commands,
    clock: Res<crate::calendar::WorldClock>,
    name: Option<Res<DivineName>>,
    members: Query<&MemberOf>,
    stores: Query<&super::work::Stockpile>,
    bushes: Query<(&GlobalTransform, &FoodSource)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut notices: MessageWriter<crate::ui::Notice>,
    // Bundled: some of these systems already press Bevy's parameter ceiling.
    mut telling: (
        Option<ResMut<crate::telling::Tongue>>,
        Option<Res<crate::attention::Attention>>,
    ),
    mut hungry: Query<
        (
            Entity,
            &Transform,
            &Needs,
            &Person,
            &mut Activity,
            &mut MoveTarget,
            Option<&mut Chronicle>,
        ),
        (
            With<Villager>,
            Without<Prayer>,
            Without<Held>,
            Without<Airborne>,
            Without<Corpse>,
        ),
    >,
) {
    let god = name.as_ref().map_or("their god", |n| n.0.as_str());

    for (entity, transform, needs, person, mut activity, mut target, chronicle) in &mut hungry {
        // Prayer answers an empty larder — this person's OWN larder. A full
        // store in the next town over is no comfort here.
        let store_has_food = members
            .get(entity)
            .ok()
            .and_then(|member| stores.get(member.0).ok())
            .is_some_and(|store| store.food() >= 1.0);
        if needs.hunger < DESPERATE_HUNGER || store_has_food {
            continue;
        }
        if matches!(*activity, Activity::Eating(_) | Activity::Sleeping) {
            continue;
        }
        // A bush in reach with fruit on it is hope enough to keep walking.
        let food_in_reach = bushes.iter().any(|(bush, source)| {
            source.amount > 0.2 && bush.translation().distance(transform.translation) < 45.0
        });
        if food_in_reach {
            continue;
        }

        *activity = Activity::Praying;
        target.0 = None;
        commands.entity(entity).insert(Prayer {
            remaining: PRAYER_PATIENCE,
        });

        // The mote: a small golden light over their head, for the god to see.
        commands.spawn((
            PrayerMote,
            Mesh3d(meshes.add(Cuboid::new(0.22, 0.22, 0.22))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: crate::palette::shade(&crate::palette::CLOTH_GOLD, 0.95),
                emissive: LinearRgba::from(crate::palette::shade(
                    &crate::palette::CLOTH_GOLD,
                    0.95,
                )) * 6.0,
                ..default()
            })),
            Transform::from_xyz(0.0, 2.6, 0.0),
            bevy::light::NotShadowCaster,
            ChildOf(entity),
        ));

        info!("{} prays to {god} for food", person.name);
        // A watched prayer is composed — this person, this hunger, kneeling
        // now — and arrives over their head a breath later. Elsewhere, the
        // written words serve as they always have.
        let composed = telling
            .0
            .as_mut()
            .filter(|_| {
                crate::attention::regard(telling.1.as_deref(), transform.translation)
                    .worth_composing()
            })
            .map(|tongue| {
                tongue.muse(crate::telling::Musing {
                    who: entity,
                    voice: None,
                    bearing: crate::villager::traits::Bearing::Plain,
                    faith: crate::telling::FaithBand::Sure,
                    body: vec!["hungry"],
                    place: Vec::new(),
                    mind: "you kneel and beg the god for food".into(),
                    heard: None,
                    aloud: false,
                    known: Vec::new(),
                })
            })
            .is_some();
        // Unwatched or unanswered: the moment passes quietly. Nothing
        // written plays anywhere any more.
        let _ = composed;
        notices.write(crate::ui::Notice::new(format!(
            "{} prays to {god} for food",
            person.name
        )));
        if let Some(mut chronicle) = chronicle {
            chronicle.record(clock.day(), format!("prayed to {god} for food"));
        }
    }
}

/// The motes breathe, so a prayer reads as alive rather than as a marker.
pub(super) fn animate_motes(time: Res<Time>, mut motes: Query<&mut Transform, With<PrayerMote>>) {
    let t = time.elapsed_secs();
    for mut mote in &mut motes {
        mote.translation.y = 2.6 + (t * 2.1).sin() * 0.18;
        mote.rotation = Quat::from_rotation_y(t * 0.9);
    }
}

/// Removes a villager's prayer state and its mote.
fn end_prayer(
    commands: &mut Commands,
    entity: Entity,
    children: &Query<&Children>,
    motes: &Query<Entity, With<PrayerMote>>,
) {
    commands.entity(entity).remove::<Prayer>();
    if let Ok(kids) = children.get(entity) {
        for &child in kids {
            if motes.contains(child) {
                commands.entity(child).despawn();
            }
        }
    }
}

/// Food from the hand, set beside the praying: providence.
pub(super) fn answer_prayers(
    mut commands: Commands,
    clock: Res<crate::calendar::WorldClock>,
    name: Option<Res<DivineName>>,
    children: Query<&Children>,
    motes: Query<Entity, With<PrayerMote>>,
    offerings: Query<(&GlobalTransform, &FoodSource), (With<DivinelyPlaced>, Without<Held>)>,
    mut notices: MessageWriter<crate::ui::Notice>,
    mut witnessed: MessageWriter<DivineEvent>,
    mut praying: Query<
        (
            Entity,
            &Transform,
            &Person,
            &mut Activity,
            &mut Faith,
            Option<&mut Chronicle>,
        ),
        (With<Prayer>, Without<Corpse>),
    >,
) {
    let god = name.as_ref().map_or("their god", |n| n.0.as_str());

    for (entity, transform, person, mut activity, mut faith, chronicle) in &mut praying {
        let answered = offerings.iter().any(|(offering, source)| {
            source.amount > 0.2
                && offering.translation().distance(transform.translation) < ANSWER_RADIUS
        });
        if !answered {
            continue;
        }

        faith.shift(0.3);
        *activity = Activity::Idle;
        end_prayer(&mut commands, entity, &children, &motes);

        info!("{}'s prayer to {god} was answered", person.name);
        notices.write(crate::ui::Notice::fanfare(format!(
            "{}'s prayer was answered",
            person.name
        )));
        if let Some(mut chronicle) = chronicle {
            chronicle.record(clock.day(), "prayed for food, and food came");
        }

        // The answer is an act, and acts have witnesses.
        witnessed.write(DivineEvent {
            kind: DivineEventKind::Provided,
            position: transform.translation,
            subject: Some(entity),
            intensity: 0.8,
        });
    }
}

/// Hope has a horizon. A prayer left open long enough closes itself, and
/// takes something with it.
pub(super) fn despair(
    mut commands: Commands,
    time: Res<Time>,
    clock: Res<crate::calendar::WorldClock>,
    children: Query<&Children>,
    motes: Query<Entity, With<PrayerMote>>,
    mut notices: MessageWriter<crate::ui::Notice>,
    mut praying: Query<
        (
            Entity,
            &mut Prayer,
            &Person,
            &mut Activity,
            &mut Faith,
            Option<&mut Chronicle>,
        ),
        Without<Corpse>,
    >,
) {
    for (entity, mut prayer, person, mut activity, mut faith, chronicle) in &mut praying {
        prayer.remaining -= time.delta_secs();
        if prayer.remaining > 0.0 {
            continue;
        }

        faith.shift(-0.15);
        if *activity == Activity::Praying {
            *activity = Activity::Idle;
        }
        end_prayer(&mut commands, entity, &children, &motes);

        info!("{}'s prayer went unanswered", person.name);
        notices.write(crate::ui::Notice::new(format!(
            "{}'s prayer went unanswered",
            person.name
        )));
        if let Some(mut chronicle) = chronicle {
            chronicle.record(clock.day(), "prayed, and no answer came");
        }
    }
}

/// Seeing providence firsthand moves the watchers too.
pub(super) fn faith_of_witnesses(
    clock: Res<crate::calendar::WorldClock>,
    name: Option<Res<super::DivineName>>,
    mut notices: MessageWriter<crate::ui::Notice>,
    mut events: MessageReader<DivineEvent>,
    mut watchers: Query<
        (
            Entity,
            &Transform,
            &mut Faith,
            &super::Person,
            &crate::witness::Temperament,
            (Option<&mut Chronicle>, Option<&super::traits::Traits>),
        ),
        (With<Villager>, Without<Corpse>),
    >,
    witnesses: Query<&crate::witness::Witnessed>,
) {
    let god = name.as_ref().map_or("their god", |n| n.0.as_str());
    for event in events.read() {
        for (entity, transform, mut faith, person, temperament, (chronicle, manner)) in
            &mut watchers
        {
            let conviction = manner.map_or(1.0, |m| m.conviction());
            if Some(entity) == event.subject {
                continue;
            }
            if transform.translation.distance(event.position) > event.kind.carry() {
                continue;
            }
            // Faith moves only for witnesses who read the god into it at
            // all. The verdict lives on the memory the witness system just
            // recorded (this system is ordered after it); most people watch
            // lightning and see weather, and their faith holds still.
            let attributed = witnesses
                .get(entity)
                .ok()
                .and_then(|w| w.recent.first().map(|m| m.divine))
                .unwrap_or(true);
            if !attributed {
                continue;
            }
            let believed_before = faith.is_believer();
            match event.kind {
                DivineEventKind::Provided => {
                    faith.shift(0.1 * conviction);
                    if let Some(mut chronicle) = chronicle {
                        chronicle.record(clock.day(), "saw a prayer answered");
                    }
                }
                // The doctrine fork in miniature: the same lightning, read two
                // ways. The bold call it power and believe harder; the timid
                // call it terror and pull away. Both are right.
                DivineEventKind::Smote => {
                    // Wrath converts the bold outright — one seen bolt is a
                    // conversion, not a nudge — and drives the timid away
                    // just as hard. A smiting reshapes a congregation.
                    if temperament.boldness >= 0.45 {
                        faith.shift(0.16 * conviction);
                        if let Some(mut chronicle) = chronicle {
                            chronicle.record(clock.day(), "saw the god's power in the lightning");
                        }
                    } else {
                        faith.shift(-0.12 * conviction);
                        if let Some(mut chronicle) = chronicle {
                            chronicle.record(clock.day(), "saw the god's anger in the lightning");
                        }
                    }
                }
                // A smaller awe than lightning, read the same two ways.
                DivineEventKind::Uprooted => {
                    if temperament.boldness >= 0.45 {
                        faith.shift(0.04 * conviction);
                    } else {
                        faith.shift(-0.04 * conviction);
                    }
                }
                _ => {}
            }
            // Crossing the line is an event: the whole point of acting in
            // front of witnesses is that some of them become believers.
            if !believed_before && faith.is_believer() {
                notices.write(crate::ui::Notice::new(format!(
                    "{} has come to believe in {god}",
                    person.name
                )));
            }
        }
    }
}

/// The god's strength is the sum of what its people believe.
pub(super) fn tally_belief(
    time: Res<Time>,
    mut belief: ResMut<Belief>,
    faithful: Query<&Faith, (With<Villager>, Without<Corpse>)>,
) {
    let faith_sum: f32 = faithful.iter().map(|f| f.trust).sum();
    belief.total = faith_sum;
    belief.regenerate(faith_sum, time.delta_secs());
}

/// The Flourish miracle: every bush near the settlement fills with fruit.
///
/// The first purchase belief can make — famine, ended wholesale, at the price
/// of credibility earned one answered prayer at a time.
pub fn flourish(
    belief: &mut Belief,
    site: &SettlementSite,
    bushes: &mut Query<(&GlobalTransform, &mut FoodSource)>,
) -> bool {
    if belief.available() < FLOURISH_COST {
        return false;
    }
    belief.spent += FLOURISH_COST;
    for (at, mut source) in bushes.iter_mut() {
        if at.translation().distance(site.centre) < 160.0 {
            source.amount = source.amount.max(3.0);
        }
    }
    true
}

/// What kind of god the people have witnessed, accumulating toward a name.
///
/// Two legends grow side by side: providence, fed by answered prayers and
/// gifts; dread, fed by lightning and torn trees. When the god's strength
/// crosses a threshold, the *dominant* legend decides which new power
/// crystallises — you ascend as the god they already believe you to be.
#[derive(Resource)]
pub struct Legend {
    pub providence: f32,
    pub dread: f32,
    /// Belief, smoothed over ~half a minute, so one massacre or one festival
    /// does not whipsaw the tier.
    pub sustained: f32,
    pub tier: u8,
    pub unlocked: Option<crate::miracles::Miracle>,
    pub epithet: Option<&'static str>,
}

impl Default for Legend {
    fn default() -> Self {
        Legend {
            providence: 0.0,
            dread: 0.0,
            sustained: 0.0,
            tier: 1,
            unlocked: None,
            epithet: None,
        }
    }
}

/// Witnessed acts feed the legends.
pub(super) fn grow_legend(mut legend: ResMut<Legend>, mut events: MessageReader<DivineEvent>) {
    for event in events.read() {
        match event.kind {
            DivineEventKind::Provided | DivineEventKind::Mended => legend.providence += 1.0,
            DivineEventKind::Smote | DivineEventKind::Quaked => legend.dread += 1.0,
            DivineEventKind::Uprooted => legend.dread += 0.4,
            _ => {}
        }
    }
}

/// Sustained belief, and the moment of ascension.
pub(super) fn ascend(
    time: Res<Time>,
    belief: Res<Belief>,
    name: Option<Res<DivineName>>,
    settlements: Query<&super::Settlement>,
    faithful: Query<(), (With<Faith>, Without<Corpse>)>,
    mut legend: ResMut<Legend>,
    mut notices: MessageWriter<crate::ui::Notice>,
) {
    let dt = time.delta_secs();
    legend.sustained += (belief.total - legend.sustained) * (dt / 30.0).min(1.0);

    if legend.tier == 1 && legend.sustained >= 8.0 && faithful.iter().count() >= 10 {
        legend.tier = 2;
        let (miracle, epithet) = if legend.providence >= legend.dread {
            (crate::miracles::Miracle::Mend, "the Provider")
        } else {
            (crate::miracles::Miracle::Quake, "the Stormhand")
        };
        legend.unlocked = Some(miracle);
        legend.epithet = Some(epithet);

        let god = name.as_ref().map_or("the god", |n| n.0.as_str());
        let home = settlements
            .iter()
            .next()
            .map_or("the village", |s| s.name.as_str());
        info!("in {home} they now speak of {god} {epithet}");
        notices.write(crate::ui::Notice::fanfare(format!(
            "In {home} they now speak of {god} {epithet}"
        )));
    }
}

/// The god's biography in one line: the sum of living faith, sampled once
/// a day since the founding. The deity page draws it; saves carry it.
#[derive(Resource, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct FaithHistory {
    pub samples: Vec<f32>,
    pub last_day: u32,
}

/// Writes the day's sample when the day turns (and the founding's first).
pub(super) fn record_faith(
    clock: Res<crate::calendar::WorldClock>,
    belief: Res<Belief>,
    mut history: ResMut<FaithHistory>,
) {
    let day = clock.day();
    if history.samples.is_empty() || history.last_day != day {
        history.samples.push(belief.total);
        history.last_day = day;
    }
}

/// Members of the settlement carry Faith from the moment they exist.
pub(super) fn endow_faith(
    mut commands: Commands,
    newcomers: Query<Entity, (With<MemberOf>, Without<Faith>, Without<Corpse>)>,
) {
    for entity in &newcomers {
        commands.entity(entity).insert(Faith::default());
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn spent_belief_comes_back() {
        let mut belief = super::Belief {
            total: 6.0,
            spent: 6.0,
        };
        assert_eq!(belief.available(), 0.0);
        belief.regenerate(6.0, 120.0);
        assert!(belief.available() > 5.9, "two minutes should restore it");
        belief.regenerate(6.0, 120.0);
        assert_eq!(belief.spent, 0.0, "regeneration never goes negative");
    }

    use super::*;
    use crate::rng::Rng;

    /// A minimal world with a starving villager and an empty store.
    fn starving_world() -> (App, Entity) {
        let mut app = App::new();
        app.insert_resource(Time::<()>::default());
        app.init_resource::<crate::calendar::WorldClock>();
        app.init_resource::<Assets<Mesh>>();
        app.init_resource::<Assets<StandardMaterial>>();
        app.insert_resource(DivineName("Vessel".into()));
        app.add_message::<crate::ui::Notice>();
        app.add_message::<crate::ui::Say>();
        app.add_message::<DivineEvent>();
        app.add_systems(Update, (kneel, answer_prayers, despair).chain());

        let settlement = app
            .world_mut()
            .spawn(super::super::work::Stockpile::default())
            .id();
        app.insert_resource(SettlementSite {
            centre: Vec3::ZERO,
            radius: 24.0,
            woodpile: Vec3::ZERO,
            settlement,
        });

        let soul = app
            .world_mut()
            .spawn((
                Villager,
                Transform::default(),
                Person::born("Asketh".into(), "Prayerly".into()),
                Needs {
                    hunger: 0.9,
                    ..default()
                },
                Activity::Idle,
                MoveTarget::default(),
                Faith::default(),
                Chronicle::default(),
            ))
            .id();
        (app, soul)
    }

    #[test]
    fn the_desperate_pray_and_an_answer_deepens_faith() {
        let (mut app, soul) = starving_world();
        let _ = Rng::new(1);

        app.update();
        assert_eq!(
            *app.world().get::<Activity>(soul).unwrap(),
            Activity::Praying,
            "a starving soul with nothing to eat should kneel",
        );
        assert!(app.world().get::<Prayer>(soul).is_some());

        // The hand sets food down beside them.
        app.world_mut().spawn((
            FoodSource {
                amount: 3.0,
                regrowth: 0.0,
            },
            Transform::from_xyz(2.0, 0.0, 0.0),
            GlobalTransform::from(Transform::from_xyz(2.0, 0.0, 0.0)),
            crate::hand::DivinelyPlaced { remaining: 20.0 },
        ));
        app.update();

        let faith = app.world().get::<Faith>(soul).unwrap();
        assert!(faith.trust > 0.5, "an answered prayer must deepen faith");
        assert!(
            app.world().get::<Prayer>(soul).is_none(),
            "the prayer should close",
        );
        let chronicle = app.world().get::<Chronicle>(soul).unwrap();
        assert!(
            chronicle
                .events
                .iter()
                .any(|e| e.text.contains("food came")),
            "the answer must enter the chronicle: {:?}",
            chronicle.events,
        );
    }

    #[test]
    fn silence_curdles_into_doubt() {
        let (mut app, soul) = starving_world();
        app.update();
        assert!(app.world().get::<Prayer>(soul).is_some());

        for _ in 0..30 {
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_secs(4));
            app.update();
        }

        let faith = app.world().get::<Faith>(soul).unwrap();
        assert!(
            faith.trust < Faith::default().trust,
            "unanswered prayer must cost faith",
        );
        let chronicle = app.world().get::<Chronicle>(soul).unwrap();
        assert!(
            chronicle
                .events
                .iter()
                .any(|e| e.text.contains("no answer came")),
            "the silence must enter the chronicle: {:?}",
            chronicle.events,
        );
    }

    #[test]
    fn the_legend_names_the_god_it_saw() {
        // Providence outweighing dread earns the Provider and Mend; the
        // reverse earns the Stormhand and Quake. The player levels up the god
        // the people already believe in.
        let provider = Legend {
            providence: 6.0,
            dread: 2.0,
            ..default()
        };
        assert!(provider.providence >= provider.dread);

        let stormhand = Legend {
            providence: 1.0,
            dread: 5.0,
            ..default()
        };
        assert!(stormhand.dread > stormhand.providence);

        // The tier threshold demands both fervour and a congregation.
        let legend = Legend::default();
        assert_eq!(legend.tier, 1);
        assert!(legend.unlocked.is_none());
    }

    #[test]
    fn faith_is_clamped_and_worded() {
        let mut faith = Faith::default();
        faith.shift(10.0);
        assert_eq!(faith.trust, 1.0);
        assert_eq!(faith.describe(), "sure of you");
        faith.shift(-10.0);
        assert_eq!(faith.trust, 0.0);
        assert_eq!(faith.describe(), "doubts you");
    }

    #[test]
    fn an_answer_outweighs_a_silence() {
        // The loop must reward playing: one answered prayer more than undoes
        // one ignored one, or the optimal god is an absent one.
        let mut answered = Faith::default();
        answered.shift(0.3);
        let mut ignored = Faith::default();
        ignored.shift(-0.15);
        assert!(answered.trust - Faith::default().trust > Faith::default().trust - ignored.trust);
    }

    #[test]
    fn belief_spends_down_but_never_negative() {
        let mut belief = Belief {
            total: 12.0,
            spent: 0.0,
        };
        assert!(belief.available() >= FLOURISH_COST);
        belief.spent += FLOURISH_COST;
        assert!((belief.available() - 2.0).abs() < 1e-5);
        belief.spent += 100.0;
        assert_eq!(belief.available(), 0.0);
    }
}
