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
pub(super) const DESPERATE_HUNGER: f32 = 0.65;

/// How many food prayers may stand open from one town before the rest of
/// the hungry hold their asking. Enough for a scene, short of a flashmob.
const CHORUS_OF_ASKING: usize = 3;

/// How long a prayer stays open before it curdles into doubt.
///
/// Long enough to read the pink notice, open nothing, and fly there — the
/// prayer board makes prayers answerable, and a horizon nobody can reach is
/// a doubt machine. Not longer, because a food prayer is made by someone
/// starving: hope that outlives the hopeful helps no one.
const PRAYER_PATIENCE: f32 = 120.0;

/// How near the answer must land to be *their* answer.
pub(super) const ANSWER_RADIUS: f32 = 9.0;

/// How long a devotion hangs before it fades. Longer than a food prayer's
/// patience — nobody is dying — and its lapse costs nothing, so the length
/// is generosity, not pressure.
const DEVOTION_PATIENCE: f32 = 150.0;

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

    pub(super) fn shift(&mut self, amount: f32) {
        self.trust = (self.trust + amount).clamp(0.0, 1.0);
    }
}

/// What a prayer asks for.
#[derive(Debug, Clone)]
pub enum PrayerKind {
    /// The crisis kind: starving, with an empty larder.
    Food,
    /// The dark kind: against a neighbor. Hatred deep enough, in a heart
    /// faithful enough to believe asking might work.
    Dark {
        against: Entity,
        name: String,
        /// What the grudge is over, as the bond remembers it — the board
        /// reads it out, so the god knows what is being avenged.
        over: Option<String>,
    },
    /// The founding kind: a would-be colony leader asks the god's blessing
    /// for the road before a party walks out to raise a new town. Answer it
    /// with a gift and they go with heart; smite the ground before them and
    /// the venture is forbidden; say nothing and they go anyway, unheard.
    Road {
        /// Where the new banner would go up.
        destination: Vec3,
        /// The name the new town already carries — a place people mean to
        /// walk to needs a name to speak of on the road.
        colony: String,
        /// The town they would leave.
        mother: Entity,
        /// Rolled when the asking is made, so the blessing blesses a
        /// particular flag and not an idea.
        banner_ramp: usize,
        sigil: usize,
    },
    /// The rhetorical kind: no crisis, no request the larder could meet —
    /// a hope said out loud. "If you are there, keep the town safe
    /// tonight." Answered by the god SHOWING THEMSELF: any unmistakably
    /// divine act the asker witnesses while it is open. Unanswered, it
    /// fades — it never curdles. Brett: "It would make the system feel
    /// like its working in the begining."
    Devotion {
        /// Believers give thanks; doubters ask if anyone is there. The
        /// board reads them differently.
        grateful: bool,
    },
}

impl PrayerKind {
    /// The asking, in board words: what this prayer wants from the god,
    /// named after whoever is asking. One writer, so the codex board and
    /// the pinned tracker can never drift apart.
    pub fn ask_line(&self, asker: &str) -> String {
        match self {
            PrayerKind::Food => format!("{asker} asks for food"),
            PrayerKind::Dark { name, over, .. } => match over {
                Some(over) => {
                    format!("{asker} asks that {name} be struck down - over {over}")
                }
                None => format!("{asker} asks that {name} be struck down"),
            },
            PrayerKind::Road { colony, .. } => {
                format!("{asker} asks your blessing for the road - to found {colony}")
            }
            PrayerKind::Devotion { grateful } => {
                if *grateful {
                    format!("{asker} gives thanks")
                } else {
                    format!("{asker} asks if you are there")
                }
            }
        }
    }

    /// The same asking with a crowd behind it: the board clumps prayers
    /// for the same thing onto one card, fronted by the most urgent
    /// asker. Brett: "clump multiple people praying for the same thing
    /// into one prayer card to prevent getting swamped with cards."
    /// Dark and road prayers never clump - each names a particular
    /// neighbor or a particular flag - so those arms fall back to the
    /// single voice.
    pub fn ask_line_many(&self, asker: &str, others: usize) -> String {
        let company = if others == 1 {
            "one other".to_string()
        } else {
            format!("{others} others")
        };
        match self {
            PrayerKind::Food => format!("{asker} and {company} ask for food"),
            PrayerKind::Devotion { grateful } => {
                if *grateful {
                    format!("{asker} and {company} give thanks")
                } else {
                    format!("{asker} and {company} ask if you are there")
                }
            }
            _ => self.ask_line(asker),
        }
    }
}

/// An open prayer: what they are asking for, and how long hope lasts.
#[derive(Component, Debug)]
pub struct Prayer {
    pub remaining: f32,
    /// Their own words, picked from the corpus the moment they knelt —
    /// carried HERE rather than through the watched-head musing channel,
    /// because a prayer is addressed to the player: the codex board reads
    /// it sight unseen, and the bubble replays it for every fresh look.
    pub words: Option<String>,
    /// Whether the pink bubble is up for the current look. Cleared when
    /// regard moves off them, so coming back to a praying villager shows
    /// the words again instead of exactly once per prayer.
    pub bubbled: bool,
    pub kind: PrayerKind,
}

/// What became of a prayer, for the board's "lately" strip.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum PrayerOutcome {
    Answered,
    Curdled,
    Died,
    /// The world fed them before the god got around to it — a meal from the
    /// larder, a fisher home heavy. No credit claimed, no doubt seeded.
    Fed,
    /// A devotion nobody answered. Hope unanswered fades; it does not
    /// sour — no notice, no doubt, a receipt and nothing else.
    Faded,
}

impl PrayerOutcome {
    pub fn describe(&self) -> &'static str {
        match self {
            PrayerOutcome::Answered => "answered",
            PrayerOutcome::Curdled => "went unanswered",
            PrayerOutcome::Died => "died waiting",
            PrayerOutcome::Fed => "ended with a meal",
            PrayerOutcome::Faded => "faded on the wind",
        }
    }
}

/// A closed prayer, kept so the board can show what the god did and did
/// not do — and kept across saves: the receipts are the point.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ClosedPrayer {
    pub name: String,
    pub words: Option<String>,
    pub outcome: PrayerOutcome,
}

/// The recent history of prayers, newest last. The open ones live on the
/// praying themselves as [`Prayer`] components; this holds only the closed.
#[derive(Resource, Default)]
pub struct PrayerLedger {
    pub closed: Vec<ClosedPrayer>,
}

impl PrayerLedger {
    /// How many closed prayers the board remembers.
    const KEPT: usize = 8;

    pub fn close(&mut self, name: &str, words: Option<String>, outcome: PrayerOutcome) {
        self.closed.push(ClosedPrayer {
            name: name.to_string(),
            words,
            outcome,
        });
        if self.closed.len() > Self::KEPT {
            self.closed.remove(0);
        }
    }
}

/// The visible mote of a prayer, hanging over the praying — the player's cue
/// that someone, somewhere, is asking.
#[derive(Component)]
pub struct PrayerMote;

/// Hangs the golden mote over someone praying. Every asking looks the same
/// from the sky — food, hatred or the road — and what kind it is waits on
/// the board. One writer for the light, so no prayer's cue can drift.
pub(super) fn raise_prayer_mote(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    over: Entity,
) {
    commands.spawn((
        PrayerMote,
        Mesh3d(meshes.add(Cuboid::new(0.22, 0.22, 0.22))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: crate::palette::shade(&crate::palette::CLOTH_GOLD, 0.95),
            emissive: LinearRgba::from(crate::palette::shade(&crate::palette::CLOTH_GOLD, 0.95))
                * 6.0,
            ..default()
        })),
        Transform::from_xyz(0.0, 2.6, 0.0),
        bevy::light::NotShadowCaster,
        ChildOf(over),
    ));
}

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
/// Desperation means real absence: hungry, with an empty larder — the
/// village eats from its stores and nowhere else, so an empty larder IS
/// hunger with nowhere to turn. A player who empties a village's sacks
/// *causes* prayer — famine is a lever, and so is generosity. And whoever
/// is starving outright prays even if the larder reads full: stores go
/// unreachable, and a full sack nobody can walk to feeds nobody.
/// Prayer is something you can see now: whoever is praying goes down on
/// their knees, and stands back up when the prayer leaves them. One writer
/// for the posture, so there are no cleanup edges to miss.
pub(crate) fn take_a_knee(
    mut folk: Query<
        (
            &Activity,
            &mut crate::creature::anim::CreatureMotion,
            Has<crate::creature::Held>,
            Has<crate::creature::Airborne>,
        ),
        (With<Villager>, Without<crate::creature::Corpse>),
    >,
) {
    // The dial forces the whole village to its knees, so the pose can be
    // photographed without waiting for someone to feel like praying.
    let staged = std::env::var("DIVUS_FACTUS_KNEEL_TEST").is_ok();
    for (activity, mut motion, held, airborne) in &mut folk {
        // The god's grip outranks the prayer: a body plucked mid-devotion
        // unfolds to dangle and kick, and kneels again when set down.
        // One writer for the pose, keyed off what the person is DOING:
        // asking on their knees, or on their knees at the sight of the
        // god's own hand. A second system writing `kneeling` of its own
        // accord is how the last two shakes in this game happened.
        let kneeling = !held
            && !airborne
            && (staged || matches!(activity, Activity::Praying | Activity::Marvelling));
        if motion.kneeling != kneeling {
            motion.kneeling = kneeling;
        }
    }
}

pub(super) fn kneel(
    mut commands: Commands,
    clock: Res<crate::calendar::WorldClock>,
    name: Option<Res<DivineName>>,
    members: Query<&MemberOf>,
    stores: Query<&super::work::Stockpile>,
    grounds: Query<&super::SettlementGround>,
    gifts: Query<&Transform, (With<DivinelyPlaced>, With<FoodSource>, Without<Held>)>,
    // The dead do not hold places in the chorus: a corpse still wearing
    // its unanswered prayer must not mute the living behind it.
    asking: Query<(&Prayer, &MemberOf), Without<Corpse>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut notices: MessageWriter<crate::ui::Notice>,
    mut tongue: Option<ResMut<crate::sermo::Tongue>>,
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
            Without<crate::avatar::Ridden>,
            Without<Airborne>,
            Without<Corpse>,
        ),
    >,
) {
    let god = name.as_ref().map_or("their god", |n| n.0.as_str());

    for (entity, at, needs, person, mut activity, mut target, chronicle) in &mut hungry {
        // Prayer answers an empty larder — this person's OWN larder. A full
        // store in the next town over is no comfort here.
        // A stocked larder eighty strides behind you feeds you nothing
        // NOW. Whoever hungers ON THE ROAD — explorer, colonist, anyone
        // past the town's working ground — prays at desperate rather than
        // at death's door, because out there the god is the only larder
        // in reach. Kileb starved on the road while this gate read his
        // town's full sacks as comfort.
        let home = members.get(entity).ok().map(|member| member.0);
        let on_the_road = home
            .and_then(|town| grounds.get(town).ok())
            .is_some_and(|ground| at.translation.distance(ground.center) > 80.0);
        // A FULL TOWN SILENCES THE PRAYER WHEREVER THE HUNGRY ARE STANDING.
        //
        // This used to be `!on_the_road && ...`, so anyone eighty strides
        // out prayed for food over a granary holding fifty-two of it -
        // Brett: "peopel are praying for food even though the grainery has
        // 52 food." That was honest when it was written, because distance
        // meant the larder might as well not exist: nobody with an errand
        // in hand ever turned back for it, whatever their hunger.
        //
        // They do now (see `eat_from_store`: any errand is dropped past
        // desperate), so the walk home IS the answer to being hungry on the
        // road, and a prayer over a full larder is a false alarm - the
        // loudest kind, since the god is being asked for something the
        // village already has.
        //
        // What still gets through is the one that matters: `dying_regardless`
        // below, for whoever cannot reach it in time.
        let store_has_food = home
            .and_then(|town| stores.get(town).ok())
            .is_some_and(|store| store.food() >= 1.0);
        // A stocked larder is only comfort if it can actually feed them. At
        // the edge of death the prayer goes up REGARDLESS: stores go
        // unreachable — a square terraced for the hall, a route refused —
        // and "there is food somewhere in town" must never again silence
        // someone starving beside it. Six villagers died in VisitingStore at
        // hunger 1.00 with the larder reading full, and not one prayer rose.
        let dying_regardless = needs.hunger >= 0.92;
        // Hunger synchronizes across a town - same mealtimes, same thin
        // night - and one shared threshold turned that into a whole
        // village kneeling at a single dawn. Brett, watching it: "Everyone
        // is praying like crazy people!" Each soul carries its own point
        // of desperation instead, spread a few meals wide, so the askings
        // arrive as a trickle a town can watch and answer. Not on the
        // road: out there the prayer is the rescue flare, and it goes up
        // at desperate, unstaggered.
        let pride = if on_the_road {
            0.0
        } else {
            (entity.to_bits() % 8) as f32 * 0.025
        };
        if needs.hunger < DESPERATE_HUNGER + pride || (store_has_food && !dying_regardless) {
            continue;
        }
        // And one voice can ask for a whole town: while a few food
        // prayers already stand from this town, the next hungry soul
        // holds their asking - no less hungry, just not everyone on
        // their knees at once. The dying wait for nobody, and neither
        // does anyone on the road - a walker past the working ground is
        // not part of the town's chorus, and their asking is the only
        // sign of them the god gets.
        if !on_the_road && !dying_regardless {
            let chorus = asking
                .iter()
                .filter(|(prayer, member)| {
                    matches!(prayer.kind, PrayerKind::Food) && Some(member.0) == home
                })
                .count();
            if chorus >= CHORUS_OF_ASKING {
                continue;
            }
        }
        if matches!(*activity, Activity::Eating(_) | Activity::Sleeping) {
            continue;
        }
        // A gift already lying within reach IS the answer: the god has
        // provided, and the next move is dinner, not another asking.
        // Without this, an offering set beside a praying villager was
        // answered, re-asked and re-answered every frame - Brett: "she
        // prayed for food and I answered and it spammed notifications."
        let provided = gifts
            .iter()
            .any(|gift| gift.translation.distance(at.translation) < 45.0);
        if provided {
            continue;
        }

        *activity = Activity::Praying;
        target.0 = None;
        // The words are picked NOW, watched or not. The old attention gate
        // belonged to the retired teller, which paid real compute per line
        // and so composed only for watched heads — with the corpus the pick
        // is free, and it left the prayer wordless unless the player was
        // already staring at the right villager on the right frame. Brett
        // saw the notices and never once the pink bubble.
        let words = tongue.as_mut().and_then(|tongue| {
            tongue.pray(entity, &["hungry"], crate::sermo::FaithBand::Sure, None)
        });
        commands.entity(entity).insert(Prayer {
            remaining: PRAYER_PATIENCE,
            words,
            bubbled: false,
            kind: PrayerKind::Food,
        });

        // The mote: a small golden light over their head, for the god to see.
        raise_prayer_mote(&mut commands, &mut meshes, &mut materials, entity);

        info!("{} prays to {god} for food", person.name);
        notices.write(crate::ui::Notice::prayer(format!(
            "{} prays to {god} for food",
            person.name
        )));
        if let Some(mut chronicle) = chronicle {
            chronicle.record(clock.day(), format!("prayed to {god} for food"));
        }
    }
}

/// The dark kneel: hatred deep enough, in a heart faithful enough.
///
/// Someone who has come to HATE a neighbor — the regard graph's floor,
/// fed by soured talks and ugly gossip — and who believes in the god
/// enough to think asking might work, kneels and prays AGAINST them.
/// The ask lands on the board like any prayer, named: granting it is
/// the god's own choice, made with lightning, and the kind of god you
/// are is the sum of which prayers you answer.
pub(super) fn kneel_in_hatred(
    mut commands: Commands,
    time: Res<Time>,
    clock: Res<crate::calendar::WorldClock>,
    name: Option<Res<DivineName>>,
    mut rng: ResMut<super::SimRng>,
    mut tongue: Option<ResMut<crate::sermo::Tongue>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut notices: MessageWriter<crate::ui::Notice>,
    names: Query<&Person>,
    mut hateful: Query<
        (
            Entity,
            &Person,
            &super::regard::Regard,
            &Faith,
            &mut Activity,
            &mut MoveTarget,
            Option<&mut super::Stirrings>,
        ),
        (
            With<Villager>,
            Without<Prayer>,
            Without<Held>,
            Without<crate::avatar::Ridden>,
            Without<Airborne>,
            Without<Corpse>,
        ),
    >,
) {
    let god = name.as_ref().map_or("their god", |n| n.0.as_str());
    let dt = time.delta_secs();

    for (entity, person, regard, faith, mut activity, mut target, stirred) in &mut hateful {
        // Rare by construction: hatred at the floor of the heart, faith
        // worth praying with, and even then only now and again - a dark
        // prayer is a breaking point, not a habit. The doubt that follows
        // an unanswered one (-0.15) is what keeps it from looping: pray
        // into silence often enough and you stop believing asking works.
        if !rng.0.chance(dt * 0.01) {
            continue;
        }
        if faith.trust < 0.45 {
            continue;
        }
        if matches!(*activity, Activity::Eating(_) | Activity::Sleeping) {
            continue;
        }
        let Some(hated) = regard.sourest().filter(|bond| bond.warmth <= -0.6) else {
            continue;
        };
        let Ok(enemy) = names.get(hated.toward) else {
            continue;
        };
        let against = hated.toward;
        let enemy_name = enemy.name.clone();
        let over = hated.over.clone();

        *activity = Activity::Praying;
        target.0 = None;
        let words = tongue.as_mut().and_then(|tongue| {
            tongue.pray(
                entity,
                &["grudge"],
                crate::sermo::FaithBand::of(faith.trust),
                Some(&enemy_name),
            )
        });
        commands.entity(entity).insert(Prayer {
            remaining: PRAYER_PATIENCE,
            words,
            bubbled: false,
            kind: PrayerKind::Dark {
                against,
                name: enemy_name.clone(),
                over,
            },
        });

        // The mote over a dark prayer is the same gold: the god sees every
        // asking the same way, and what kind it is waits on the board.
        raise_prayer_mote(&mut commands, &mut meshes, &mut materials, entity);

        info!("{} prays to {god} against {enemy_name}", person.name);
        notices.write(crate::ui::Notice::prayer(format!(
            "{} prays to {god} against {enemy_name}",
            person.name
        )));
        if let Some(mut stirred) = stirred {
            stirred.stir(
                clock.day(),
                format!("prayed against {enemy_name} - hatred spoke"),
            );
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

/// Shows a praying villager's words in the pink bubble, for as long as the
/// prayer is open — not once at the kneel, which is what left Brett flying
/// to every prayer notice and arriving to silence. Regard leaving them
/// re-arms the bubble, so every fresh look reads the prayer again.
pub(super) fn show_prayer_bubbles(
    attention: Option<Res<crate::attention::Attention>>,
    name: Option<Res<DivineName>>,
    mut say: MessageWriter<crate::ui::Say>,
    mut praying: Query<
        (Entity, &Transform, &mut Prayer),
        (With<Villager>, Without<Corpse>, Without<Held>),
    >,
) {
    let god = name.as_ref().map_or("the god", |n| n.0.as_str());
    for (entity, at, mut prayer) in &mut praying {
        let watched = crate::attention::regard(attention.as_deref(), at.translation).worth_saying();
        if !watched {
            if prayer.bubbled {
                prayer.bubbled = false;
            }
            continue;
        }
        if prayer.bubbled {
            continue;
        }
        let Some(words) = prayer.words.clone() else {
            continue;
        };
        prayer.bubbled = true;
        say.write(crate::ui::Say {
            speaker: entity,
            text: words.replace("the god", god),
            thought: true,
            prayer: true,
        });
    }
}

/// One floating mark of faith moving: a pink "+" rising off the newly
/// convinced, an ash "-" off the doubting. The pin and the climb are
/// Ordo's; the age, the fade and the colors are this game's.
#[derive(Component)]
pub(super) struct FaithMark {
    left: f32,
}

/// How long a faith mark lives, rising and then thinning to nothing.
const MARK_LIFE: f32 = 1.4;

/// Puts a mark over anyone whose faith just moved. Brett: "when a villager
/// gains belief can we have some pink + float up from them and when they
/// disbelieve can we get some other colored -?" Watching `Changed<Faith>`
/// against a remembered value catches every author of a shift — prayers
/// answered, witnesses awed, sleepers hammered awake at midnight — without
/// a single one of them knowing about it.
pub(super) fn mark_the_faith_moved(
    mut commands: Commands,
    fonts: Option<Res<crate::ui::Fonts>>,
    mut remembered: Local<std::collections::HashMap<Entity, f32>>,
    moved: Query<(Entity, &Faith), (With<Villager>, Changed<Faith>, Without<Corpse>)>,
) {
    let Some(fonts) = fonts else {
        return;
    };
    for (entity, faith) in &moved {
        let before = remembered.insert(entity, faith.trust);
        // First sighting is the endowment at birth, not a change of heart.
        let Some(before) = before else {
            continue;
        };
        let delta = faith.trust - before;
        if delta.abs() < 0.005 {
            continue;
        }
        let (glyph, ink) = if delta > 0.0 {
            // The prayer channel's pink: belief moving toward the god.
            ("+", crate::palette::shade(&crate::palette::CLOTH_PINK, 1.0))
        } else {
            // Doubt goes out to ash, the same ash a doubter's nameplate wears.
            ("-", crate::palette::shade(&crate::palette::STONE, 0.78))
        };
        let pin = ordo::pin(&mut commands, entity, 2.3, Some(170.0), 55.0);
        commands
            .entity(pin)
            .insert((ordo::Rising(0.85), FaithMark { left: MARK_LIFE }));
        // Big and shadowed: at 19px the mark drowned in the world behind
        // it. Brett: "these need to be bolder and bigger so they are
        // easier to see, same with the -."
        commands.spawn((
            Text::new(glyph),
            TextFont {
                font: fonts.display_bold.clone().into(),
                font_size: bevy::text::FontSize::Px(32.0),
                ..default()
            },
            TextColor(ink),
            bevy::ui::widget::TextShadow {
                offset: Vec2::splat(2.0),
                color: Color::BLACK.with_alpha(0.6),
            },
            ChildOf(pin),
        ));
    }
}

/// Ages the marks: they thin over their last half and go out. The shadow
/// thins with the ink, or the glyph would leave its own dark ghost.
pub(super) fn fade_faith_marks(
    mut commands: Commands,
    time: Res<Time>,
    mut marks: Query<(Entity, &mut FaithMark, &Children)>,
    mut inks: Query<(&mut TextColor, Option<&mut bevy::ui::widget::TextShadow>)>,
) {
    for (entity, mut mark, children) in &mut marks {
        mark.left -= time.delta_secs();
        if mark.left <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        let thin = (mark.left / (MARK_LIFE * 0.5)).clamp(0.0, 1.0);
        for child in children {
            if let Ok((mut ink, shadow)) = inks.get_mut(*child) {
                let faded = ink.0.with_alpha(thin);
                if ink.0 != faded {
                    ink.0 = faded;
                }
                if let Some(mut shadow) = shadow {
                    shadow.color = Color::BLACK.with_alpha(0.6 * thin);
                }
            }
        }
    }
}

/// A prayer does not survive the praying. Whoever dies mid-devotion has
/// their prayer closed on the board as the dark receipt it is — and their
/// mote taken down, which nothing else would ever do for a corpse.
pub(super) fn close_the_prayers_of_the_dead(
    mut commands: Commands,
    children: Query<&Children>,
    motes: Query<Entity, With<PrayerMote>>,
    mut ledger: ResMut<PrayerLedger>,
    dead: Query<(Entity, &Person, &Prayer), With<Corpse>>,
) {
    for (entity, person, prayer) in &dead {
        ledger.close(&person.name, prayer.words.clone(), PrayerOutcome::Died);
        end_prayer(&mut commands, entity, &children, &motes);
    }
}

/// The small prayers: devotions, said into the air on ordinary days.
///
/// No crisis calls them. The first uncertain days after a founding, a storm
/// leaning on the roofs, a house finished, a quiet dusk — someone kneels
/// and says a hope out loud. Doubters ask if anyone is there; believers
/// give thanks. This is the board's heartbeat while the village thrives:
/// Brett — "It feels weird that in the begining you never see any prayers
/// coming in."
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn kneel_in_devotion(
    mut commands: Commands,
    time: Res<Time>,
    mut since_last: Local<f32>,
    clock: Res<crate::calendar::WorldClock>,
    weather: Option<Res<crate::weather::Weather>>,
    name: Option<Res<DivineName>>,
    mut rng: ResMut<super::SimRng>,
    mut tongue: Option<ResMut<crate::sermo::Tongue>>,
    mut visuals: (ResMut<Assets<Mesh>>, ResMut<Assets<StandardMaterial>>),
    mut notices: MessageWriter<crate::ui::Notice>,
    towns: Query<(Entity, &super::Settlement)>,
    new_roofs: Query<&MemberOf, Added<super::work::Building>>,
    open: Query<&Prayer>,
    mut folk: Query<
        (
            Entity,
            &MemberOf,
            &Person,
            &Faith,
            &mut Activity,
            &mut MoveTarget,
            Option<&mut Chronicle>,
        ),
        (
            With<Villager>,
            Without<Prayer>,
            Without<Held>,
            Without<crate::avatar::Ridden>,
            Without<Airborne>,
            Without<Corpse>,
            Without<crate::creature::Childhood>,
        ),
    >,
) {
    // Roofs finished this frame are read every pass; the clock gate below
    // only meters how often the QUIET occasions are rolled.
    let roofed_towns: Vec<Entity> = new_roofs.iter().map(|member| member.0).collect();

    *since_last += time.delta_secs();
    if *since_last < 10.0 && roofed_towns.is_empty() {
        return;
    }
    *since_last = 0.0;

    // One devotion open at a time in the whole world: they are the board's
    // heartbeat, not its flood.
    if open
        .iter()
        .any(|prayer| matches!(prayer.kind, PrayerKind::Devotion { .. }))
    {
        return;
    }

    let god = name.as_ref().map_or("their god", |n| n.0.as_str());
    let stormy = weather
        .as_ref()
        .is_some_and(|w| w.kind() == crate::weather::WeatherKind::Storm);
    let night = !super::work::is_work_hour(clock.time_of_day());

    for (town, settlement) in &towns {
        let roof = roofed_towns.contains(&town);
        // The first days after a founding are the asking days: the flag is
        // planted, the god is a hope, and "if you are there" is the whole
        // register of the town.
        let young = clock.day().saturating_sub(settlement.founded) < 12;
        let chance = if roof {
            0.65
        } else if stormy {
            0.3
        } else if young {
            0.22
        } else if night {
            0.06
        } else {
            0.02
        };
        if !rng.0.chance(chance) {
            continue;
        }

        let candidates: Vec<Entity> = folk
            .iter()
            .filter(|(_, member, ..)| member.0 == town)
            .map(|(who, ..)| who)
            .collect();
        if candidates.is_empty() {
            continue;
        }
        let who = candidates[rng.0.next_u32() as usize % candidates.len()];
        let Ok((_, _, person, faith, mut activity, mut target, chronicle)) = folk.get_mut(who)
        else {
            continue;
        };
        if matches!(*activity, Activity::Eating(_) | Activity::Sleeping) {
            continue;
        }

        let grateful = roof || faith.is_believer();
        let mut body = vec!["devotion"];
        if roof {
            body.push("roof");
        } else if stormy {
            body.push("storm");
        } else if night {
            body.push("night");
        }
        let band = if faith.trust > 0.6 {
            crate::sermo::FaithBand::Sure
        } else if faith.trust > 0.3 {
            crate::sermo::FaithBand::Wavering
        } else {
            crate::sermo::FaithBand::Doubting
        };
        let words = tongue
            .as_mut()
            .and_then(|tongue| tongue.pray(who, &body, band, None));

        *activity = Activity::Praying;
        target.0 = None;
        commands.entity(who).insert(Prayer {
            remaining: DEVOTION_PATIENCE,
            words,
            bubbled: false,
            kind: PrayerKind::Devotion { grateful },
        });
        raise_prayer_mote(&mut commands, &mut visuals.0, &mut visuals.1, who);

        info!("{} offers a small prayer to {god}", person.name);
        notices.write(crate::ui::Notice::prayer(if grateful {
            format!("{} gives thanks to {god}", person.name)
        } else {
            format!("{} wonders if {god} is there", person.name)
        }));
        if let Some(mut chronicle) = chronicle {
            chronicle.record(clock.day(), "offered a small prayer");
        }
        // One asking per pass, everywhere.
        return;
    }
}

/// A devotion is answered by the god SHOWING THEMSELF: any unmistakably
/// divine act the asker witnesses while their small prayer hangs — a stone
/// lifted, a gift set down, a roof mended by no hand they can see. The
/// asking was "are you there"; the answer is anything only a god does.
pub(super) fn answer_devotions(
    mut commands: Commands,
    clock: Res<crate::calendar::WorldClock>,
    mut ledger: ResMut<PrayerLedger>,
    mut notices: MessageWriter<crate::ui::Notice>,
    children: Query<&Children>,
    motes: Query<Entity, With<PrayerMote>>,
    mut seen: MessageReader<DivineEvent>,
    gifts: Query<&Transform, (With<DivinelyPlaced>, Without<Held>)>,
    mut praying: Query<(
        Entity,
        &Transform,
        &Person,
        &Prayer,
        &mut Faith,
        &mut Activity,
        Option<&mut Chronicle>,
        Option<&mut super::Stirrings>,
    )>,
) {
    let shows: Vec<Vec3> = seen
        .read()
        .filter(|event| event.kind.unmistakably_divine() >= 0.5)
        .map(|event| event.position)
        .collect();

    for (entity, at, person, prayer, mut faith, mut activity, chronicle, stirred) in &mut praying {
        let PrayerKind::Devotion { grateful } = prayer.kind else {
            continue;
        };
        let shown = shows
            .iter()
            .any(|show| show.distance(at.translation) < 60.0)
            || gifts
                .iter()
                .any(|gift| gift.translation.distance(at.translation) < 20.0);
        if !shown {
            continue;
        }

        faith.shift(0.15);
        ledger.close(&person.name, prayer.words.clone(), PrayerOutcome::Answered);
        if let Some(mut chronicle) = chronicle {
            chronicle.record(
                clock.day(),
                if grateful {
                    "gave thanks, and was heard"
                } else {
                    "asked if you were there, and saw the answer"
                },
            );
        }
        if let Some(mut stirred) = stirred {
            stirred.stir(clock.day(), "saw the answer with their own eyes");
        }
        notices.write(crate::ui::Notice::fanfare(format!(
            "{}'s small prayer was answered",
            person.name
        )));
        if *activity == Activity::Praying {
            *activity = Activity::Idle;
        }
        end_prayer(&mut commands, entity, &children, &motes);
    }
}

/// A meal ends a food prayer, however the meal arrived — the larder came
/// back, a fisher walked in heavy, the god's own loaf landed. The hunger is
/// gone and the prayer goes with it. Without this a fed villager's stale
/// prayer ran its whole patience out and CURDLED, and the village lost
/// faith over dinners that were eaten on time.
pub(super) fn meals_answer_prayers(
    mut commands: Commands,
    children: Query<&Children>,
    motes: Query<Entity, With<PrayerMote>>,
    mut ledger: ResMut<PrayerLedger>,
    fed: Query<(Entity, &Person, &Needs, &Prayer)>,
) {
    for (entity, person, needs, prayer) in &fed {
        if !matches!(prayer.kind, PrayerKind::Food) || needs.hunger > 0.35 {
            continue;
        }
        ledger.close(&person.name, prayer.words.clone(), PrayerOutcome::Fed);
        end_prayer(&mut commands, entity, &children, &motes);
    }
}

/// Removes a villager's prayer state and its mote.
pub(super) fn end_prayer(
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
    offerings: Query<(&Transform, &FoodSource), (With<DivinelyPlaced>, Without<Held>)>,
    mut notices: MessageWriter<crate::ui::Notice>,
    mut witnessed: MessageWriter<DivineEvent>,
    mut ledger: ResMut<PrayerLedger>,
    mut praying: Query<
        (
            Entity,
            &Transform,
            &Person,
            &Prayer,
            &mut Activity,
            &mut Faith,
            Option<&mut Chronicle>,
            Option<&mut super::Stirrings>,
        ),
        Without<Corpse>,
    >,
) {
    let god = name.as_ref().map_or("their god", |n| n.0.as_str());

    for (entity, transform, person, prayer, mut activity, mut faith, chronicle, stirred) in
        &mut praying
    {
        let answered = offerings.iter().any(|(offering, source)| {
            source.amount > 0.2
                && offering.translation.distance(transform.translation) < ANSWER_RADIUS
        });
        if !answered {
            continue;
        }

        faith.shift(0.3);
        *activity = Activity::Idle;
        if let Some(mut stirred) = stirred {
            stirred.stir(clock.day(), "prayed, and the god answered - faith surged");
        }
        ledger.close(&person.name, prayer.words.clone(), PrayerOutcome::Answered);
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

/// The dark prayers, watching the sky for their answer.
///
/// A smite that lands on the hated WHILE the prayer is open is the god
/// siding with the asker, and everyone learns what kind of god that is:
/// the asker's faith surges past what providence ever pays, and the
/// legend's dread swells — answering hate on request is a darker act
/// than any unprompted bolt, because it makes the god an instrument
/// anyone hateful enough might reach for. Which is the point, and the
/// path.
pub(super) fn answer_dark_prayers(
    mut commands: Commands,
    clock: Res<crate::calendar::WorldClock>,
    name: Option<Res<DivineName>>,
    children: Query<&Children>,
    motes: Query<Entity, With<PrayerMote>>,
    mut smitings: MessageReader<DivineEvent>,
    mut notices: MessageWriter<crate::ui::Notice>,
    mut ledger: ResMut<PrayerLedger>,
    mut legend: ResMut<Legend>,
    mut praying: Query<
        (
            Entity,
            &Person,
            &Prayer,
            &mut Activity,
            &mut Faith,
            Option<&mut Chronicle>,
            Option<&mut super::Stirrings>,
        ),
        Without<Corpse>,
    >,
) {
    let struck: Vec<Entity> = smitings
        .read()
        .filter(|event| matches!(event.kind, DivineEventKind::Smote))
        .filter_map(|event| event.subject)
        .collect();
    if struck.is_empty() {
        return;
    }
    let god = name.as_ref().map_or("their god", |n| n.0.as_str());

    for (entity, person, prayer, mut activity, mut faith, chronicle, stirred) in &mut praying {
        let PrayerKind::Dark {
            against,
            name: enemy,
            ..
        } = &prayer.kind
        else {
            continue;
        };
        if !struck.contains(against) {
            continue;
        }

        faith.shift(0.35);
        legend.dread += 2.0;
        *activity = Activity::Idle;
        info!(
            "{}'s prayer against {enemy} was answered by {god}",
            person.name
        );
        notices.write(crate::ui::Notice::fanfare(format!(
            "{}'s dark prayer was answered",
            person.name
        )));
        if let Some(mut chronicle) = chronicle {
            chronicle.record(
                clock.day(),
                format!("prayed against {enemy}, and the lightning came"),
            );
        }
        if let Some(mut stirred) = stirred {
            stirred.stir(
                clock.day(),
                format!("the god struck {enemy} for them - dark faith"),
            );
        }
        ledger.close(&person.name, prayer.words.clone(), PrayerOutcome::Answered);
        end_prayer(&mut commands, entity, &children, &motes);
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
    mut ledger: ResMut<PrayerLedger>,
    mut praying: Query<
        (
            Entity,
            &mut Prayer,
            &Person,
            &mut Activity,
            &mut Faith,
            Option<&mut Chronicle>,
            Option<&mut super::Stirrings>,
        ),
        Without<Corpse>,
    >,
) {
    for (entity, mut prayer, person, mut activity, mut faith, chronicle, stirred) in &mut praying {
        prayer.remaining -= time.delta_secs();
        if prayer.remaining > 0.0 {
            continue;
        }

        // Hope unanswered fades; it does not sour. A devotion asked for
        // nothing the larder could meet, so its lapse seeds no doubt,
        // raises no notice, and costs the god nothing — the receipt on
        // the board is the whole of it.
        if matches!(prayer.kind, PrayerKind::Devotion { .. }) {
            ledger.close(&person.name, prayer.words.clone(), PrayerOutcome::Faded);
            if *activity == Activity::Praying {
                *activity = Activity::Idle;
            }
            if let Some(mut chronicle) = chronicle {
                chronicle.record(clock.day(), "offered a small prayer; the wind kept it");
            }
            end_prayer(&mut commands, entity, &children, &motes);
            continue;
        }

        faith.shift(-0.15);
        if *activity == Activity::Praying {
            *activity = Activity::Idle;
        }
        if let Some(mut stirred) = stirred {
            stirred.stir(clock.day(), "prayed into silence - doubt crept in");
        }
        ledger.close(&person.name, prayer.words.clone(), PrayerOutcome::Curdled);
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
    let _ = god;
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
                // The worldly turns, for the few who attributed them at all.
                // A death read as the god's doing cuts both ways by grain:
                // the bold call it the god's right hand, the timid feel the
                // god turn away.
                DivineEventKind::Perished => {
                    if temperament.boldness >= 0.45 {
                        faith.shift(0.06 * conviction);
                        if let Some(mut chronicle) = chronicle {
                            chronicle.record(clock.day(), "believed the god called one home");
                        }
                    } else {
                        faith.shift(-0.09 * conviction);
                        if let Some(mut chronicle) = chronicle {
                            chronicle.record(clock.day(), "believed the god let one of us die");
                        }
                    }
                }
                DivineEventKind::Delivered => {
                    faith.shift(0.06 * conviction);
                    if let Some(mut chronicle) = chronicle {
                        chronicle.record(clock.day(), "gave thanks for the newborn");
                    }
                }
                DivineEventKind::Flourished => {
                    faith.shift(0.05 * conviction);
                    if let Some(mut chronicle) = chronicle {
                        chronicle.record(clock.day(), "read the god's favor in the harvest");
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
pub fn flourish(site: &SettlementSite, bushes: &mut Query<(&GlobalTransform, &mut FoodSource)>) {
    for (at, mut source) in bushes.iter_mut() {
        if at.translation().distance(site.center) < 160.0 {
            source.amount = source.amount.max(3.0);
        }
    }
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
            // The worldly turns weigh lighter on the legend than the hand's
            // own acts: they happen to every village, whoever its god is.
            DivineEventKind::Delivered | DivineEventKind::Flourished => legend.providence += 0.3,
            DivineEventKind::Perished => legend.dread += 0.3,
            // The wonders feed the legends like their kin: called rain and
            // the beckoning light are providence, the falling stone and the
            // sown shadow are dread of the oldest kind.
            DivineEventKind::Rained => legend.providence += 0.5,
            DivineEventKind::Beckoned => legend.providence += 0.6,
            DivineEventKind::Fell => legend.dread += 1.2,
            DivineEventKind::DoubtSown => legend.dread += 0.8,
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
        app.init_resource::<PrayerLedger>();
        app.add_systems(
            Update,
            (kneel, answer_prayers, answer_devotions, despair).chain(),
        );

        let settlement = app
            .world_mut()
            .spawn(super::super::work::Stockpile::default())
            .id();
        app.insert_resource(SettlementSite {
            center: Vec3::ZERO,
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
        // And the board gets its receipt.
        let ledger = app.world().resource::<PrayerLedger>();
        assert!(
            ledger
                .closed
                .iter()
                .any(|closed| matches!(closed.outcome, PrayerOutcome::Answered)),
            "an answered prayer must reach the board's ledger",
        );

        // THE LOOP: the gift still lies in reach and the hunger is still
        // high, and Tiwa knelt again the very next frame — answered again,
        // asked again, thirteen receipts and a tray full of spam. Brett:
        // "she prayed for food and I answered and it spammed
        // notifications." A gift in reach IS the answer: no re-kneeling.
        app.update();
        app.update();
        assert!(
            app.world().get::<Prayer>(soul).is_none(),
            "a gift within reach must hold the next asking",
        );
        let receipts = app
            .world()
            .resource::<PrayerLedger>()
            .closed
            .iter()
            .filter(|closed| matches!(closed.outcome, PrayerOutcome::Answered))
            .count();
        assert_eq!(receipts, 1, "one gift, one answer, one receipt");
    }

    /// A full larder holds the prayer wherever the hungry are standing -
    /// and lets it go the moment the walk home can no longer save them.
    ///
    /// This rule has now been round the houses twice, and both readings
    /// were right about the game they were written for.
    ///
    /// It began as "a stocked larder is comfort", which killed Kileb 172
    /// strides out: his town's sacks were full, his prayer was held, and
    /// nobody came. So the road was exempted - out there, hunger prayed at
    /// desperate whatever the larder said.
    ///
    /// That was honest while NOBODY ON THE ROAD EVER TURNED BACK TO EAT,
    /// which was true until today: the decision to go and eat was only ever
    /// taken by somebody already idle, so an errand in hand meant walking
    /// on until you died. With that fixed, the walk home IS the answer to
    /// being hungry on the road, and a prayer over a full granary is a
    /// false alarm - Brett: "peopel are praying for food even though the
    /// grainery has 52 food."
    ///
    /// So the larder holds the prayer at any distance, and `dying_regardless`
    /// releases it for whoever cannot reach it in time. Kileb would walk
    /// home now; if something stopped him, he would still be heard.
    #[test]
    fn a_full_larder_holds_the_prayer_until_it_cannot_save_them() {
        let (mut app, soul) = starving_world();
        let settlement = app.world().resource::<SettlementSite>().settlement;
        app.world_mut()
            .entity_mut(settlement)
            .insert(crate::villager::SettlementGround {
                center: Vec3::ZERO,
                radius: 36.0,
                woodpile: Vec3::ZERO,
                foodpile: Vec3::ZERO,
            });
        app.world_mut()
            .get_mut::<super::super::work::Stockpile>(settlement)
            .unwrap()
            .larder
            .add(super::super::work::FoodKind::Berries, 20.0);
        app.world_mut()
            .entity_mut(soul)
            .insert(super::super::MemberOf(settlement));
        app.world_mut().get_mut::<Needs>(soul).unwrap().hunger = 0.75;

        // At home, that hunger waits for the walk to the sacks.
        app.update();
        assert!(
            app.world().get::<Prayer>(soul).is_none(),
            "a stocked larder in walking reach should hold the prayer",
        );

        // And on the road it still waits, because the walk home is now a
        // thing that actually happens.
        app.world_mut()
            .get_mut::<Transform>(soul)
            .unwrap()
            .translation = Vec3::new(120.0, 0.0, 0.0);
        app.update();
        assert!(
            app.world().get::<Prayer>(soul).is_none(),
            "a full larder and a walk they will take is not worth a prayer",
        );

        // Past saving, it goes up wherever they are: this is the one that
        // Kileb needed, and the one that must never be gated on comfort.
        app.world_mut().get_mut::<Needs>(soul).unwrap().hunger = 0.95;
        app.update();
        assert!(
            app.world().get::<Prayer>(soul).is_some(),
            "whoever cannot reach the food in time must still be heard",
        );
    }

    /// Hope unanswered fades; it does not sour. A lapsed devotion costs
    /// no faith, raises no notice, and leaves only its receipt.
    #[test]
    fn a_devotion_fades_free() {
        let (mut app, soul) = starving_world();
        // Fed, so the crisis machinery stays out of the way.
        app.world_mut().get_mut::<Needs>(soul).unwrap().hunger = 0.2;
        let faith_before = app.world().get::<Faith>(soul).unwrap().trust;
        app.world_mut().entity_mut(soul).insert(Prayer {
            remaining: 0.05,
            words: None,
            bubbled: false,
            kind: PrayerKind::Devotion { grateful: false },
        });

        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs(1));
        app.update();

        assert!(
            app.world().get::<Prayer>(soul).is_none(),
            "the devotion should close",
        );
        let faith_after = app.world().get::<Faith>(soul).unwrap().trust;
        assert_eq!(
            faith_before, faith_after,
            "a faded devotion must cost nothing",
        );
        let ledger = app.world().resource::<PrayerLedger>();
        assert!(
            ledger
                .closed
                .iter()
                .any(|closed| matches!(closed.outcome, PrayerOutcome::Faded)),
            "the board keeps the faded receipt",
        );
    }

    /// A devotion is answered by the god showing themself: any
    /// unmistakably divine act the asker witnesses while it hangs.
    #[test]
    fn a_devotion_is_answered_by_any_divine_sign() {
        let (mut app, soul) = starving_world();
        app.world_mut().get_mut::<Needs>(soul).unwrap().hunger = 0.2;
        let faith_before = app.world().get::<Faith>(soul).unwrap().trust;
        app.world_mut().entity_mut(soul).insert(Prayer {
            remaining: DEVOTION_PATIENCE,
            words: None,
            bubbled: false,
            kind: PrayerKind::Devotion { grateful: false },
        });

        // The god lifts a stone across the square, in plain sight.
        app.world_mut().write_message(DivineEvent {
            kind: DivineEventKind::Lifted,
            position: Vec3::new(20.0, 0.0, 0.0),
            subject: None,
            intensity: 0.6,
        });
        app.update();

        assert!(
            app.world().get::<Prayer>(soul).is_none(),
            "the sign should answer the asking",
        );
        assert!(
            app.world().get::<Faith>(soul).unwrap().trust > faith_before,
            "seeing the answer deepens faith",
        );
        let ledger = app.world().resource::<PrayerLedger>();
        assert!(
            ledger
                .closed
                .iter()
                .any(|closed| matches!(closed.outcome, PrayerOutcome::Answered)),
            "the board records the answered devotion",
        );
    }

    /// A stocked larder must never silence someone dying beside it.
    ///
    /// Six villagers starved in VisitingStore at hunger 1.00 with the larder
    /// reading full — the square had gone unwalkable, no meal could be
    /// reached — and the old gate treated "there is food somewhere in town"
    /// as a reason not to pray. Dying of hunger is the unambiguous case: the
    /// prayer goes up regardless of what the ledgers say.
    #[test]
    fn the_dying_pray_past_a_stocked_larder() {
        let (mut app, soul) = starving_world();
        let settlement = app.world().resource::<SettlementSite>().settlement;
        app.world_mut()
            .get_mut::<super::super::work::Stockpile>(settlement)
            .unwrap()
            .larder
            .add(super::super::work::FoodKind::Berries, 10.0);
        app.world_mut()
            .entity_mut(soul)
            .insert(super::super::MemberOf(settlement));

        // Merely desperate: the stocked larder is a better answer than the
        // god, and no prayer should rise over a solvable meal.
        app.world_mut().get_mut::<Needs>(soul).unwrap().hunger = 0.8;
        app.update();
        assert!(
            app.world().get::<Prayer>(soul).is_none(),
            "hunger a full larder can still fix should not kneel",
        );

        // Dying: whatever the ledgers say, the prayer goes up.
        app.world_mut().get_mut::<Needs>(soul).unwrap().hunger = 0.97;
        app.update();
        assert!(
            app.world().get::<Prayer>(soul).is_some(),
            "someone starving to death must pray even beside a full larder",
        );
    }

    /// A meal ends a food prayer no matter where the meal came from.
    ///
    /// Without this, a villager fed from the larder kept their stale prayer
    /// open until it curdled, and the village lost faith over dinners that
    /// were eaten on time.
    #[test]
    fn a_meal_ends_a_food_prayer() {
        let (mut app, soul) = starving_world();
        app.update();
        assert!(app.world().get::<Prayer>(soul).is_some());

        // The larder comes back and they eat — by whatever table fed them.
        app.world_mut().get_mut::<Needs>(soul).unwrap().hunger = 0.1;
        use bevy::ecs::system::RunSystemOnce;
        app.world_mut()
            .run_system_once(meals_answer_prayers)
            .unwrap();

        assert!(
            app.world().get::<Prayer>(soul).is_none(),
            "a fed villager's food prayer should close",
        );
        let ledger = app.world().resource::<PrayerLedger>();
        assert!(
            ledger
                .closed
                .iter()
                .any(|closed| matches!(closed.outcome, PrayerOutcome::Fed)),
            "the board should show the prayer ended with a meal",
        );
    }

    #[test]
    fn the_ledger_keeps_only_the_recent_receipts() {
        let mut ledger = PrayerLedger::default();
        for n in 0..12 {
            ledger.close(&format!("Soul {n}"), None, PrayerOutcome::Curdled);
        }
        assert_eq!(ledger.closed.len(), PrayerLedger::KEPT);
        // Oldest fell off the front; the newest is the last.
        assert_eq!(ledger.closed.first().unwrap().name, "Soul 4");
        assert_eq!(ledger.closed.last().unwrap().name, "Soul 11");
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
        let ledger = app.world().resource::<PrayerLedger>();
        assert!(
            ledger
                .closed
                .iter()
                .any(|closed| matches!(closed.outcome, PrayerOutcome::Curdled)),
            "a curdled prayer must reach the board's ledger",
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
    fn belief_never_reads_negative() {
        // Belief is the LADDER now, not the fuel - nothing spends it - but
        // the arithmetic guard stays for whatever old saves carry.
        let belief = Belief {
            total: 12.0,
            spent: 100.0,
        };
        assert_eq!(belief.available(), 0.0);
    }
}
