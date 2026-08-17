//! Villagers: one need, and the decision between satisfying it and idling.
//!
//! The decision layer is utility-based from the start even though there are only
//! two options. That is deliberate — the shape it needs to grow into is a dozen
//! competing needs scored against each other, and retrofitting scoring onto a
//! state machine later would mean rewriting every behavior. Adding a need here
//! means adding a scorer, not editing a branch.

pub mod attire;
pub mod belief;
pub mod civic;
pub mod colony;
pub mod explore;
pub mod home;
pub mod names;
pub mod rampart;
pub mod rites;
pub mod speech;
pub mod tools;
pub mod traits;
pub mod work;

pub mod gossip;
pub mod kin;
pub mod regard;
pub(crate) use gossip::*;

use bevy::prelude::*;

use crate::creature::anim::CreatureMotion;
use crate::creature::body::{CreatureAssets, build_body};
use crate::creature::genome::{Age, CreatureGenome, Sex, Species};
use crate::creature::{
    Airborne, Childhood, Held, MoveTarget, random_walkable_point, spawn_creature,
};
use crate::palette;
use crate::rng::Rng;
use crate::scatter::FoodSource;
use crate::terrain::{Biome, Terrain, WATER_LEVEL};

/// How many villagers the settlement starts with: five women and five
/// men, all grown, dealt by the alternation in `found_the_village`.
///
/// Ten because the hall sleeps ten. The founding village is housed from
/// its first minute, which is the whole point - the first quarter-hour
/// used to be twelve people in the dirt splitting a dozen hands between
/// food, timber, stone and a frame, and losing at all four. If the bench
/// longhouse is ever redrawn with a different number of beds, this
/// should move with it.
pub const STARTING_POPULATION: usize = 10;

/// Seconds of not eating it takes to go from fed to starving.
///
/// A day and a half (day = 600s). This is not a starvation simulator:
/// hunger is a rhythm the day is built around - a reason to fish, a
/// reason to come home - not the thing most likely to kill you. At 300 a
/// stomach lasted half a day, and any walk past eighty strides was a
/// gamble with a life; at the 150 before that, food logistics smothered
/// every other ambition the village had.
pub(crate) const SECONDS_TO_STARVE: f32 = 900.0;

/// Hunger above this makes finding food the villager's priority.
const HUNGRY_THRESHOLD: f32 = 0.35;

/// How close a villager must be to a bush to eat from it.
const EATING_RANGE: f32 = 1.6;

/// Minimum fraction of a settlement's surroundings that must be walkable ground.
const MIN_SETTLEMENT_LAND: f32 = 0.55;

/// Seconds of standing at empty starving before hunger kills. Long
/// enough that an empty larder is a crisis somebody can still walk out
/// of, and a village that is losing people to it has been failing for
/// most of a day rather than a couple of minutes.
const SECONDS_STARVING_TO_DIE: f32 = 300.0;

/// Seconds of decent feeding to heal from the brink back to whole.
/// Kept above SECONDS_STARVING_TO_DIE, and moved up with it: dying is
/// faster than healing, deliberately, and softening hunger must not
/// quietly turn a wound into something a good lunch undoes.
const SECONDS_TO_MEND: f32 = 400.0;

// There is no numeric population cap: a village grows as far as its
// shelter, its larder and its land allow. Cities are meant to get huge -
// the limits are the ones the villagers build their way past.

/// Seconds between chances of a birth.
const BIRTH_INTERVAL: f32 = 18.0;

/// Seconds a newborn takes to come of age: sixteen days - a childhood
/// spanning most of a season, proportioned to the 28-day calendar.
const SECONDS_TO_COME_OF_AGE: f32 = crate::calendar::DAY_SECONDS * 16.0;

/// Seconds of adulthood before age begins to show: three seasons of
/// prime. Under the old 950 seconds every founding mother was an elder
/// by the first week and no village could ever grow - lifespans must be
/// proportioned to the calendar they live inside.
const SECONDS_OF_PRIME: f32 = crate::calendar::DAY_SECONDS * 84.0;

/// Seconds between rounds of courtship.
const BOND_INTERVAL: f32 = 12.0;

/// How close two people must be for a bond to form between them.
///
/// Proximity, not a global roll: who marries whom emerges from who actually
/// crosses paths — which the player shapes every time they move someone, place
/// food, or scatter a crowd.
const COURTSHIP_DISTANCE: f32 = 12.0;

/// Days a pair walk out together before they wed. Without this, standing
/// near someone WAS the wedding: on the founding morning ten strangers
/// are milling around one fire, and half the village married inside a
/// minute of the world beginning.
const COURTSHIP_DAYS: u32 = 4;

/// How near the god's house has to be to be the village's own.
const SHRINE_REACH: f32 = 140.0;

/// A pair who are walking out together, and the day they began.
#[derive(Component, Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Courting {
    pub with: Entity,
    pub since: u32,
}

impl Courting {
    /// Whether this pair have courted long enough to want a wedding.
    /// What the village reads to decide it needs a shrine.
    pub fn ripe(&self, today: u32) -> bool {
        today.saturating_sub(self.since) >= COURTSHIP_DAYS
    }
}

/// Seconds between rounds of talk.
const GOSSIP_INTERVAL: f32 = 8.0;

/// How close two people must be to talk.
const EARSHOT: f32 = 7.0;

pub struct VillagerPlugin;

impl Plugin for VillagerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WorldChronicle>()
            // Each town's graveyard, keyed by settlement. Registered rather
            // than inserted on first burial, because the system that chooses
            // the ground now writes into it.
            .init_resource::<rites::RestingGround>()
            .init_resource::<work::KitchenWarm>()
            .init_resource::<work::StoreTrends>()
            .init_resource::<belief::Belief>()
            .init_resource::<belief::FaithHistory>()
            .init_resource::<belief::Legend>()
            .init_resource::<belief::PrayerLedger>()
            .add_message::<home::Knock>()
            .add_message::<home::KnockReport>()
            .add_systems(Update, (home::answer_the_knock, work::pin_petitions))
            .add_systems(
                Update,
                (
                    regard::ensure_regard,
                    regard::feelings_cool,
                    regard::kin_warmth,
                    ensure_stirrings,
                ),
            )
            // The village is founded when the flag goes in, not when the
            // program starts. A world with nobody in it is the opening.
            .init_resource::<ChosenGround>()
            // Known ground exists from the first frame, empty. It is the
            // village's own once one is founded, but the systems that
            // read it run whether or not there is a village.
            .init_resource::<explore::KnownWorld>()
            .init_resource::<explore::GroundWanted>()
            // AFTER the walk home is decided, so a bush at hand beats a
            // larder half a mile off. Both systems steer the same hungry
            // villager: `eat_from_store` now turns anyone past desperate
            // toward the sacks wherever they are, and this one feeds them
            // off the land they are standing on. Whichever runs last wins
            // the errand - and the right answer is the food in arm's reach.
            // Brett: "shouldnt they eat form bushes when on the road?"
            //
            // It only takes the errand when it FINDS something: no carcass
            // and no fruiting bush leaves the walk home standing.
            .add_systems(
                Update,
                explore::the_road_feeds_who_walks_it.after(work::eat_from_store),
            )
            .add_systems(Startup, deal_the_dice)
            // The maker's own bench, furnished with this game's palette and
            // vocabulary before anybody opens it. Startup rather than a
            // build step: what it writes has to match the game the player is
            // actually running, not the one somebody packaged.
            .add_systems(Startup, |_: Commands| {
                work::baked::furnish_the_makers_bench()
            })
            // Its own registration rather than the chain above: that
            // tuple is at Bevy's twenty-system ceiling, and the rising
            // needs no ordering against any of it.
            .add_systems(Update, work::rise_out_of_the_earth)
            // The doors: leaves hung the moment a shell stands, swung for
            // whoever comes and goes. Unordered on purpose - a door that
            // opens a frame late is a door.
            .add_systems(
                Update,
                (
                    work::buildings::hang_the_doors,
                    work::buildings::swing_the_doors,
                ),
            )
            // Likewise unordered: the trades dress whoever took up a
            // calling this frame, whenever in the frame that happened.
            .add_systems(
                Update,
                (attire::dress_for_work, attire::twirl_to_redress).chain(),
            )
            .add_systems(
                Update,
                (tools::equip_work_tools, tools::animate_work_tools).chain(),
            )
            .add_systems(
                Update,
                (
                    work::advance_hunting_arrows,
                    work::baked::tell_the_hour,
                    // Its own registration: the muster tuple above is at
                    // Bevy's chain-tuple ceiling, and one more element there
                    // stops compiling with an error about trait bounds that
                    // says nothing about the real cause.
                    work::stack_on_the_pallets,
                ),
            )
            // Civic life: the ballot and the decree, neither ordered
            // against anything - a mayor chosen a frame late is chosen.
            .add_systems(
                Update,
                (
                    civic::hold_elections,
                    civic::set_the_agenda,
                    civic::stage_civic_assemblies,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                name_the_god.run_if(not(resource_exists::<DivineName>)),
            )
            .add_systems(
                OnEnter(crate::GameState::Playing),
                (spawn_settlement, point_camera_at_settlement).chain(),
            )
            .add_systems(
                Update,
                (
                    home::assign_beds,
                    home::hold_abed,
                    rites::mark_the_dead,
                    rites::mourn,
                    rites::burials,
                    seek_company,
                    speech::muse_the_watched,
                    speech::remember_what_was_said,
                    speech::show_musings,
                    stretch_settlement,
                    explore::chart_worksites,
                    explore::walk_the_world,
                    explore::expeditions,
                    explore::escort_duty,
                    explore::raise_cairns,
                )
                    .chain(),
            )
            // The stopwatch brackets: each big chain banks its span under
            // DIVUS_FACTUS_TIMINGS=1, so the unwatched column can say which
            // chain to open instead of shrugging. Every bracket carries BOTH
            // edges - pinned to the seam between two chains - or the
            // scheduler runs it at the frame's rim and the span swallows
            // the whole schedule instead of its own chain.
            .add_systems(
                Update,
                (
                    crate::debug::timings::open_span("villager: rhythms").before(home::assign_beds),
                    crate::debug::timings::close_span("villager: rhythms")
                        .after(explore::raise_cairns),
                    // The halves, hunting a burst no watch has named: if one
                    // half fattens alone the beast is a member of it, and if
                    // both fatten together the beast is a stranger both wait
                    // on. Retire these once it is caught.
                    crate::debug::timings::close_span("villager: rhythms, rites half")
                        .after(rites::burials)
                        .before(seek_company),
                    crate::debug::timings::open_span("villager: rhythms, rites half")
                        .before(home::assign_beds),
                    crate::debug::timings::open_span("villager: rhythms, roads half")
                        .after(rites::burials)
                        .before(seek_company),
                    crate::debug::timings::close_span("villager: rhythms, roads half")
                        .after(explore::raise_cairns),
                    crate::debug::timings::open_span("villager: the body")
                        .before(accumulate_hunger),
                    crate::debug::timings::close_span("villager: the body")
                        .after(choose_activity)
                        .before(work::assign_vocations),
                    crate::debug::timings::close_span("villager: the body, ages half")
                        .after(births)
                        .before(bereave),
                    crate::debug::timings::open_span("villager: the body, ages half")
                        .before(accumulate_hunger),
                    crate::debug::timings::open_span("villager: the body, talk half")
                        .after(births)
                        .before(bereave),
                    crate::debug::timings::close_span("villager: the body, talk half")
                        .after(choose_activity)
                        .before(work::assign_vocations),
                    crate::debug::timings::open_span("witness: the set")
                        .before(crate::witness::WitnessSet),
                    crate::debug::timings::close_span("witness: the set")
                        .after(crate::witness::WitnessSet),
                    crate::debug::timings::open_span("villager: the work")
                        .after(choose_activity)
                        .before(work::assign_vocations),
                    crate::debug::timings::close_span("villager: the work")
                        .after(work::log_stores)
                        .before(belief::endow_faith),
                    crate::debug::timings::open_span("villager: the belief")
                        .after(work::log_stores)
                        .before(belief::endow_faith),
                    crate::debug::timings::close_span("villager: the belief")
                        .after(work::settle_field_claims)
                        .before(home::assign_homes),
                    crate::debug::timings::open_span("villager: the homes")
                        .after(work::settle_field_claims)
                        .before(home::assign_homes),
                    crate::debug::timings::close_span("villager: the homes").after(record_history),
                ),
            )
            .add_systems(
                Update,
                (
                    (
                        accumulate_hunger,
                        starvation_watch,
                        grow_up,
                        grow_old,
                        form_bonds,
                        births,
                        bereave,
                        chronicle_divine_touch,
                        meet_to_talk,
                        hold_conversations,
                        choose_activity,
                    )
                        .chain(),
                    (
                        work::assign_vocations,
                        work::morning_muster,
                        work::forget_shunned,
                        work::plan_houses,
                        work::take_up_work,
                        work::open_boardwalks,
                        (work::do_work, work::apply_work_facing).chain(),
                        work::grow_crops,
                        work::sermons,
                        work::eat_from_store,
                        // Paired: the chain tuple is at Bevy's ceiling, and
                        // these two are one subject - things becoming stores.
                        (work::haul_wood, work::receive_offerings),
                        work::redress_carriers,
                        work::salvage_timber,
                        work::lend_a_hand,
                        work::update_woodpile,
                        work::update_store_piles,
                        work::stores_move_indoors,
                        // Paired: the chain tuple is at Bevy's ceiling, and
                        // these two are one subject - piles moving and the
                        // town's delivery points following them.
                        (work::rehouse_stores, work::pile_points_follow),
                        work::track_store_trends,
                        work::log_stores,
                    )
                        .chain(),
                    (
                        belief::endow_faith,
                        // Paired: the chain tuple is at Bevy's ceiling, and
                        // these are one subject - the kneelings and their
                        // answers.
                        (
                            belief::kneel,
                            belief::kneel_in_hatred,
                            belief::kneel_in_devotion,
                        ),
                        belief::take_a_knee,
                        // Tripled: the chain tuple is at Bevy's ceiling, and
                        // these are one subject - the askings being answered.
                        // The road's judge runs before despair on purpose:
                        // a lapsed road prayer departs unheard rather than
                        // curdling like a hunger nobody fed.
                        (
                            belief::answer_prayers,
                            belief::answer_dark_prayers,
                            belief::answer_devotions,
                            colony::bless_or_bar_the_road,
                        ),
                        belief::despair,
                        belief::faith_of_witnesses.after(crate::witness::WitnessSet),
                        belief::tally_belief,
                        belief::grow_legend,
                        belief::ascend,
                        belief::record_faith,
                        belief::animate_motes,
                        belief::show_prayer_bubbles,
                        // Paired: the chain tuple is at Bevy's ceiling, and
                        // these two are one subject - prayers closing, by
                        // death or by dinner.
                        (
                            belief::close_the_prayers_of_the_dead,
                            belief::meals_answer_prayers,
                        ),
                        // Paired: the chain tuple nears Bevy's ceiling, and
                        // these two are one subject - faith marks floating.
                        (belief::mark_the_faith_moved, belief::fade_faith_marks),
                        work::bake,
                        work::smelt,
                        work::dye_cloth,
                        work::famine_watch,
                        // Sharing one seat, and not for tidiness: this
                        // chain stands at Bevy's tuple ceiling, so a new
                        // system joins it only by sitting with its kin.
                        (
                            work::the_sacks_hold_what_they_hold,
                            work::settle_field_claims,
                            rampart::raise_the_fence,
                            rampart::stand_the_posts,
                        )
                            .chain(),
                    )
                        .chain(),
                    (
                        home::assign_homes,
                        home::rehome_the_misplaced,
                        colony::muster_colonists,
                        colony::walk_to_the_new_ground,
                        home::burn_weathered,
                        home::take_shelter,
                        home::midday_meal,
                        home::family_supper,
                        home::tavern_evenings,
                        home::well_gatherings,
                        home::tavern_cheer,
                        home::tend_fire,
                        home::night_routine,
                        home::use_doors,
                        home::weariness,
                        home::burn,
                        home::rouse_the_taken,
                        pursue_activity,
                        crate::scatter::regrow_food,
                        record_history,
                    )
                        .chain(),
                )
                    .chain(),
            );
    }
}

/// Marks a human villager, as distinct from wildlife.
#[derive(Component)]
pub struct Villager;

/// Adulthood, counting quietly down toward gray hair.
#[derive(Component, Debug)]
pub struct Prime {
    pub remaining: f32,
}

/// Who this person is wed to. Inserted on both partners.
///
/// The bond outlives its people: a spouse who dies is still pointed at, which is
/// how the inspector knows a widow from a spinster — and how grief will one day
/// know whom to belong to.
#[derive(Component, Debug, Clone, Copy)]
pub struct Spouse(pub Entity);

/// Who bore and fathered this person. Recorded at birth, kept for life.
///
/// The founding generation has none — they are the first families, sprung whole
/// from the world. Everyone after is somebody's child.
#[derive(Component, Debug, Clone, Copy)]
pub struct Parentage {
    pub mother: Entity,
    pub father: Entity,
}

/// Who someone is.
///
/// The given name is fixed at birth and never changes. Everything the belief
/// system will eventually record — what they saw, what they concluded, who they
/// told — hangs off a person rather than an entity id.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Person {
    pub name: String,
    /// The family name they answer to now, carried down the father's line. A
    /// wife takes her husband's at the wedding, so a household reads as one
    /// house. Empty for anyone restored from a save written before families
    /// had names.
    #[serde(default)]
    pub surname: String,
    /// The house they were BORN into, set once and never changed.
    ///
    /// Kept because the marriage that renames a woman is exactly the edge a
    /// family tree needs to draw: without this, a wife is cut loose from her
    /// parents the day she weds and the maternal half of every lineage
    /// vanishes. Equal to `surname` for anyone who never changed it.
    #[serde(default)]
    pub born_surname: String,
}

impl Person {
    /// A person newly come into the world, born to a house.
    pub fn born(name: String, surname: String) -> Person {
        Person {
            name,
            born_surname: surname.clone(),
            surname,
        }
    }

    /// Given name and family name together, for anywhere a person is being
    /// *identified* rather than merely mentioned. Logs and notices keep to
    /// the given name — "Daekru raised a house" reads better than the full
    /// formal version, and the village would say it that way too.
    /// The name as a roster lists it: "Tiwep, Rafant".
    ///
    /// A roll of people is read by SURNAME - you look for a family and
    /// then for the one you want inside it - and an A-Z list of given
    /// names scatters every household across the page. Brett: "the people
    /// list in the codex should be Last, First."
    ///
    /// Everywhere else keeps `full_name`: over their head, in the ledger's
    /// prose, in a bubble, a person is Rafant Tiwep.
    pub fn roll_name(&self) -> String {
        if self.surname.is_empty() {
            self.name.clone()
        } else {
            format!("{}, {}", self.surname, self.name)
        }
    }

    pub fn full_name(&self) -> String {
        if self.surname.is_empty() {
            self.name.clone()
        } else {
            format!("{} {}", self.name, self.surname)
        }
    }

    /// The house they were born into, where that is not the one they carry —
    /// the thread back to their parents for anyone reading a family tree.
    ///
    /// Nothing draws the tree yet, so nothing on screen calls this. It exists
    /// because the data has to be *kept* from the first wedding onward: a
    /// maiden name that was never recorded cannot be recovered later.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn maiden_house(&self) -> Option<&str> {
        let born = self.born_surname.as_str();
        if born.is_empty() || born == self.surname {
            None
        } else {
            Some(born)
        }
    }

    /// The full name with the birth house noted where it differs: the form a
    /// codex or a family tree wants, as against the form a neighbor shouts.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn name_with_house(&self) -> String {
        match self.maiden_house() {
            Some(born) => format!("{} (of {born})", self.full_name()),
            None => self.full_name(),
        }
    }
}

/// A community: a named place people belong to.
///
/// An entity, not a resource, because there will one day be more than one — and
/// because a settlement with a founder and members is a *thing that happened*,
/// not a property of the map.
#[derive(Component, Debug)]
pub struct Settlement {
    pub name: String,
    /// The day it was raised.
    pub founded: u32,
    /// The cloth the banner flies, as a palette ramp index.
    pub banner_ramp: usize,
    /// The sign upon it, into [`crate::sigil::SIGILS`].
    pub sigil: usize,
}

/// Which settlement this person belongs to.
#[derive(Component, Debug, Clone, Copy)]
pub struct MemberOf(pub Entity);

/// Everything that has ever happened, stamped with the day and hour.
///
/// The written history of the world: every notice the simulation announces is
/// also entered here, permanently. Personal chronicles forget their middles;
/// the world's chronicle forgets nothing.
#[derive(Resource, Default, serde::Serialize, serde::Deserialize)]
pub struct WorldChronicle {
    pub events: Vec<HistoryEvent>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HistoryEvent {
    pub stamp: String,
    pub text: String,
    /// Whole days since founding, for the chronicle's time filters. Old
    /// saves default to day zero and land in the first spring.
    #[serde(default)]
    pub day: u32,
}

/// Copies every notice into the permanent record.
fn record_history(
    clock: Res<crate::calendar::WorldClock>,
    mut history: ResMut<WorldChronicle>,
    mut notices: MessageReader<crate::ui::Notice>,
) {
    for notice in notices.read() {
        history.events.push(HistoryEvent {
            stamp: clock.date_phrase(),
            text: notice.text.clone(),
            day: clock.day(),
        });
    }
}

/// What the people call their god.
///
/// The player never chooses it. A god is named by its believers, in their own
/// tongue, at the founding — being named is the first thing belief does to you.
#[derive(Resource, Debug, Clone)]
pub struct DivineName(pub String);

/// One thing that happened to one person.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LifeEvent {
    pub day: u32,
    pub text: String,
}

/// One movement of the heart, and why: the ledger under the numbers.
#[derive(Debug)]
pub struct Stirring {
    pub day: u32,
    pub text: String,
}

/// What has MOVED this person lately — every discrete shift of faith,
/// spirits or feeling, with its cause. The chronicle keeps what they did;
/// this keeps what got to them. Brett: "everytime something changes their
/// stats there should be a log on what changed and why."
///
/// Discrete events only, never climate: rain wearing on the spirits and
/// kin warmth holding steady are weather, and a ledger of weather buries
/// the signal. Deliberately short — the question a heart page answers is
/// what they are like NOW, not a biography.
#[derive(Component, Debug, Default)]
pub struct Stirrings(pub Vec<Stirring>);

/// How many stirrings are worth keeping.
const STIRRINGS_KEPT: usize = 14;

impl Stirrings {
    pub fn stir(&mut self, day: u32, text: impl Into<String>) {
        self.0.push(Stirring {
            day,
            text: text.into(),
        });
        let over = self.0.len().saturating_sub(STIRRINGS_KEPT);
        self.0.drain(..over);
    }
}

/// Everyone can be moved: villagers get an empty ledger the moment they
/// exist, souls from old saves included.
pub(crate) fn ensure_stirrings(
    mut commands: Commands,
    unmoved: Query<Entity, (With<Villager>, Without<Stirrings>)>,
) {
    for soul in &unmoved {
        commands.entity(soul).insert(Stirrings::default());
    }
}

/// A person's own history, in their own order: born, wed, bereaved, seized by
/// the hand of god. This is the raw material doctrine will be spun from — a
/// believer's theology is a reading of their own life.
#[derive(Component, Default, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Chronicle {
    pub events: Vec<LifeEvent>,
}

impl Chronicle {
    pub fn record(&mut self, day: u32, text: impl Into<String>) {
        // A whole life is kept: the chronicle page shows everything, and
        // doctrine will one day be spun from the full record. But hearing
        // the same sermon four nights running is one fact, not four - an
        // exact repeat of the latest entry only deepens it.
        let text = text.into();
        if let Some(last) = self.events.last_mut()
            && last
                .text
                .trim_end_matches(|c: char| c == ')' || c.is_ascii_digit() || c == ' ')
                .trim_end_matches("(x")
                .trim_end()
                == text
        {
            let seen = last
                .text
                .rsplit_once("(x")
                .and_then(|(_, n)| n.trim_end_matches(')').parse::<u32>().ok())
                .unwrap_or(1);
            last.text = format!("{text} (x{})", seen + 1);
            last.day = day;
            return;
        }
        self.events.push(LifeEvent { day, text });
    }

    /// Records a rumor heard, stacking retellings of the same story into one
    /// line — "heard from Gayou and 3 others that ..." — because a life is
    /// not a ledger of repetitions.
    pub fn hear(&mut self, day: u32, speaker: &str, rumor: &str) {
        let suffix = format!(" that {rumor}");
        if let Some(event) = self
            .events
            .iter_mut()
            .rev()
            .find(|e| e.text.starts_with("heard from") && e.text.ends_with(&suffix))
        {
            let head = event.text[..event.text.len() - suffix.len()].to_string();
            let told = if let Some(base) = head.strip_suffix(" and another") {
                format!("{base} and 2 others")
            } else if let Some((base, count)) = head.rsplit_once(" and ").and_then(|(b, tail)| {
                tail.strip_suffix(" others")
                    .and_then(|n| n.parse::<u32>().ok())
                    .map(|n| (b.to_string(), n))
            }) {
                format!("{base} and {} others", count + 1)
            } else {
                format!("{head} and another")
            };
            event.text = format!("{told}{suffix}");
            return;
        }
        self.record(day, format!("heard from {speaker} that {rumor}"));
    }
}

/// Where a settlement stands, and how far the ground it works reaches.
///
/// A **component**, on the settlement entity, beside its name, banner and
/// stockpile — because the world is going to hold more than one town, and a
/// town's own position cannot live in a resource once that is true. Systems
/// reach it through a villager's [`MemberOf`]: whose town you belong to
/// decides which square you walk back to at dusk.
#[derive(Component, Debug, Clone, Copy)]
pub struct SettlementGround {
    pub center: Vec3,
    pub radius: f32,
    /// Where the visible woodpile stands; where timber is delivered and drawn.
    pub woodpile: Vec3,
    /// Where the food sacks stand; where meals are eaten and the harvest
    /// gathers. Follows the sacks wherever they are rehoused — the square
    /// first, the storehouse when one rises, the granary after that.
    pub foodpile: Vec3,
}

/// The town the player's eye is on: the one the camera opens over, the one the
/// codex panels read, and the one world generation seeded the map around.
///
/// Deliberately NOT the town the simulation works from. Every system that acts
/// on a person resolves their own settlement through [`MemberOf`] instead, so
/// that a second town is simulated exactly as fully as the first. This is a
/// convenience for the interface, and for the founding, and nothing more.
#[derive(Resource, Debug, Clone, Copy)]
pub struct SettlementSite {
    pub center: Vec3,
    pub radius: f32,
    /// Where the visible woodpile stands; where timber is delivered and drawn.
    pub woodpile: Vec3,
    /// The [`Settlement`] entity itself, for membership and naming.
    pub settlement: Entity,
}

impl SettlementSite {
    /// The ground half of this, as it sits on the settlement entity.
    pub fn ground(&self) -> SettlementGround {
        SettlementGround {
            center: self.center,
            radius: self.radius,
            woodpile: self.woodpile,
            // Seeded beside the timber; `pile_points_follow` moves it onto
            // the food sacks the moment they stand.
            foodpile: self.woodpile,
        }
    }
}

/// What this community shares that is not genetic — for now, its language.
///
/// Held as a resource so children are named in the same tongue as their parents.
/// The founding generation's language was built and thrown away before this
/// existed, which would have made every newborn sound foreign in their own home.
#[derive(Resource)]
pub struct SettlementCulture {
    pub language: names::Language,
}

/// Everything a villager currently wants. One field for now; the scoring in
/// [`choose_activity`] is built to take more.
#[derive(Component, Debug, serde::Serialize, serde::Deserialize)]
pub struct Needs {
    /// 0 is fed, 1 is starving.
    pub hunger: f32,
    /// 0 is rested, 1 is exhausted. Sleep drains it; being awake builds it.
    pub rest: f32,
}

impl Default for Needs {
    fn default() -> Self {
        Needs {
            hunger: 0.0,
            rest: 0.2,
        }
    }
}

/// The state of a person's spirits, 0 hollow to 1 bright.
///
/// The first piece of a mind that can suffer: sleeplessness wears it down,
/// good rest restores it. Grief and awe will hang off this same number when
/// doctrine arrives — a mind is where belief will live.
#[derive(Component, Debug, serde::Serialize, serde::Deserialize)]
pub struct Morale {
    pub spirits: f32,
}

impl Default for Morale {
    fn default() -> Self {
        Morale { spirits: 0.8 }
    }
}

/// What a villager has decided to do.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Activity {
    Idle,
    Wandering,
    /// Walking toward a specific food source.
    SeekingFood(Entity),
    /// Standing at a food source, eating.
    Eating(Entity),
    /// At their vocation; the details live in a [`work::Job`].
    Working,
    /// Walking to the settlement store to eat from it.
    VisitingStore,
    /// Carrying wood to the village fire.
    TendingFire,
    /// Carrying timber home to the pile.
    Hauling,
    /// Standing with the dead.
    Mourning,
    /// Stopped face to face with a neighbor, trading news.
    Chatting,
    /// Indoors, waiting out the weather.
    Sheltering,
    /// Carrying a body to the resting ground.
    Bearing,
    /// On their knees, asking.
    Praying,
    /// On their knees at the sight of the god's own hand - and asking for
    /// nothing. Worship is not a request: they saw what happened and
    /// their legs went. Brett: "they should fall to their knees in
    /// worship. It doesnt mean they pray, they just do the animation."
    ///
    /// It is its OWN activity because the witness has to own the body
    /// while it lasts. Reacting used to write `Idle` every frame, the
    /// activity chooser handed the freed villager somewhere to walk, and
    /// the two fought at whatever rate the frame ran - the same shake the
    /// doorways had, for the same reason. Brett: "they start shaking like
    /// crazy. it looks like the door bug."
    Marvelling,
    /// Walking home for the night, or already inside.
    Sleeping,
}

/// Shared generator for behavior decisions.
#[derive(Resource)]
pub struct SimRng(pub Rng);

/// Scores how badly a villager wants to eat.
///
/// Squared so that mild hunger loses to almost anything and real hunger wins
/// against almost everything. A linear curve makes villagers abandon tasks for
/// a snack constantly.
fn food_utility(needs: &Needs) -> f32 {
    if needs.hunger < HUNGRY_THRESHOLD {
        return 0.0;
    }
    let t = (needs.hunger - HUNGRY_THRESHOLD) / (1.0 - HUNGRY_THRESHOLD);
    t * t * 2.0
}

/// Baseline pull toward wandering, so idle villagers still look alive.
fn wander_utility() -> f32 {
    0.15
}

/// How far from the origin the founding settlement may be placed.
///
/// The world is endless, so "somewhere good" has to be bounded by something. A
/// search radius keeps the founding village near where the player starts looking.
const SETTLEMENT_SEARCH_RADIUS: f32 = 900.0;

/// Finds a walkable, dry, reasonably flat site for the settlement — near
/// water on some worlds, well inland on others.
/// Scores one patch of ground as a place to raise a town, or rejects it.
///
/// Shared by the world's first settlement and by every colony founded after
/// How much of the ground a village would stand on is dry, level and
/// buildable: twelve spokes out to the far edge of the building rings.
/// 1.0 is a parade ground, 0.0 is a cliff or open water.
fn level_room(terrain: &Terrain, x: f32, z: f32) -> f32 {
    let mut buildable = 0;
    let mut samples = 0;
    for angle_step in 0..12 {
        let angle = angle_step as f32 / 12.0 * std::f32::consts::TAU;
        for distance in [6.0, 12.0, 18.0, 24.0, 30.0, 36.0] {
            let sx = x + angle.cos() * distance;
            let sz = z + angle.sin() * distance;
            samples += 1;
            if terrain.is_walkable(sx, sz) && terrain.height_at(sx, sz) > WATER_LEVEL + 2.0 {
                buildable += 1;
            }
        }
    }
    buildable as f32 / samples as f32
}

/// What the ground offers a village that stood here: how much of the
/// working walk bears wood, how much is stony enough to shed boulders,
/// and whether there is shore in reach.
///
/// The same reckoning the site search scores on, said out loud - so the
/// god chooses with the founders' own information instead of guessing
/// from the look of the place.
pub fn reckon_ground(terrain: &Terrain, x: f32, z: f32) -> (f32, f32, bool) {
    let mut timber = 0.0;
    let mut stony = 0;
    let mut samples = 0;
    for angle_step in 0..12 {
        let angle = angle_step as f32 / 12.0 * std::f32::consts::TAU;
        for distance in [45.0, 70.0, 95.0, 120.0] {
            let sx = x + angle.cos() * distance;
            let sz = z + angle.sin() * distance;
            samples += 1;
            if terrain.is_submerged(sx, sz) {
                continue;
            }
            if terrain.slope_at(sx, sz) > 0.42 {
                if distance >= 70.0 {
                    stony += 1;
                }
            } else if terrain.forest_at(sx, sz) > 0.50 && terrain.moisture_at(sx, sz) > 0.38 {
                timber += match terrain.biome_at(sx, sz) {
                    Biome::Arid => 0.2,
                    Biome::Alpine => 0.35,
                    _ => 1.0,
                };
            }
        }
    }
    let shore = (0..12).any(|i| {
        let angle = i as f32 / 12.0 * std::f32::consts::TAU;
        (1..=15).any(|step| {
            let d = step as f32 * 4.0;
            terrain.is_submerged(x + angle.cos() * d, z + angle.sin() * d)
        })
    });
    (
        (timber / samples as f32 / 0.33).min(1.0),
        (stony as f32 / samples as f32 / 0.20).min(1.0),
        shore,
    )
}

/// Why this ground will not take a village, or `None` if it will.
///
/// The one standard, used by the site search AND by the flag in the
/// god's hand — a player must not be able to plant where the machine
/// would have refused to. It says WHY, because "no" is not something a
/// person can act on.
pub fn will_take_a_village(terrain: &Terrain, x: f32, z: f32) -> Option<&'static str> {
    if terrain.is_submerged(x, z) {
        return Some("this is water");
    }
    if !terrain.is_walkable(x, z) {
        return Some("too steep to stand a banner in");
    }
    let height = terrain.height_at(x, z);
    if height < WATER_LEVEL + 3.0 {
        // The banner itself stands well above the tide, always.
        return Some("the tide would come up to it");
    }
    // And never on the mountain: summits are for mines, not banners. The
    // line is the ALPINE BIOME - the game's own word for "on the mountain"
    // - not an absolute height. The old check refused anything 30 units
    // over the sea in a world whose ordinary rolling interior sits at 50+,
    // so most of every continent claimed to be a mountain; Brett planted
    // on gentle riverside forest and was told "too high up the mountain."
    if terrain.biome_for(x, z, height) == crate::terrain::Biome::Alpine {
        return Some("too high up the mountain");
    }
    if level_room(terrain, x, z) < MIN_SETTLEMENT_LAND {
        return Some("not enough level ground for a village");
    }
    None
}

/// it, so a daughter town is held to exactly the standards its parent was:
/// dry, buildable, off the mountain, with timber and stone in a working walk.
///
/// Returns the score, the spot, and how much wood and rock lie in reach.
fn score_town_ground(
    terrain: &Terrain,
    x: f32,
    z: f32,
    coastal_yearning: f32,
) -> Option<(f32, Vec3, f32, f32)> {
    // One standard for the searcher and for the god's own flag: whatever
    // ground the machine would refuse, the player is refused too.
    if will_take_a_village(terrain, x, z).is_some() {
        return None;
    }
    let height = terrain.height_at(x, z);

    let buildable_fraction = level_room(terrain, x, z);

    let mut reach_water = 0;
    let mut reach_samples = 0;
    for angle_step in 0..12 {
        let angle = angle_step as f32 / 12.0 * std::f32::consts::TAU;
        for distance in [42.0, 50.0, 58.0] {
            let sx = x + angle.cos() * distance;
            let sz = z + angle.sin() * distance;
            reach_samples += 1;
            if terrain.is_submerged(sx, sz) {
                reach_water += 1;
            }
        }
    }
    let water_fraction = reach_water as f32 / reach_samples as f32;
    let flatness = 1.0 - terrain.slope_at(x, z);

    // Some shoreline in reach is desirable, a lot is a peninsula. Peak
    // the reward around a quarter of the outer band being water.
    let shoreline = 1.0 - ((water_fraction - 0.25) / 0.25).abs().min(1.0);

    // The materials band: what the founders could actually build from.
    // Sample the working-walk ring for ground that will bear trees (the
    // same forest field the scatterer seeds from) and for the steep rocky
    // ground that sheds boulders. A pretty shore with nothing to cut or
    // quarry is a slow death; timber in reach outweighs any view.
    let mut timber = 0.0;
    let mut stony = 0;
    let mut material_samples = 0;
    for angle_step in 0..12 {
        let angle = angle_step as f32 / 12.0 * std::f32::consts::TAU;
        for distance in [45.0, 70.0, 95.0, 120.0] {
            let sx = x + angle.cos() * distance;
            let sz = z + angle.sin() * distance;
            material_samples += 1;
            if terrain.is_submerged(sx, sz) {
                continue;
            }
            if terrain.slope_at(sx, sz) > 0.42 {
                // Rock scores only at a respectful distance: a mountain
                // IN walking reach is wealth, a mountain OVER the
                // bedrolls is a hard place to raise a street.
                if distance >= 70.0 {
                    stony += 1;
                }
            } else if terrain.forest_at(sx, sz) > 0.50 && terrain.moisture_at(sx, sz) > 0.38 {
                // Weight by how thickly this biome actually grows trees,
                // so an arid "forest" cell promises what it delivers.
                timber += match terrain.biome_at(sx, sz) {
                    Biome::Arid => 0.2,
                    Biome::Alpine => 0.35,
                    _ => 1.0,
                };
            }
        }
    }
    // Saturate at about a third of the ring bearing wood: enough to found
    // on, and it keeps whole-forest sites from drowning every other need.
    let timberland = (timber / material_samples as f32 / 0.33).min(1.0);
    let stoneland = (stony as f32 / material_samples as f32 / 0.20).min(1.0);

    let score = buildable_fraction * 4.0
        + flatness * 2.0
        + timberland * 3.0
        + stoneland * 1.0
        + shoreline * coastal_yearning;
    Some((score, Vec3::new(x, height, z), timberland, stoneland))
}

pub(crate) fn choose_settlement_site(terrain: &Terrain, rng: &mut Rng) -> Vec3 {
    let mut best: Option<(f32, Vec3, f32, f32)> = None;
    // How much this founding people care for the sea. Rolled per world:
    // some folk are fishers to the bone, some would rather farm a valley
    // three days from the sound of surf - and with mud brick and masonry
    // in the building repertoire, inland is a real life, not a death.
    let coastal_yearning = rng.range(0.0, 1.5);

    // Outward until there is somewhere to stand.
    //
    // Half of this world is ocean, and the founders begin at the world's
    // reference point whether or not that point is dry. A single search box
    // round the origin used to be the whole of it, with a fallback that planted
    // the village at the origin AT SEA LEVEL if nothing in the box scored — so
    // one seed in several founded a town in open water, on the seabed, with the
    // waves closing over the longhouse. (Seed 3, caught by
    // `settlement_site_is_not_a_sandbar`: not one sample of its surroundings was
    // land.)
    //
    // So the search widens instead. Each ring is four times the area of the last
    // and gets its own sampling, and the first that finds ground wins — which
    // keeps the usual case exactly as it was, a village within a few hundred
    // units of the origin, and only walks further when the origin is at sea.
    for widening in 0..5 {
        let reach = SETTLEMENT_SEARCH_RADIUS * (1u32 << widening) as f32;
        for _ in 0..6_000 {
            let x = rng.range(-reach, reach);
            let z = rng.range(-reach, reach);

            let Some(candidate) = score_town_ground(terrain, x, z, coastal_yearning) else {
                continue;
            };
            if best.is_none_or(|(b, ..)| candidate.0 > b) {
                best = Some(candidate);
            }
        }
        if best.is_some() {
            if widening > 0 {
                info!(
                    "the founders walked further than usual for their ground: {:.0} units of sea to cross",
                    reach
                );
            }
            break;
        }
    }

    if let Some((_, site, timberland, stoneland)) = best {
        info!(
            "the founders chose their ground: woods {:.0}%, stony rises {:.0}% within a working walk",
            timberland * 100.0,
            stoneland * 100.0
        );
        return site;
    }

    // Nothing scored anywhere within fifteen kilometers, which on a world half
    // dry should not happen. Take the highest ground found rather than the
    // origin: a poor village on a hill is recoverable, a village underwater is
    // not.
    let mut driest = Vec3::new(0.0, WATER_LEVEL, 0.0);
    for _ in 0..20_000 {
        let reach = SETTLEMENT_SEARCH_RADIUS * 16.0;
        let x = rng.range(-reach, reach);
        let z = rng.range(-reach, reach);
        let height = terrain.height_at(x, z);
        if height > driest.y {
            driest = Vec3::new(x, height, z);
        }
    }
    warn!(
        "no ground would take a village; the founders settle for the driest land found, {:.0} up",
        driest.y
    );
    driest
}

/// Set before re-running [`spawn_settlement`] during a save load: fixtures
/// are raised at the saved place under the saved names, and no founders or
/// wildlife spawn - the save file supplies the living.
#[derive(Resource)]
pub struct RestoringSeed {
    pub center: Vec3,
    pub name: String,
    pub god: String,
    pub founded: u32,
    /// The banner as it flew: (cloth ramp, sigil). None re-rolls once, for
    /// saves older than heraldry.
    pub banner: Option<(usize, usize)>,
}

/// Raises a town: the settlement entity itself, with its name, arms, ground
/// and a founder's satchel of provisions.
///
/// Split out of `spawn_settlement` so that a town can be founded at any
/// moment of a run and not only at world generation. Everything a settlement
/// needs to *be* a settlement is here; nothing about the world around it.
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_town(
    commands: &mut Commands<'_, '_>,
    center: Vec3,
    settlement_name: &str,
    founded: u32,
    banner_ramp: usize,
    sigil: usize,
) -> Entity {
    let settlement_name = settlement_name.to_string();
    let settlement = commands
        .spawn((
            Name::new(format!("Settlement of {settlement_name}")),
            Settlement {
                name: settlement_name.clone(),
                founded,
                banner_ramp,
                sigil,
            },
            // The banner is the town's face: hover it and the inspector opens
            // on the settlement. Rooted — a god who can uproot the town square
            // is a bug, not a feature. Yet.
            crate::hand::PickRadius(3.4),
            crate::hand::Rooted,
            // The founders arrive with a few days' provisions, not nothing:
            // the first bad berry season should be a scare, not a wipe.
            work::Stockpile {
                // A mixed satchel: berries picked on the road, a little
                // grain from wherever they came from.
                larder: work::Larder {
                    berries: 4.0,
                    grain: 2.0,
                    ..default()
                },
                timber: 0.0,
                stone: 0.0,
                ..default()
            },
            Transform::from_translation(center),
            Visibility::default(),
        ))
        .id();

    commands.entity(settlement).insert(SettlementGround {
        center,
        radius: 36.0,
        woodpile: center,
        foodpile: center,
    });
    settlement
}

/// Raises everything that makes a town's square a square: the banner, the
/// fire, the woodpile, the stone pile and the food sacks.
///
/// Returns where the woodpile stands, which is where timber is delivered and
/// drawn from for the rest of that town's life.
#[allow(clippy::too_many_arguments)]
pub(crate) fn raise_town_fixtures(
    commands: &mut Commands<'_, '_>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    terrain: &Terrain,
    center: Vec3,
    settlement: Entity,
    banner_ramp: usize,
    sigil: usize,
) -> Vec3 {
    // The banner that marks the town's heart — a pole, a crossarm, and a drop
    // of cloth in the village's color. This is where the stockpile lives and
    // where the hungry come when the bushes are bare.
    {
        let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
        let wood = materials.add(StandardMaterial {
            base_color: palette::shade(&palette::WOOD, 0.55),
            perceptual_roughness: 0.9,
            ..default()
        });
        let cloth = materials.add(StandardMaterial {
            base_color: palette::shade(&palette::ALL_RAMPS[banner_ramp], 0.8),
            perceptual_roughness: 0.85,
            double_sided: false,
            cull_mode: None,
            ..default()
        });
        let trim = materials.add(StandardMaterial {
            base_color: palette::shade(&palette::CLOTH_GOLD, 0.9),
            perceptual_roughness: 0.6,
            ..default()
        });

        fn part_rotated(
            commands: &mut Commands,
            cube: &Handle<Mesh>,
            material: &Handle<StandardMaterial>,
            settlement: Entity,
            offset: Vec3,
            rotation: Quat,
            size: Vec3,
        ) {
            commands.spawn((
                Mesh3d(cube.clone()),
                MeshMaterial3d(material.clone()),
                Transform::from_translation(offset)
                    .with_rotation(rotation)
                    .with_scale(size),
                ChildOf(settlement),
            ));
        }
        let mut part = |offset: Vec3, size: Vec3, material: &Handle<StandardMaterial>| {
            commands.spawn((
                Mesh3d(cube.clone()),
                MeshMaterial3d(material.clone()),
                Transform::from_translation(offset).with_scale(size),
                ChildOf(settlement),
            ));
        };

        // Pole, crossarm, finial.
        part(Vec3::new(0.0, 2.4, 0.0), Vec3::new(0.16, 4.8, 0.16), &wood);
        part(Vec3::new(0.7, 4.55, 0.0), Vec3::new(1.6, 0.14, 0.14), &wood);
        part(Vec3::new(0.0, 4.95, 0.0), Vec3::new(0.3, 0.3, 0.3), &trim);
        // The cloth hangs from the crossarm.
        part(
            Vec3::new(0.75, 3.55, 0.0),
            Vec3::new(1.3, 1.9, 0.06),
            &cloth,
        );
        // A gold hem along its foot.
        part(
            Vec3::new(0.75, 2.55, 0.0),
            Vec3::new(1.3, 0.14, 0.08),
            &trim,
        );
        // The sign, in blocks proud of the cloth, on both faces - the very
        // rectangles the codex draws, made voxel, inked by the rule of
        // tincture so it reads on any cloth.
        let field = palette::shade(&palette::ALL_RAMPS[banner_ramp], 0.8).to_srgba();
        let sign_material = if crate::sigil::gold_reads_on([field.red, field.green, field.blue]) {
            trim.clone()
        } else {
            let dark = crate::sigil::dark_ink();
            materials.add(StandardMaterial {
                base_color: Color::srgb(dark[0], dark[1], dark[2]),
                perceptual_roughness: 0.7,
                ..default()
            })
        };
        for &(x, y, w, h, turn, _round) in crate::sigil::rects(sigil) {
            let unit = 1.05 / 16.0;
            let cx = (x + w * 0.5) / 16.0 - 0.5;
            let cy = (y + h * 0.5) / 16.0 - 0.5;
            for face in [-1.0f32, 1.0] {
                part_rotated(
                    commands,
                    &cube,
                    &sign_material,
                    settlement,
                    Vec3::new(0.75 + cx * 1.05, 3.62 - cy * 1.05, face * 0.05),
                    Quat::from_rotation_z(-turn.to_radians()),
                    Vec3::new(w * unit, h * unit, 0.035),
                );
            }
        }
    }

    // The village fire, a few steps from the banner — on the driest ground
    // around it. Nobody builds their hearth facing the tide.
    let fire_angle = (0..12)
        .map(|step| step as f32 / 12.0 * std::f32::consts::TAU)
        .max_by(|a, b| {
            let dryness = |angle: f32| {
                let (sin, cos) = angle.sin_cos();
                [4.5_f32, 9.0, 14.0]
                    .iter()
                    .map(|reach| terrain.height_at(center.x + cos * reach, center.z + sin * reach))
                    .fold(f32::INFINITY, f32::min)
            };
            dryness(*a).total_cmp(&dryness(*b))
        })
        .unwrap_or(0.0);
    {
        let (sin, cos) = fire_angle.sin_cos();
        let (fx, fz) = (center.x + cos * 4.5, center.z + sin * 4.5);
        let fire_at = Vec3::new(fx, terrain.height_at(fx, fz), fz);
        let fire = home::spawn_bonfire(commands, meshes, materials, fire_at);
        // Whose hearth it is. Each town tends its own fire from its own
        // woodpile, so the fire has to know which town that is.
        commands.entity(fire).insert(MemberOf(settlement));
    }

    // The woodpile, across the square from the fire: every log the village
    // owns, stacked and countable at a glance.
    let woodpile = {
        let (sin, cos) = (fire_angle + std::f32::consts::PI * 0.8).sin_cos();
        let (wx, wz) = (center.x + cos * 5.0, center.z + sin * 5.0);
        let at = Vec3::new(wx, terrain.height_at(wx, wz), wz);

        let log_mesh = meshes.add(Cuboid::new(1.3, 0.2, 0.2));
        let log_material = materials.add(StandardMaterial {
            base_color: palette::shade(&palette::WOOD, 0.42),
            perceptual_roughness: 0.95,
            ..default()
        });
        let pile = commands
            .spawn((
                Name::new("The woodpile"),
                crate::globe::RigidlySeated,
                work::StorePile(work::PileKind::Timber),
                MemberOf(settlement),
                Transform::from_translation(at),
                Visibility::default(),
                crate::hand::PickRadius(1.8),
                crate::hand::Rooted,
            ))
            .id();
        for i in 0..24u8 {
            let layer = i / 4;
            let slot = (i % 4) as f32;
            let across = slot * 0.26 - 0.39;
            let y = 0.12 + layer as f32 * 0.21;
            let (offset, yaw) = if layer % 2 == 0 {
                (Vec3::new(0.0, y, across), 0.0)
            } else {
                (Vec3::new(across, y, 0.0), std::f32::consts::FRAC_PI_2)
            };
            commands.spawn((
                work::WoodpileLog(i),
                Mesh3d(log_mesh.clone()),
                MeshMaterial3d(log_material.clone()),
                Transform::from_translation(offset).with_rotation(Quat::from_rotation_y(yaw)),
                Visibility::Hidden,
                ChildOf(pile),
            ));
        }
        at
    };

    // The stone pile and the food sacks flank the woodpile: the whole
    // larder of the village, standing in the square where it can be seen.
    {
        let (sin, cos) = (fire_angle + std::f32::consts::PI * 0.62).sin_cos();
        let (px_, pz) = (center.x + cos * 6.2, center.z + sin * 6.2);
        let at = Vec3::new(px_, terrain.height_at(px_, pz), pz);
        let block_mesh = meshes.add(Cuboid::new(0.42, 0.34, 0.42));
        let block_material = materials.add(StandardMaterial {
            base_color: palette::shade(&palette::STONE, 0.5),
            perceptual_roughness: 1.0,
            ..default()
        });
        let pile = commands
            .spawn((
                Name::new("The stone pile"),
                crate::globe::RigidlySeated,
                work::StorePile(work::PileKind::Stone),
                MemberOf(settlement),
                Transform::from_translation(at),
                Visibility::default(),
                crate::hand::PickRadius(1.4),
                crate::hand::Rooted,
            ))
            .id();
        for i in 0..12u8 {
            let layer = i / 4;
            let slot = (i % 4) as f32;
            commands.spawn((
                work::StonePileBlock(i),
                Mesh3d(block_mesh.clone()),
                MeshMaterial3d(block_material.clone()),
                Transform::from_xyz(
                    (slot % 2.0) * 0.46 - 0.23 + layer as f32 * 0.08,
                    0.17 + layer as f32 * 0.35,
                    (slot / 2.0).floor() * 0.46 - 0.23,
                ),
                Visibility::Hidden,
                ChildOf(pile),
            ));
        }
    }
    // The clay pile and the ore heap: small faces, but a store the god can
    // haul home deserves to be countable at a glance like the others.
    {
        let (sin, cos) = (fire_angle + std::f32::consts::PI * 1.30).sin_cos();
        let (px_, pz) = (center.x + cos * 6.2, center.z + sin * 6.2);
        let at = Vec3::new(px_, terrain.height_at(px_, pz), pz);
        let brick_mesh = meshes.add(Cuboid::new(0.44, 0.22, 0.3));
        let brick_material = materials.add(StandardMaterial {
            base_color: Color::srgb(0.62, 0.36, 0.24),
            perceptual_roughness: 1.0,
            ..default()
        });
        let pile = commands
            .spawn((
                Name::new("The clay pile"),
                crate::globe::RigidlySeated,
                work::StorePile(work::PileKind::Clay),
                MemberOf(settlement),
                Transform::from_translation(at),
                Visibility::default(),
                crate::hand::PickRadius(1.2),
                crate::hand::Rooted,
            ))
            .id();
        for i in 0..8u8 {
            let layer = i / 4;
            let slot = (i % 4) as f32;
            commands.spawn((
                work::ClayPileBrick(i),
                Mesh3d(brick_mesh.clone()),
                MeshMaterial3d(brick_material.clone()),
                Transform::from_xyz(
                    (slot % 2.0) * 0.48 - 0.24 + layer as f32 * 0.06,
                    0.11 + layer as f32 * 0.23,
                    (slot / 2.0).floor() * 0.34 - 0.17,
                ),
                Visibility::Hidden,
                ChildOf(pile),
            ));
        }
    }
    {
        let (sin, cos) = (fire_angle + std::f32::consts::PI * 1.62).sin_cos();
        let (px_, pz) = (center.x + cos * 6.2, center.z + sin * 6.2);
        let at = Vec3::new(px_, terrain.height_at(px_, pz), pz);
        let chunk_mesh = meshes.add(Cuboid::new(0.36, 0.3, 0.36));
        let chunk_material = materials.add(StandardMaterial {
            base_color: Color::srgb(0.34, 0.27, 0.23),
            perceptual_roughness: 1.0,
            ..default()
        });
        let rust_material = materials.add(StandardMaterial {
            base_color: Color::srgb(0.48, 0.26, 0.14),
            perceptual_roughness: 1.0,
            ..default()
        });
        let pile = commands
            .spawn((
                Name::new("The ore heap"),
                crate::globe::RigidlySeated,
                work::StorePile(work::PileKind::Ore),
                MemberOf(settlement),
                Transform::from_translation(at),
                Visibility::default(),
                crate::hand::PickRadius(1.2),
                crate::hand::Rooted,
            ))
            .id();
        for i in 0..8u8 {
            let layer = i / 4;
            let slot = (i % 4) as f32;
            commands.spawn((
                work::OrePileChunk(i),
                Mesh3d(chunk_mesh.clone()),
                MeshMaterial3d(if i % 3 == 0 {
                    rust_material.clone()
                } else {
                    chunk_material.clone()
                }),
                Transform::from_xyz(
                    (slot % 2.0) * 0.4 - 0.2 + layer as f32 * 0.07,
                    0.15 + layer as f32 * 0.3,
                    (slot / 2.0).floor() * 0.4 - 0.2,
                )
                .with_rotation(Quat::from_rotation_y(i as f32 * 0.7)),
                Visibility::Hidden,
                ChildOf(pile),
            ));
        }
    }
    {
        let (sin, cos) = (fire_angle + std::f32::consts::PI * 0.98).sin_cos();
        let (px_, pz) = (center.x + cos * 6.2, center.z + sin * 6.2);
        let at = Vec3::new(px_, terrain.height_at(px_, pz), pz);
        let sack_mesh = meshes.add(Cuboid::new(0.4, 0.34, 0.4));
        let sack_material = materials.add(StandardMaterial {
            base_color: palette::shade(&palette::BONE, 0.6),
            perceptual_roughness: 1.0,
            ..default()
        });
        let pile = commands
            .spawn((
                Name::new("The food store"),
                crate::globe::RigidlySeated,
                work::StorePile(work::PileKind::Food),
                MemberOf(settlement),
                Transform::from_translation(at),
                Visibility::default(),
                crate::hand::PickRadius(1.4),
                crate::hand::Rooted,
            ))
            .id();
        for i in 0..12u8 {
            let layer = i / 4;
            let slot = (i % 4) as f32;
            commands.spawn((
                work::FoodSack(i),
                Mesh3d(sack_mesh.clone()),
                MeshMaterial3d(sack_material.clone()),
                Transform::from_xyz(
                    (slot % 2.0) * 0.44 - 0.22,
                    0.17 + layer as f32 * 0.33,
                    (slot / 2.0).floor() * 0.44 - 0.22 + layer as f32 * 0.07,
                )
                .with_rotation(Quat::from_rotation_y(i as f32 * 0.4)),
                Visibility::Hidden,
                ChildOf(pile),
            ));
        }
    }

    woodpile
}

/// Founds a town whole: the settlement and its square together.
///
/// The one entry point for a new settlement, wherever it comes from — world
/// generation at the start, or a party of colonists walking out of a crowded
/// village mid-game.
#[allow(clippy::too_many_arguments)]
/// The ground the god chose with the flag. Empty until it is planted,
/// and read once by `spawn_settlement` on the way into `Playing`.
#[derive(Resource, Default)]
pub struct ChosenGround(pub Option<Vec3>);

pub(crate) fn found_settlement(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    terrain: &Terrain,
    center: Vec3,
    name: &str,
    founded: u32,
    banner_ramp: usize,
    sigil: usize,
) -> (Entity, Vec3) {
    let settlement = spawn_town(commands, center, name, founded, banner_ramp, sigil);
    let woodpile = raise_town_fixtures(
        commands,
        meshes,
        materials,
        terrain,
        center,
        settlement,
        banner_ramp,
        sigil,
    );
    commands.entity(settlement).insert(SettlementGround {
        center,
        radius: 36.0,
        woodpile,
        foodpile: woodpile,
    });
    (settlement, woodpile)
}

/// How many quarries a village is given, and the furthest one may sit.
///
/// Closer than iron and far more numerous, and both on purpose. Iron is a
/// reason to explore; stone is an everyday errand that every footing in the
/// civic ladder waits on, so a village that has to walk four hundred units for
/// it simply never builds.
/// FEW and deep, not many and shallow. Six of them within an easy walk put
/// three in one view, and Brett — "they weirdly bought three of them" — read
/// them as things the village had built rather than as places in the land.
/// Three, far enough apart that you come across one at a time, each holding
/// what two of the old ones held.
const QUARRIES: usize = 3;
const QUARRY_NEAREST: f32 = 90.0;
const QUARRY_FURTHEST: f32 = 300.0;
/// How far apart, so working the stone is not one pit emptied — and so that
/// two are never in the frame together.
const QUARRY_APART: f32 = 90.0;

/// The pit a quarry cuts for itself: how deep, how wide, and how sharply the
/// rim comes back to open ground.
///
/// A quarry is a hole with a face. The first cut of this modeled one as
/// geometry standing ON the ground, and since the pieces sat at fixed heights
/// around a single origin, any slope left half of them hanging in the air —
/// Brett: "it looks weird lol". So the ground is worked first and the stone
/// stands on a floor that has been leveled for it.
///
/// A tight falloff, deliberately, against the graded one a building pad uses.
/// [`Terrain::terrace`] describes its own gentle grade as "graded earth rather
/// than a quarry face" — this is the thing that comment is defined against.
const QUARRY_DEPTH: f32 = 2.2;
const QUARRY_PIT: f32 = 6.5;
const QUARRY_RIM: f32 = 4.0;
/// Tries spent insisting on broken ground before any walkable ground will do.
const QUARRY_PATIENCE: usize = 900;
const QUARRY_TRIES: usize = 1_200;

/// Where a village's quarries go: faces of workable stone within an easy
/// errand of the square, on the most broken ground that can be found.
///
/// Pure, and lifted out of the founding so the one thing here that must never
/// fail can be tested across many worlds. A village with no stone cannot raise
/// a single footing, and "the seed was unlucky" is not an answer — see
/// `every_village_is_given_stone_to_build_with`.
///
/// The preference for broken ground is a PREFERENCE. That is the whole
/// difference between this and [`BuildingKind::Mine`], whose siting demands a
/// genuine face to drive a drift into and therefore never finds one in rolling
/// or coastal country — the mine's own comment admits it: "a village on merely
/// rolling country never found any, so it never wanted a mine, and its only
/// stone was the loose boulders, which run out". Those loose boulders are gone
/// now, so this may not have the same hole in it.
/// Where a founding flock may seed, as (nearest, furthest) from the square.
///
/// Prey splits between the view and the wilds: half close enough to be part
/// of the scenery, half out where the hunters have to go find it.
///
/// PREDATORS open past the furthest any trade will range. Seed 4242 dealt a
/// wolf pack inside the old ninety-unit ring and seven of the ten founders
/// were dead before the first hall was framed; three other seeds dealt no
/// close pack and lost nobody, which is the difference between a game and a
/// coin toss. Brett: "we should prevent close wolf spawns in the beginning.
/// It should initially be peaceful." A head start and not a wall - wolves
/// still roam, and hunger still walks them in - but the opening days belong
/// to the village.
pub(crate) fn founding_flock_range(species: Species, flock: usize) -> (f32, f32) {
    match species {
        Species::Wolf => (
            crate::villager::work::RANGE_REACH + 20.0,
            crate::villager::work::RANGE_REACH + 170.0,
        ),
        _ if flock % 2 == 0 => (0.0, 90.0),
        _ => (0.0, 210.0),
    }
}

pub(crate) fn quarry_sites(terrain: &Terrain, center: Vec3, rng: &mut Rng) -> Vec<Vec3> {
    let mut sites: Vec<Vec3> = Vec::new();
    for attempt in 0..QUARRY_TRIES {
        if sites.len() >= QUARRIES {
            break;
        }
        let angle = rng.range(0.0, std::f32::consts::TAU);
        let reach = rng.range(QUARRY_NEAREST, QUARRY_FURTHEST);
        let (sin, cos) = angle.sin_cos();
        let (x, z) = (center.x + cos * reach, center.z + sin * reach);
        if !terrain.is_walkable(x, z) {
            continue;
        }
        let height = terrain.height_at(x, z);
        if height < crate::terrain::WATER_LEVEL + 2.0 {
            continue;
        }
        let at = Vec3::new(x, height, z);
        if sites.iter().any(|q| q.distance(at) < QUARRY_APART) {
            continue;
        }
        // Broken ground while there are tries left to spend looking for it,
        // and anywhere walkable once they run thin.
        //
        // Gated on the TRY and not on how many have been placed. Gated on the
        // count — which is how this was first written — a world with no stony
        // ground anywhere would never place a first quarry, so the relaxation
        // that exists to save exactly that village would never fire: the
        // failure it was written to prevent, written into the thing meant to
        // prevent it.
        if attempt < QUARRY_PATIENCE && terrain.slope_at(x, z) <= 0.18 {
            continue;
        }
        sites.push(at);
    }
    sites
}

pub(crate) fn spawn_settlement(
    mut commands: Commands,
    assets: Res<CreatureAssets>,
    // Bundled: this system is near Bevy's parameter ceiling, and the
    // founding hall needs to level its plot and clear it, which is the
    // same handful of world-building resources every other site uses.
    mut ground: (
        ResMut<Terrain>,
        ResMut<crate::terrain::LoadedChunks>,
        Res<crate::terrain::TerrainAssets>,
        ResMut<crate::scatter::StrippedGround>,
        ResMut<crate::grass::GrassChunks>,
    ),
    woods: Query<(Entity, &GlobalTransform), With<crate::scatter::FellableTree>>,
    world_seed: Res<crate::WorldSeed>,
    clock: Res<crate::calendar::WorldClock>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    // Deposits are ground scenery and wear the veil-carrying material; the
    // buildings and the glory above them do not.
    mut ground_materials: ResMut<Assets<crate::fog::GroundMaterial>>,
    mut notices: MessageWriter<crate::ui::Notice>,
    restoring: Option<Res<RestoringSeed>>,
    chosen: Res<ChosenGround>,
    mut known: ResMut<crate::villager::explore::KnownWorld>,
) {
    let mut rng = Rng::stream(world_seed.0 as u64, "settlement");

    // One language for the whole settlement, so its people sound like a people —
    // kept for life, so its children are named in it too.
    let language = names::Language::random(&mut Rng::stream(world_seed.0 as u64, "language"));

    // Where the god put the flag. A restored world keeps its own ground,
    // and a world that somehow arrived here unplanted falls back on the
    // old search rather than founding nothing.
    let center = restoring
        .as_ref()
        .map(|r| r.center)
        .or(chosen.0)
        .unwrap_or_else(|| choose_settlement_site(&ground.0, &mut rng));
    ground
        .1
        .prime_first_zoom(ground.0.chunk_of(center.x, center.z));

    // The place is named in the same tongue as its people, because the people
    // named it.
    let settlement_name = restoring
        .as_ref()
        .map(|r| r.name.clone())
        .unwrap_or_else(|| language.name(&mut rng));
    // The town's arms: the cloth's color and the sign upon it, rolled at
    // the founding and kept for the rest of its history.
    let (banner_ramp, sigil) = restoring
        .as_ref()
        .and_then(|r| r.banner)
        .unwrap_or_else(|| {
            (
                *rng.pick(palette::CLOTH_RAMPS),
                (rng.next_u32() as usize) % crate::sigil::SIGILS.len(),
            )
        });
    let settlement = spawn_town(
        &mut commands,
        center,
        &settlement_name,
        restoring.as_ref().map_or(clock.day(), |r| r.founded),
        banner_ramp,
        sigil,
    );
    if restoring.is_none() {
        let sign = crate::sigil::name(sigil);
        info!("the village of {settlement_name} was founded under the sign of {sign}");
        notices.write(crate::ui::Notice::fanfare(format!(
            "The village of {settlement_name} is founded, under the sign of {sign}"
        )));

        // The land is salted with what the village will one day want:
        // iron in the far hills, clay along the wet banks. Deliberately
        // out past the home circle — deposits are why explorers matter,
        // and the road to one is a road the village will wear itself.
        let mut placed_iron = 0;
        for _ in 0..400 {
            if placed_iron >= 4 {
                break;
            }
            let angle = rng.range(0.0, std::f32::consts::TAU);
            let reach = rng.range(140.0, 460.0);
            let (sin, cos) = angle.sin_cos();
            let (x, z) = (center.x + cos * reach, center.z + sin * reach);
            if !ground.0.is_walkable(x, z) {
                continue;
            }
            let height = ground.0.height_at(x, z);
            if height > crate::terrain::WATER_LEVEL + 20.0 {
                crate::matter::spawn_deposit(
                    &mut commands,
                    &mut meshes,
                    &mut ground_materials,
                    Vec3::new(x, height, z),
                    crate::matter::DepositKind::Iron,
                    rng.range(16.0, 26.0),
                );
                placed_iron += 1;
            }
        }

        // Clay is a creature of the waterline: wherever the coast runs,
        // the wet banks along it are rich with it — strung out with a
        // little space between banks, so working the clay means walking
        // the shore rather than emptying one pit.
        let mut clay_banks: Vec<Vec3> = Vec::new();
        for _ in 0..900 {
            if clay_banks.len() >= 10 {
                break;
            }
            let angle = rng.range(0.0, std::f32::consts::TAU);
            let reach = rng.range(40.0, 700.0);
            let (sin, cos) = angle.sin_cos();
            let (x, z) = (center.x + cos * reach, center.z + sin * reach);
            if !ground.0.is_walkable(x, z) {
                continue;
            }
            let height = ground.0.height_at(x, z);
            if !(crate::terrain::WATER_LEVEL + 0.5..crate::terrain::WATER_LEVEL + 3.5)
                .contains(&height)
            {
                continue;
            }
            let at = Vec3::new(x, height, z);
            if clay_banks.iter().any(|b| b.distance(at) < 25.0) {
                continue;
            }
            crate::matter::spawn_deposit(
                &mut commands,
                &mut meshes,
                &mut ground_materials,
                at,
                crate::matter::DepositKind::Clay,
                rng.range(24.0, 40.0),
            );
            clay_banks.push(at);
        }
        // And the quarries, which are where the village's stone comes from
        // now that the ground is not strewn with it.
        //
        // CLOSER than iron and far more numerous, and both on purpose. Iron is
        // a reason to explore; stone is an everyday errand that every footing
        // in the civic ladder waits on, so a village that has to walk four
        // hundred units for it simply never builds. The nearest ring starts
        // just outside the home circle - far enough that the square is not a
        // building site, near enough that a miner is back before noon.
        //
        // Sited on the most BROKEN ground each try can find, so a quarry
        // stands where a quarry would: a bank, a bluff, a stony shoulder. That
        // is a preference and not a requirement, which is the whole difference
        // between this and the mine - see `BuildingKind::Mine`, whose siting
        // wants a genuine face and so never finds one in rolling or coastal
        // country. A village on a flat green shore gets its quarry regardless,
        // because the alternative is a village that cannot build.
        let quarries = quarry_sites(&ground.0, center, &mut rng);
        for at in &quarries {
            // The pit first, then the stone standing in it. See `QUARRY_DEPTH`
            // for why the ground is worked rather than the model made taller.
            // `flatten` leaves the floor BARE, which is what a working quarry
            // is, and keeps the scatter from seeding trees in the middle of it.
            let floor = at.y - QUARRY_DEPTH;
            ground.0.flatten(at.x, at.z, QUARRY_PIT, QUARRY_RIM, floor);
            crate::matter::spawn_deposit(
                &mut commands,
                &mut meshes,
                &mut ground_materials,
                Vec3::new(at.x, floor, at.z),
                crate::matter::DepositKind::Stone,
                // Deeper, because there are half as many. A village's whole
                // civic ladder is under a hundred stone; one of these is
                // several ladders, and running dry is the failure mode that
                // matters - see `every_village_is_given_stone_to_build_with`.
                rng.range(200.0, 300.0),
            );
        }
        // Any chunk already standing was built before the pits were cut, so it
        // still shows unbroken ground with a quarry sitting on top of it.
        for at in &quarries {
            crate::terrain::rebuild_chunks_near(
                &mut commands,
                &mut meshes,
                &ground.2,
                &ground.0,
                &mut ground.1,
                at.x,
                at.z,
                QUARRY_PIT + QUARRY_RIM,
            );
        }
        info!(
            "the land holds {placed_iron} iron veins, {} clay banks and {} quarries",
            clay_banks.len(),
            quarries.len()
        );
    }

    // The god is named by its people, in their own tongue - the player
    // never picks it, because being named is the first thing belief does
    // to you. And it happens when they have SEEN something to name you
    // for, not at the founding: a village that names a god on the
    // morning it arrives has named it for nothing. See `name_the_god`.
    //
    // A restored world already knows what it calls you.
    if let Some(known) = restoring.as_ref().map(|r| r.god.clone()) {
        commands.insert_resource(DivineName(known));
    }

    // The square itself, raised by the same code that raises a colony's:
    // banner, fire, woodpile, stone pile, food sacks.
    let woodpile = raise_town_fixtures(
        &mut commands,
        &mut meshes,
        &mut materials,
        &ground.0,
        center,
        settlement,
        banner_ramp,
        sigil,
    );

    let site = SettlementSite {
        center,
        radius: 36.0,
        woodpile,
        settlement,
    };
    // The village knows the ground it stands on, and the veil lifts from THERE.
    //
    // `KnownWorld::center` was never written by anybody: it defaulted to the
    // world's origin and stayed there, so the fog cleared over the middle of the
    // map however far away the flag was actually planted. Brett: "if you go to
    // the other side of the world to plant your flag it works but the fog of war
    // doesnt clear around the settlement." Near the origin the two happened to
    // be the same place, which is why it took a walk to the far side to see.
    known.center = center;
    // The same ground, on the town itself. One source of truth: the resource
    // says which town the player is looking at, the component says where that
    // town is — and every other town carries its own.
    commands.entity(settlement).insert(site.ground());
    let day = clock.day();

    // The hall the god raised, and the doorstep the founders walk out
    // of. Only for a new world: a restored one already has its own.
    let doorstep = restoring.is_none().then(|| {
        let standing: Vec<(Entity, Vec3)> = woods
            .iter()
            .map(|(tree, at)| (tree, at.translation()))
            .collect();
        let (terrain, chunks, chunk_assets, stripped, grass) = &mut ground;
        work::raise_the_founding_hall(
            &mut commands,
            &mut meshes,
            &mut materials,
            terrain,
            chunks,
            chunk_assets,
            stripped,
            grass,
            &standing,
            settlement,
            center,
            &mut rng,
        )
    });
    let doorstep = doorstep.flatten();
    // The plot is leveled and cleared by now; everything below reads the
    // land as it finally stands.
    let terrain = &ground.0;

    // The light. It comes down BEFORE anything stands in it - that
    // ordering is what makes the founding read as an act by someone
    // rather than a building with a lamp on it.
    if restoring.is_none() {
        crate::miracles::spawn_glory(
            &mut commands,
            &mut meshes,
            &mut materials,
            center,
            palette::shade(&palette::CLOTH_GOLD, 0.95),
            true,
        );
    }

    let founders = if restoring.is_some() {
        0
    } else {
        STARTING_POPULATION
    };
    // Kept as they spawn, so the founding can wed two pairs of them below.
    let mut first_families: Vec<(Entity, Person)> = Vec::with_capacity(founders);
    for i in 0..founders {
        // Out of the hall's door, one after another, rather than
        // scattered across a field they were never in.
        let position = match doorstep {
            Some(step) => {
                let turn = i as f32 * 2.399_963;
                let out = Vec3::new(turn.cos(), 0.0, turn.sin()) * (1.2 + i as f32 * 0.35);
                let spot = step + out;
                Vec3::new(spot.x, terrain.height_at(spot.x, spot.z), spot.z)
            }
            None => random_walkable_point(&terrain, &mut rng, center, site.radius * 0.6)
                .unwrap_or(center),
        };
        // Five and five, all grown. A founding generation with children
        // in it is a founding generation that cannot work: a quarter of
        // the world's first hands used to be too small to lift anything,
        // and the village spent its first week learning that.
        let sex = if i % 2 == 0 { Sex::Female } else { Sex::Male };
        let genome = CreatureGenome::adult(Species::Human, sex, &mut rng);
        let is_child = genome.age == Age::Child;
        let genome_sex = genome.sex;
        let genome_age = genome.age;

        let entity = spawn_creature(
            &mut commands,
            &assets,
            genome,
            position,
            rng.range(0.0, std::f32::consts::TAU),
            i as f32 * 0.618,
        );

        // Founders are strangers to one another: twelve people, twelve
        // houses. Paternal descent will thin that down over the
        // generations on its own — the names that survive are the ones
        // that had sons, which is how a village ends up with a handful
        // of old families and a lot of forgotten ones.
        let person = Person::born(
            language.name_for(genome_sex, &mut rng),
            language.surname(&mut rng),
        );
        first_families.push((entity, person.clone()));
        commands.entity(entity).insert((
            Villager,
            traits::Traits::roll(&mut rng),
            person,
            crate::witness::Temperament::random(&mut rng),
            crate::witness::Witnessed::default(),
            speech::RecentlySaid::default(),
            Needs {
                // Stagger starting hunger so the settlement does not arrive at the
                // food crisis in lockstep.
                hunger: rng.range(0.0, 0.3),
                ..default()
            },
            Morale::default(),
            Activity::Idle,
            MemberOf(settlement),
        ));

        // Every life opens with how it came to this place. The first villager
        // is the founder; the rest arrived with them.
        let mut chronicle = Chronicle::default();
        chronicle.record(
            day,
            if i == 0 {
                format!("founded {settlement_name} beside the water")
            } else {
                format!("settled {settlement_name} among the first families")
            },
        );
        commands.entity(entity).insert(chronicle);

        // Founding children are part-grown: they come of age at staggered times
        // rather than as a single graduating class. Founding adults are at
        // staggered points of their prime, so the village grays gradually.
        if is_child {
            commands.entity(entity).insert(Childhood {
                remaining: rng.range(SECONDS_TO_COME_OF_AGE * 0.2, SECONDS_TO_COME_OF_AGE),
            });
        } else if genome_age == Age::Adult {
            commands.entity(entity).insert(Prime {
                remaining: rng.range(SECONDS_OF_PRIME * 0.3, SECONDS_OF_PRIME),
            });
        }
    }

    // Two of the pairs came to the valley already wed. Ten strangers read
    // as an expedition; two households and six single hands read as a
    // MIGRATION - and a founding with families in it wants houses at once
    // and starts having children a season sooner. Brett: "have two couples
    // that are already married at game start." The founders alternate
    // woman and man out of the door, so (0,1) and (2,3) are the pairs; the
    // effects are the wedding rite's own - spouse both ways, her name
    // follows his house, hearts already warm - minus the ceremony, which
    // happened somewhere back down the road they came in on.
    if restoring.is_none() && first_families.len() >= 4 {
        let day = clock.day();
        for (wife_at, husband_at) in [(0usize, 1usize), (2, 3)] {
            let (wife, her) = first_families[wife_at].clone();
            let (husband, him) = first_families[husband_at].clone();
            commands.entity(wife).insert(Spouse(husband));
            commands.entity(husband).insert(Spouse(wife));
            // She takes his house; her born name keeps the maternal line.
            let married = Person {
                name: her.name.clone(),
                surname: him.surname.clone(),
                born_surname: her.born_surname.clone(),
            };
            commands.entity(wife).insert(married.clone());
            // Chronicles rebuilt whole: the arrival line each founder
            // already carries, and the marriage that predates the banner.
            let mut hers = Chronicle::default();
            hers.record(
                day,
                if wife_at == 0 {
                    format!("founded {settlement_name} beside the water")
                } else {
                    format!("settled {settlement_name} among the first families")
                },
            );
            hers.record(
                day,
                format!("came to the valley already wed to {}", him.full_name()),
            );
            commands.entity(wife).insert(hers);
            let mut his = Chronicle::default();
            his.record(
                day,
                format!("settled {settlement_name} among the first families"),
            );
            his.record(
                day,
                format!("came to the valley already wed to {}", married.full_name()),
            );
            commands.entity(husband).insert(his);
            // Not the rite's day-of spark - the worn-in devotion of a
            // marriage that predates the road. Near the ceiling, so the
            // pair reads as LOVE from the first glance at the dossier and
            // stays devoted against the heart's slow cooling.
            let mut her_heart = regard::Regard::default();
            her_heart.warm(husband, 0.95);
            commands.entity(wife).insert(her_heart);
            let mut his_heart = regard::Regard::default();
            his_heart.warm(wife, 0.95);
            commands.entity(husband).insert(his_heart);
            info!(
                "{} and {} came to the valley already wed",
                married.full_name(),
                him.full_name()
            );
        }
    }

    // Wildlife, so the world is not only people. Prey outnumbers wolves
    // heavily — the first ecology build spawned one wolf for every two deer
    // and the wolves ate the entire wilderness inside a morning.
    let mut wildlife_rng = Rng::stream(world_seed.0 as u64, "wildlife");
    let flocks = if restoring.is_some() { 0 } else { 18 };
    for flock in 0..flocks {
        let species = *wildlife_rng.pick(&[
            Species::Deer,
            Species::Deer,
            Species::Deer,
            Species::Deer,
            Species::Boar,
            Species::Boar,
            Species::Wolf,
        ]);
        let (nearest, reach) = founding_flock_range(species, flock);
        let Some(position) = crate::creature::random_walkable_ring(
            &terrain,
            &mut wildlife_rng,
            center,
            nearest,
            reach,
        ) else {
            continue;
        };
        let genome = CreatureGenome::random(species, &mut wildlife_rng);
        let entity = spawn_creature(
            &mut commands,
            &assets,
            genome,
            position,
            wildlife_rng.range(0.0, std::f32::consts::TAU),
            wildlife_rng.f32() * 6.0,
        );
        commands.entity(entity).insert((
            Activity::Idle,
            crate::creature::wildlife::Wild {
                // Staggered appetites, so the wilderness does not hunt in unison.
                hunger: wildlife_rng.range(0.0, 0.6),
                busy: 0.0,
                home: position,
            },
        ));
    }

    commands.insert_resource(explore::KnownWorld {
        center,
        radius: 170.0,
        pockets: Vec::new(),
    });
    commands.insert_resource(site);
    commands.insert_resource(SettlementCulture { language });
}

/// The first person to read a hand into what they saw gives it a name.
///
/// Every read of `DivineName` already took an `Option` and fell back to
/// "the god", so a nameless god costs nothing and always did - it was
/// simply never absent, because the founding handed one out before
/// anybody had witnessed anything at all.
///
/// A skeptic's memory does not count. The verdict is rolled per witness
/// and per act (`Memory::divine`), and somebody who decided it was the
/// weather has not met a god to name.
pub(crate) fn name_the_god(
    mut commands: Commands,
    clock: Res<crate::calendar::WorldClock>,
    culture: Option<Res<SettlementCulture>>,
    world_seed: Res<crate::WorldSeed>,
    mut notices: MessageWriter<crate::ui::Notice>,
    settlements: Query<&Settlement>,
    mut believers: Query<(&Person, &crate::witness::Witnessed, Option<&mut Chronicle>)>,
) {
    let Some(culture) = culture else {
        return;
    };
    let Some((person, chronicle)) = believers.iter_mut().find_map(|(who, seen, chronicle)| {
        seen.recent
            .iter()
            .any(|memory| memory.divine)
            .then_some((who, chronicle))
    }) else {
        return;
    };
    let mut rng = Rng::stream(world_seed.0 as u64, "divine name");
    let named = culture.language.name(&mut rng);
    let home = settlements
        .iter()
        .next()
        .map(|s| s.name.clone())
        .unwrap_or_default();
    info!(
        "{} saw the hand of something, and in {home} they name it {named}",
        person.name
    );
    notices.write(crate::ui::Notice::fanfare(format!(
        "In {home}, they name their god {named}"
    )));
    if let Some(mut chronicle) = chronicle {
        chronicle.record(
            clock.day(),
            format!("was the first to name the god {named}"),
        );
    }
    commands.insert_resource(DivineName(named));
}

/// The simulation's dice, from the first frame of the world.
///
/// These used to be handed out by the founding, which was fine while the
/// founding happened at startup. Now that the world opens empty and waits
/// on a flag, every system that rolls anything would be reaching for dice
/// that do not exist yet - and they are not the village's dice anyway,
/// they are the world's.
fn deal_the_dice(mut commands: Commands, world_seed: Res<crate::WorldSeed>) {
    commands.insert_resource(SimRng(Rng::stream(world_seed.0 as u64, "behavior")));
}

/// Points the camera at the settlement once it has been placed.
///
/// The settlement site is chosen from the terrain, so it is not known until after
/// generation. Starting the camera at the world origin instead leaves the player
/// staring at an empty hillside with the entire population off-screen.
/// The settlement's official radius follows its furthest roof: wandering,
/// sheltering and the social range all read this, so a sprawling town
/// LIVES sprawling instead of crowding its old center.
fn stretch_settlement(
    time: Res<Time>,
    mut since_last: Local<f32>,
    mut site: Option<ResMut<SettlementSite>>,
    mut towns: Query<(Entity, &mut SettlementGround)>,
    buildings: Query<(&GlobalTransform, &MemberOf), With<work::Building>>,
) {
    *since_last += time.delta_secs();
    if *since_last < 12.0 {
        return;
    }
    *since_last = 0.0;
    // Each town's working ground grows with its OWN build-out. A far-flung
    // mine in one settlement should not stretch another settlement's reach.
    for (town, mut ground) in &mut towns {
        let furthest = buildings
            .iter()
            .filter(|(_, member)| member.0 == town)
            .map(|(at, _)| at.translation().distance(ground.center))
            .fold(0.0f32, f32::max);
        let stretched = (furthest + 14.0).max(36.0);
        if (stretched - ground.radius).abs() > 1.0 {
            ground.radius = stretched;
            // Keep the interface pointer in step for the town on screen.
            if let Some(site) = site.as_mut()
                && site.settlement == town
            {
                site.radius = stretched;
            }
        }
    }
}

fn point_camera_at_settlement(
    site: Option<Res<SettlementSite>>,
    terrain_probe: Option<Res<Terrain>>,
    mut rigs: Query<&mut crate::camera::CameraRig>,
) {
    let (Some(site), Ok(mut rig)) = (site, rigs.single_mut()) else {
        return;
    };

    rig.focus = site.center;
    rig.target_focus = site.center;

    // Close enough that individual villagers are legible, far enough to see the
    // ground they are working.
    rig.distance = 80.0;
    rig.target_distance = 80.0;

    // Capture tooling: DIVUS_FACTUS_DISTANCE overrides the zoom.
    if let Some(distance) = std::env::var("DIVUS_FACTUS_DISTANCE")
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
    {
        rig.distance = distance;
        rig.target_distance = distance;
    }

    // Capture tooling: DIVUS_FACTUS_PITCH flattens the camera to photograph the
    // horizon and sky instead of the ground.
    if let Some(pitch) = std::env::var("DIVUS_FACTUS_PITCH")
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
    {
        rig.pitch = pitch;
        rig.target_pitch = pitch;
    }

    // Capture tooling: DIVUS_FACTUS_YAW turns the camera to a bearing, in
    // radians. Its companion above can raise the eye to the sky, but until
    // this there was no way to choose WHICH piece of sky - and an unattended
    // run cannot look for the sun any more than it can reach the mouse wheel.
    if let Some(yaw) = std::env::var("DIVUS_FACTUS_YAW")
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
    {
        rig.yaw = yaw;
        rig.target_yaw = yaw;
    }

    // Capture tooling: DIVUS_FACTUS_AIM_RIVER points the unattended screenshot at the
    // nearest river instead of the settlement.
    if crate::capture_path().is_some()
        && std::env::var("DIVUS_FACTUS_AIM_RIVER").is_ok()
        && let Some(t) = terrain_probe.as_deref()
    {
        'search: for iz in -60..60 {
            for ix in -60..60 {
                let x = ix as f32 * 40.0;
                let z = iz as f32 * 40.0;
                if t.river_surface_at(x, z)
                    .is_some_and(|s| s > crate::terrain::WATER_LEVEL + 6.0)
                {
                    rig.focus = Vec3::new(x, 0.0, z);
                    rig.target_focus = rig.focus;
                    rig.distance = 110.0;
                    rig.target_distance = 110.0;
                    rig.pitch = 0.5;
                    rig.target_pitch = 0.5;
                    break 'search;
                }
            }
        }
    }
}

fn accumulate_hunger(
    time: Res<Time>,
    mut villagers: Query<(
        &mut Needs,
        &mut crate::creature::Vitality,
        Option<&Activity>,
    )>,
    watch: Res<crate::debug::timings::Timings>,
) {
    let _t = watch.watch("villager: accumulate_hunger");
    let dt = time.delta_secs();
    let rate = dt / SECONDS_TO_STARVE;

    for (mut needs, mut vitality, activity) in &mut villagers {
        // A sleeping body burns slow: the night costs a quarter of what
        // the waking day does, so bedding down part-fed is a sound plan
        // rather than a gamble.
        let rate = if matches!(activity, Some(Activity::Sleeping)) {
            rate * 0.25
        } else {
            rate
        };
        needs.hunger = (needs.hunger + rate).min(1.0);

        // An empty stomach is survivable; an empty stomach that stays empty is not.
        // Food both stops the dying and slowly undoes it — including harm that came
        // from being thrown, because rest mends what it can.
        if needs.hunger >= 0.99 {
            vitality.harm = (vitality.harm + dt / SECONDS_STARVING_TO_DIE).min(1.0);
        } else if needs.hunger < 0.5 {
            vitality.harm = (vitality.harm - dt / SECONDS_TO_MEND).max(0.0);
            if vitality.harm == 0.0 {
                vitality.violent = false;
            }
        }
    }
}

/// The forensics that run BEFORE the funeral: anyone at the edge of
/// starving logs who they are, what they are doing, and how far the
/// banner stands, every few seconds. When a death notice reads like
/// nonsense - "starved within sight of a stocked larder" - this is the
/// tape to rewind to see exactly which state held them while they died.
fn starvation_watch(
    time: Res<Time>,
    mut since: Local<f32>,
    site: Option<Res<SettlementSite>>,
    towns: Query<(&SettlementGround, &work::Stockpile)>,
    watchers: Query<
        (
            &Person,
            &Needs,
            &Activity,
            &Transform,
            Option<&MemberOf>,
            Option<&crate::creature::MoveTarget>,
        ),
        (With<Villager>, Without<crate::creature::Corpse>),
    >,
    watch: Res<crate::debug::timings::Timings>,
) {
    let _t = watch.watch("villager: starvation_watch");
    *since += time.delta_secs();
    if *since < 8.0 {
        return;
    }
    *since = 0.0;
    for (person, needs, activity, at, member, target) in &watchers {
        if needs.hunger < 0.95 {
            continue;
        }
        let from_home = site
            .as_ref()
            .map_or(f32::NAN, |s| s.center.distance(at.translation));
        // The banner is not the meal. Twice now a village has starved
        // beside a full larder because the SACKS were somewhere the
        // walk could not finish, so the watch reads out the table too:
        // how far it stands, what it holds, and whether the walk is
        // even aimed at it.
        let table = member
            .map(|m| m.0)
            .or_else(|| site.as_ref().map(|s| s.settlement))
            .and_then(|town| towns.get(town).ok());
        let (to_table, larder, aimed) = table.map_or((f32::NAN, f32::NAN, f32::NAN), |(g, s)| {
            (
                g.foodpile.distance(at.translation),
                s.food(),
                target
                    .and_then(|t| t.0)
                    .map_or(f32::NAN, |goal| goal.distance(g.foodpile)),
            )
        });
        info!(
            "starvation watch: {} is {:?} at hunger {:.2}, {:.0} strides from the banner, \
             {:.1} from the sacks (larder {larder:.0}, walking to a point {aimed:.1} off them)",
            person.name, activity, needs.hunger, from_home, to_table
        );
    }
}

/// Whether the village is doing well enough to grow.
/// Whether the village can bear another child, against the current roof —
/// the town hall raises it.
/// How full the village's bellies must be, on average, for it to grow.
pub const GROWTH_HUNGER: f32 = 0.55;

/// And how many meals it must have put by, per head.
pub const GROWTH_LARDER: f32 = 1.5;

/// Why the village is or is not growing, for the dev overlay. Written every
/// time `births` wakes, whether or not a child comes of it.
pub static GROWTH_THINKING: std::sync::RwLock<String> = std::sync::RwLock::new(String::new());

fn can_grow_to(living: usize, average_hunger: f32, food_stored: f32, cap: usize) -> bool {
    // Births need SURPLUS, not merely absence of famine: a child arrives
    // into a larder holding at least a meal and a half per head, or the
    // village overshoots its food supply and starves at the cap - growth
    // constrained by hunger before hunger ever kills.
    living >= 2
        && living < cap
        && average_hunger < GROWTH_HUNGER
        && food_stored >= living as f32 * GROWTH_LARDER
}

/// Brings children into fed villages.
///
/// Parents are two living adults; the child mixes their genomes, which is what the
/// heritability in [`CreatureGenome::child_of`] has been waiting for since the
/// first day — a family you can recognize across the village green.
/// Children grow up.
///
/// Coming of age rebuilds the body from the same genome at adult stage — the
/// child and the adult are recognizably the same person because they *are* the
/// same numbers, just grown into.
fn grow_up(
    mut commands: Commands,
    time: Res<Time>,
    clock: Res<crate::calendar::WorldClock>,
    assets: Res<CreatureAssets>,
    mut rng: Option<ResMut<SimRng>>,
    mut notices: MessageWriter<crate::ui::Notice>,
    mut children: Query<
        (
            Entity,
            &mut CreatureGenome,
            &mut Childhood,
            &Person,
            Option<&mut Chronicle>,
        ),
        Without<crate::creature::Corpse>,
    >,
    watch: Res<crate::debug::timings::Timings>,
) {
    let _t = watch.watch("villager: grow_up");
    let Some(rng) = rng.as_mut() else {
        return;
    };

    for (entity, mut genome, mut childhood, person, chronicle) in &mut children {
        childhood.remaining -= time.delta_secs();
        if childhood.remaining > 0.0 {
            continue;
        }

        genome.age = Age::Adult;
        // Adulthood re-rolls what childhood suppressed.
        genome.beard = genome.sex == Sex::Male && rng.0.chance(0.45);
        genome.satchel = rng.0.chance(0.25);
        genome.proportions.leg_length += 0.04;

        info!("{} came of age", person.name);
        notices.write(crate::ui::Notice::new(format!(
            "{} came of age",
            person.name
        )));
        if let Some(mut chronicle) = chronicle {
            chronicle.record(clock.day(), "came of age");
        }

        // Tear the child's body down and raise the adult's. The despawn command
        // applies before the spawns queued by `build_body`, so the new limbs
        // survive the demolition.
        commands.entity(entity).despawn_related::<Children>();
        let rig = build_body(&mut commands, &assets, entity, &genome);
        commands.entity(entity).remove::<Childhood>().insert((
            rig,
            crate::hand::PickRadius(genome.height() * 0.45),
            Prime {
                remaining: SECONDS_OF_PRIME,
            },
        ));
    }
}

/// Age comes for everyone: the prime ends, the hair grays, the back bends.
///
/// No one dies of it — yet. Growing old is a visible season of life, not a
/// timer on it; death still has to arrive by hunger, violence or misadventure.
fn grow_old(
    mut commands: Commands,
    time: Res<Time>,
    clock: Res<crate::calendar::WorldClock>,
    assets: Res<CreatureAssets>,
    mut notices: MessageWriter<crate::ui::Notice>,
    mut aging: Query<
        (
            Entity,
            &mut CreatureGenome,
            &mut Prime,
            &Person,
            Option<&mut Chronicle>,
        ),
        Without<crate::creature::Corpse>,
    >,
    watch: Res<crate::debug::timings::Timings>,
) {
    let _t = watch.watch("villager: grow_old");
    for (entity, mut genome, mut prime, person, chronicle) in &mut aging {
        prime.remaining -= time.delta_secs();
        if prime.remaining > 0.0 {
            continue;
        }

        genome.age = Age::Elder;
        genome.hair = crate::creature::genome::Tone {
            ramp: palette::RAMP_BONE,
            step: 3,
        };
        genome.gait.lean += 0.14;

        info!("{} has grown old", person.name);
        notices.write(crate::ui::Notice::new(format!(
            "{} has grown old",
            person.name
        )));
        if let Some(mut chronicle) = chronicle {
            chronicle.record(clock.day(), "grew old, and gray");
        }

        commands.entity(entity).despawn_related::<Children>();
        let rig = build_body(&mut commands, &assets, entity, &genome);
        commands
            .entity(entity)
            .remove::<Prime>()
            .insert((rig, crate::hand::PickRadius(genome.height() * 0.45)));
    }
}

/// Death enters the record: the dead close their own story, and a surviving
/// spouse's story gains its worst line.
fn bereave(
    clock: Res<crate::calendar::WorldClock>,
    site: Option<Res<SettlementSite>>,
    stores: Query<&work::Stockpile>,
    bushes: Query<(&GlobalTransform, &crate::scatter::FoodSource)>,
    mut notices: MessageWriter<crate::ui::Notice>,
    mut the_dead: Query<
        (
            Entity,
            &Person,
            &Transform,
            Option<&crate::creature::Vitality>,
            Option<&mut Chronicle>,
        ),
        Added<crate::creature::Corpse>,
    >,
    mut survivors: Query<(&Spouse, &mut Chronicle), Without<crate::creature::Corpse>>,
) {
    let day = clock.day();
    for (dead, person, at, vitality, chronicle) in &mut the_dead {
        // A starvation is a story with a cause, and the cause is written
        // into the record: an empty larder, a picked-bare land, or a road
        // home that was simply too long. The Dwarf Fortress rule - you
        // should always be able to read WHY.
        let starved_because = || {
            let larder = site
                .as_ref()
                .and_then(|s| stores.get(s.settlement).ok())
                .map_or(0.0, |s| s.food());
            let from_home = site
                .as_ref()
                .map_or(0.0, |s| at.translation.distance(s.center));
            let nearest_berries = bushes
                .iter()
                .filter(|(_, bush)| bush.amount > 0.2)
                .map(|(t, _)| crate::globe::unbend(t.translation()).distance(at.translation) as u32)
                .min();
            if larder >= 1.0 && from_home > 80.0 {
                format!(
                    "{} starved on the road, {:.0} strides from a larder that held food",
                    person.name, from_home
                )
            } else if larder >= 1.0 {
                format!("{} starved within sight of a stocked larder", person.name)
            } else {
                match nearest_berries {
                    Some(d) if d > 60 => format!(
                        "{} starved - the larder was empty and the nearest berries stood {d} strides away",
                        person.name
                    ),
                    Some(_) => format!("{} starved beside an empty larder", person.name),
                    None => format!(
                        "{} starved - the larder was empty and the land picked bare",
                        person.name
                    ),
                }
            }
        };
        notices.write(crate::ui::Notice::new(match vitality {
            Some(v) if v.violent => format!("{} {}", person.name, v.undoing.how()),
            _ => starved_because(),
        }));
        if let Some(mut chronicle) = chronicle {
            chronicle.record(
                day,
                match vitality {
                    Some(v) if v.violent => v.undoing.how().to_string(),
                    _ => starved_because()
                        .replace(&format!("{} ", person.name), "")
                        .replacen("starved", "starved:", 1),
                },
            );
        }
        for (spouse, mut chronicle) in &mut survivors {
            if spouse.0 == dead {
                chronicle.record(day, format!("was widowed of {}", person.name));
            }
        }
    }
}

/// The god's touch enters personal history.
///
/// [`crate::witness::Witnessed`] records what people *saw*; this records what
/// happened *to them*. The difference is the seed of two different theologies.
fn chronicle_divine_touch(
    clock: Res<crate::calendar::WorldClock>,
    mut events: MessageReader<crate::witness::DivineEvent>,
    mut chronicles: Query<&mut Chronicle>,
) {
    for event in events.read() {
        let Some(subject) = event.subject else {
            continue;
        };
        let Ok(mut chronicle) = chronicles.get_mut(subject) else {
            continue;
        };
        let text = match event.kind {
            crate::witness::DivineEventKind::Lifted => "was lifted into the sky by the hand of god",
            crate::witness::DivineEventKind::Thrown => {
                "was hurled across the land by the hand of god"
            }
            crate::witness::DivineEventKind::SetDown => "was set down gently by the hand of god",
            crate::witness::DivineEventKind::Impact => "struck the earth, and lived",
            crate::witness::DivineEventKind::Provided => "was given food by the hand of god",
            crate::witness::DivineEventKind::Smote => "was struck by the god's lightning",
            crate::witness::DivineEventKind::Uprooted => "was torn from the earth",
            crate::witness::DivineEventKind::Mended => "was made whole by the hand of god",
            crate::witness::DivineEventKind::Quaked => "was thrown down when the earth buckled",
            crate::witness::DivineEventKind::Mauled => "was set upon by a wolf, and got home",
            // The worldly turns write their own chronicle lines at their own
            // sites (the death, the birth, the harvest); nothing to add here
            // - and the placeless wonders (rain, the beacon, the falling
            // stone, the sown doubt) have no single subject to write for.
            crate::witness::DivineEventKind::Perished
            | crate::witness::DivineEventKind::Delivered
            | crate::witness::DivineEventKind::Flourished
            | crate::witness::DivineEventKind::Rained
            | crate::witness::DivineEventKind::Beckoned
            | crate::witness::DivineEventKind::Fell
            | crate::witness::DivineEventKind::DoubtSown => continue,
        };
        chronicle.record(clock.day(), text);
    }
}

/// The season of recovery and nursing after a birth: while it runs, this
/// mother bears no second child. Without it, one prolific couple filled a
/// year with twenty-three children off an even dice pick.
/// How many children a woman has borne. Counted rather than derived from
/// living children, because the dead become graves and a mother's history
/// should not be rewritten by losing them.
#[derive(Component, Debug, Default, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Motherhood {
    pub borne: u32,
}

/// A couple's chance of another child, falling with each they already have.
///
/// Not a hard cap: a large family stays *possible*, it just stops being the
/// likely thing. Each birth takes roughly a third off the odds, so the first
/// few children come readily and the seventh is a rarity worth remarking on.
pub fn fertility(borne: u32) -> f32 {
    0.68_f32.powi(borne as i32)
}

/// How long a mother nurses before she may bear again.
///
/// Half a season. It was twenty-four days - most of one - and with the four
/// or five mothers a young village has, that shut the nursery more often
/// than it left it open, which is a good part of why a town would sit at
/// seventeen for years. Brett: "Maybe the cooldown for a new child could be
/// reduced to 14 as well."
pub const NURSING_DAYS: f64 = 14.0;

#[derive(Component)]
pub struct NewMother {
    pub until: f64,
}

fn births(
    mut commands: Commands,
    time: Res<Time>,
    clock: Res<crate::calendar::WorldClock>,
    mut since_last: Local<f32>,
    assets: Res<CreatureAssets>,
    terrain: Res<Terrain>,
    culture: Option<Res<SettlementCulture>>,
    site: Option<Res<SettlementSite>>,
    mut chronicles: Query<&mut Chronicle>,
    // Bundled: this system sits at Bevy's parameter ceiling.
    mut notices: (
        MessageWriter<crate::ui::Notice>,
        MessageWriter<crate::witness::DivineEvent>,
    ),
    town_halls: Query<&work::Building>,
    shelter: (
        Query<(), With<work::Hut>>,
        Query<(), With<work::Longhouse>>,
        Query<&work::Stockpile>,
        Query<&Motherhood>,
        Query<&regard::Regard>,
    ),
    mut rng: ResMut<SimRng>,
    villagers: Query<
        (
            Entity,
            &Transform,
            &CreatureGenome,
            &Needs,
            &Person,
            Option<&Spouse>,
            Option<&NewMother>,
            Option<&MemberOf>,
        ),
        (With<Villager>, Without<crate::creature::Corpse>),
    >,
    watch: Res<crate::debug::timings::Timings>,
) {
    let _t = watch.watch("villager: births");
    *since_last += time.delta_secs();
    if *since_last < BIRTH_INTERVAL {
        return;
    }
    *since_last = 0.0;

    let Some(culture) = culture else {
        return;
    };
    let (huts, longhouses, stores, borne, hearts) = shelter;

    let living = villagers.iter().count();
    let average_hunger = if living == 0 {
        1.0
    } else {
        villagers
            .iter()
            .map(|(_, _, _, n, ..)| n.hunger)
            .sum::<f32>()
            / living as f32
    };
    // Nobody is born into a village that cannot shelter them: houses are
    // the roof on growth - and a town hall raises it further, the way its
    // own line always promised. (This query was taken and thrown away for
    // an age: the hall was a building that said "room for more people"
    // and did nothing. It does it now.)
    let halls = town_halls
        .iter()
        .filter(|b| b.kind == work::BuildingKind::TownHall)
        .count();
    let houses = huts.iter().count();
    let long = longhouses.iter().count();
    let cap = home::shelter_capacity(houses, long, halls);
    let food_stored = site
        .as_ref()
        .and_then(|s| stores.get(s.settlement).ok())
        .map_or(0.0, |s| s.food());

    // Children come of marriages. A wife whose husband is alive and grown may
    // bear a child; the couple with the least on their plate is not modeled —
    // the pick among eligible mothers is even.
    //
    // Counted BEFORE the gates rather than after, so that a village which is
    // not growing can say whether it has anyone to grow from. Brett: "The
    // game gets to about 17 people and the population stops growing... I
    // wonder what is going on with the needs and wants for courting,
    // marriage, family home and children?" - and nothing anywhere said.
    let mothers: Vec<_> = villagers
        .iter()
        .filter(|(_, _, genome, _, _, spouse, recovery, _)| {
            genome.age == Age::Adult
                && genome.sex == Sex::Female
                && spouse.is_some()
                && recovery.is_none_or(|r| clock.elapsed > r.until)
        })
        .filter(|(_, _, _, _, _, spouse, _, _)| {
            // The husband must himself still be living — a widow bears no child.
            spouse.is_some_and(|s| {
                villagers
                    .get(s.0)
                    .is_ok_and(|(_, _, g, ..)| g.age == Age::Adult && g.sex == Sex::Male)
            })
        })
        .collect();

    // The whole of why the village is or is not growing, in two lines, named
    // in the order the gates are actually asked.
    let nursing = villagers
        .iter()
        .filter(|(_, _, g, _, _, spouse, recovery, _)| {
            g.sex == Sex::Female
                && spouse.is_some()
                && recovery.is_some_and(|r| clock.elapsed <= r.until)
        })
        .count();
    let held = if living < 2 {
        "too few souls"
    } else if living >= cap {
        "NO ROOM - build houses"
    } else if average_hunger >= GROWTH_HUNGER {
        "HUNGER - the village eats too late"
    } else if food_stored < living as f32 * GROWTH_LARDER {
        "THE LARDER - not enough put by"
    } else if mothers.is_empty() && nursing > 0 {
        "every mother is nursing"
    } else if mothers.is_empty() {
        "NO MOTHERS - nobody is wed"
    } else {
        "nothing - it grows"
    };
    if let Ok(mut said) = GROWTH_THINKING.write() {
        *said = format!(
            "{living}/{cap} souls (fire {} + {houses} houses + {long} long + {halls} hall), \
             hunger {average_hunger:.2}/{GROWTH_HUNGER:.2}, food {food_stored:.0}/{:.0}, \
             mothers {} + {nursing} nursing\n  held by: {held}",
            home::FIRE_CIRCLE_SHELTER,
            living as f32 * GROWTH_LARDER,
            mothers.len(),
        );
    }

    if !can_grow_to(living, average_hunger, food_stored, cap) || !rng.0.chance(0.6) {
        return;
    }
    if mothers.is_empty() {
        return;
    }

    // Who bears is weighted by how many she already has, and WHETHER anyone
    // bears is then gated on the same number. The first keeps births moving to
    // the couples with room for them; the second is what actually slows a
    // village down as its families fill up.
    //
    // And by the marriage's WARMTH: children come likelier to couples who
    // hold each other dear. A floor, never a gate - the coldest marriage
    // still bears at three quarters the rate, because a village must
    // thrive whatever its heart is doing - but across a whole village the
    // difference is a demographic: a feared god's cold marriages thin the
    // nursery, a loved god's warm ones fill it.
    let warmth_of = |mother: Entity, father: Entity| -> f32 {
        let hers = hearts.get(mother).map_or(0.0, |h| h.toward(father));
        let his = hearts.get(father).map_or(0.0, |h| h.toward(mother));
        let mutual = (hers + his) * 0.5;
        0.75 + 0.5 * mutual.max(0.0)
    };
    let weights: Vec<f32> = mothers
        .iter()
        .map(|(who, _, _, _, _, spouse, _, _)| {
            let base = fertility(borne.get(*who).map_or(0, |m| m.borne));
            let warmth = spouse.map_or(1.0, |s| warmth_of(*who, s.0));
            base * warmth
        })
        .collect();
    let total: f32 = weights.iter().sum();
    if total <= 0.0 {
        return;
    }
    let mut roll = rng.0.range(0.0, total);
    let mut pick = weights.len() - 1;
    for (index, weight) in weights.iter().enumerate() {
        if roll < *weight {
            pick = index;
            break;
        }
        roll -= *weight;
    }
    let (mother, mother_t, mother_g, _, mother_p, spouse, _, mother_home) = &mothers[pick];
    let already_borne = borne.get(*mother).map_or(0, |m| m.borne);
    if !rng.0.chance(fertility(already_borne)) {
        return;
    }
    let father = spouse.expect("filtered above").0;
    let Ok((_, _, father_g, _, father_p, ..)) = villagers.get(father) else {
        return;
    };
    // A birth is followed by a season of nursing before the next.
    commands.entity(*mother).insert(Motherhood {
        borne: already_borne + 1,
    });
    // A birth is followed by a stretch of nursing before the next.
    commands.entity(*mother).insert(NewMother {
        until: clock.elapsed + crate::calendar::DAY_SECONDS as f64 * NURSING_DAYS,
    });

    let child_genome = CreatureGenome::child_of(mother_g, father_g, &mut rng.0);
    let name = culture.language.name_for(child_genome.sex, &mut rng.0);
    // The house name descends the father's line. A child of a founder whose
    // own name predates surnames takes the mother's rather than none.
    let surname = if father_p.surname.is_empty() {
        mother_p.surname.clone()
    } else {
        father_p.surname.clone()
    };
    let position = random_walkable_point(&terrain, &mut rng.0, mother_t.translation, 4.0)
        .unwrap_or(mother_t.translation);

    info!(
        "{name} {surname} was born to {} and {}",
        mother_p.full_name(),
        father_p.full_name()
    );
    notices.0.write(crate::ui::Notice::new(format!(
        "{name} was born to {} and {}",
        mother_p.name, father_p.name
    )));

    let child = match child_genome.sex {
        Sex::Female => "a daughter",
        Sex::Male => "a son",
    };
    let entity = spawn_creature(
        &mut commands,
        &assets,
        child_genome,
        position,
        rng.0.range(0.0, std::f32::consts::TAU),
        rng.0.f32() * 6.0,
    );

    let day = clock.day();
    let mut chronicle = Chronicle::default();
    chronicle.record(
        day,
        format!("born to {} and {}", mother_p.name, father_p.name),
    );
    if let Ok(mut record) = chronicles.get_mut(*mother) {
        record.record(day, format!("bore {child}, {name}"));
    }
    if let Ok(mut record) = chronicles.get_mut(father) {
        record.record(day, format!("fathered {child}, {name}"));
    }

    // A birth is an act of the world, witnessed like any other: most who
    // stand near call it a family's good day; the few who read providence
    // into it thank the god for the child — by name, kinship and all.
    notices.1.write(crate::witness::DivineEvent {
        kind: crate::witness::DivineEventKind::Delivered,
        position,
        subject: Some(entity),
        intensity: 0.6,
    });

    commands.entity(entity).insert((
        Villager,
        traits::Traits::roll(&mut rng.0),
        Person::born(name.clone(), surname),
        Parentage {
            mother: *mother,
            father,
        },
        Childhood {
            remaining: SECONDS_TO_COME_OF_AGE,
        },
        chronicle,
        crate::witness::Temperament::random(&mut rng.0),
        crate::witness::Witnessed::default(),
        speech::RecentlySaid::default(),
        Needs {
            hunger: 0.2,
            ..default()
        },
        Morale::default(),
        Activity::Idle,
    ));
    // The child belongs to the town that bore them. Reading the focused
    // settlement here instead meant every baby in the world was born a
    // citizen of whichever town the player happened to be looking at.
    if let Some(home) = mother_home
        .map(|m| m.0)
        .or_else(|| site.as_ref().map(|s| s.settlement))
    {
        commands.entity(entity).insert(MemberOf(home));
    }
}

/// Scores each option and switches activity when something outscores the current one.
fn choose_activity(
    site: Option<Res<SettlementSite>>,
    stores: Query<&work::Stockpile>,
    mut villagers: Query<
        (
            &mut Activity,
            &Needs,
            &Transform,
            &MoveTarget,
            Option<&MemberOf>,
            Option<&work::Vocation>,
        ),
        (
            With<Villager>,
            Without<Held>,
            Without<crate::avatar::Ridden>,
            Without<Airborne>,
        ),
    >,
    food: Query<
        (Entity, &Transform, &FoodSource),
        (With<crate::hand::DivinelyPlaced>, Without<Villager>),
    >,
    watch: Res<crate::debug::timings::Timings>,
) {
    let _t = watch.watch("villager: choose_activity");
    for (mut activity, needs, transform, target, member, vocation) in &mut villagers {
        // Their OWN town's larder. Reading the focused settlement here
        // would gate a colonist's meals — and now their prayers — on how
        // full the pantry is in whichever town the player is looking at.
        let larder = member
            .map(|m| m.0)
            .or_else(|| site.as_ref().map(|s| s.settlement))
            .and_then(|town| stores.get(town).ok())
            .map_or(0.0, |s| s.food());
        let hunger_score = food_utility(needs);
        let wander_score = wander_utility();

        // Hunger outranks every social hold. Grief, gossip and the fire's
        // tending can all wait; an empty stomach cannot. Without this, one
        // death gathered mourners whose own hunger ran out where they
        // stood, and the chronicle filled with a weeping crowd starving
        // beside a stocked larder, each death calling the next.
        if needs.hunger > 0.75 {
            let held_by_the_moment = matches!(
                *activity,
                Activity::Mourning
                    | Activity::Chatting
                    | Activity::TendingFire
                    | Activity::Sheltering
            );
            // A food prayer is not a distraction from hunger — it IS the
            // response to it. Hunger breaks a prayer only when there is
            // actually a meal to walk to instead; before this guard, the
            // same starvation that sent a villager to their knees stood
            // them straight back up, and the god never saw them kneel.
            let praying_past_a_meal = matches!(*activity, Activity::Praying) && larder >= 1.0;
            if held_by_the_moment || praying_past_a_meal {
                *activity = Activity::Idle;
            }
        }

        // Work, store visits, the fire and sleep are owned by the systems that
        // start them, and end on their own terms — hunger, nightfall, dawn.
        if matches!(
            *activity,
            Activity::Working
                | Activity::VisitingStore
                | Activity::TendingFire
                | Activity::Hauling
                | Activity::Praying
                | Activity::Marvelling
                | Activity::Sleeping
                | Activity::Mourning
                | Activity::Bearing
                | Activity::Chatting
                | Activity::Sheltering
        ) {
            continue;
        }

        // Eating is not interrupted until the villager is genuinely fed. Without
        // this they bounce off the bush the moment hunger dips below threshold.
        if let Activity::Eating(source) = *activity
            && needs.hunger > 0.05
            && food.get(source).is_ok_and(|(_, _, f)| f.amount > 0.0)
        {
            continue;
        }

        if hunger_score > wander_score {
            // The god's own gift is the one meal taken where it lands:
            // bread dropped from the sky is eaten under the sky, not
            // carried to a sack first.
            let gift = food
                .iter()
                .filter(|(_, _, source)| source.amount > 0.1)
                .map(|(entity, gift_at, _)| {
                    (
                        entity,
                        gift_at.translation.distance_squared(transform.translation),
                    )
                })
                .filter(|(_, distance)| *distance < 60.0 * 60.0)
                .min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((gift, _)) = gift {
                if *activity != Activity::SeekingFood(gift) {
                    *activity = Activity::SeekingFood(gift);
                }
                continue;
            }

            // Brett: "They should not eat from bushes, they should eat
            // from the stores." The village eats from its larder and
            // nowhere else — the bushes are the gatherers' workplace, not
            // anyone's table. A stocked larder is a meal; an empty one
            // still draws the desperate to the square, because the square
            // is where the starving kneel, and a prayer is what is left
            // when the sacks are empty.
            //
            // Unless they work a food trade: the desperate hunter's
            // answer to the empty sacks is a deer, not a vigil. This gate
            // mirrors the one in `eat_from_store` exactly — if only one
            // side sent providers to the square, the two systems would
            // trade the same soul back and forth every frame.
            let provider = matches!(
                vocation,
                Some(
                    work::Vocation::Fisher
                        | work::Vocation::Gatherer
                        | work::Vocation::Hunter
                        | work::Vocation::Farmer
                )
            );
            if larder >= 1.0 || (needs.hunger >= belief::DESPERATE_HUNGER && !provider) {
                *activity = Activity::VisitingStore;
                continue;
            }
        }

        // Nothing pressing. Wander, but only pick a new destination once the last
        // one is reached, or villagers twitch between targets every frame.
        if target.0.is_none() {
            *activity = Activity::Wandering;
        }
    }
}

/// Turns the chosen activity into movement and effects.
fn pursue_activity(
    time: Res<Time>,
    terrain: Res<Terrain>,
    members: Query<&MemberOf>,
    grounds: Query<&SettlementGround>,
    mut sim_rng: ResMut<SimRng>,
    mut villagers: Query<
        (
            Entity,
            &mut Activity,
            &mut Needs,
            &mut MoveTarget,
            &mut CreatureMotion,
            &Transform,
        ),
        (
            With<Villager>,
            Without<Held>,
            Without<Airborne>,
            // A body the god is wearing takes no errands, the same way one in
            // the god's hand takes none. This is the system that turns an
            // activity into somewhere to walk, and it has to EXCLUDE a worn
            // body rather than be overruled by one: `Wandering` with nowhere
            // to go rolls a fresh destination every frame, and one drawn
            // around the settlement center rather than the walker when the
            // walker is outside its radius. Possess a hunter in the woods and
            // the body sets off home at a stride, carrying the god with it.
            Without<crate::avatar::Ridden>,
        ),
    >,
    mut food: Query<
        (&Transform, &mut FoodSource),
        (With<crate::hand::DivinelyPlaced>, Without<Villager>),
    >,
) {
    let dt = time.delta_secs();

    for (who, mut activity, mut needs, mut target, mut motion, transform) in &mut villagers {
        // Home is wherever this person belongs, which after a town splits is
        // no longer the same square for everybody.
        let home_ground = members
            .get(who)
            .ok()
            .and_then(|member| grounds.get(member.0).ok())
            .copied();
        match *activity {
            Activity::Idle => {
                target.0 = None;
            }

            Activity::Wandering => {
                if target.0.is_none() {
                    // Wander around the settlement rather than the whole map, so the
                    // population stays somewhere the player can watch it.
                    let Some(home_ground) = home_ground else {
                        continue;
                    };
                    let origin = if transform.translation.distance(home_ground.center)
                        > home_ground.radius
                    {
                        home_ground.center
                    } else {
                        transform.translation
                    };
                    target.0 = random_walkable_point(
                        &terrain,
                        &mut sim_rng.0,
                        origin,
                        home_ground.radius * 0.7,
                    );
                }
            }

            Activity::SeekingFood(source) => {
                let Ok((food_transform, _)) = food.get(source) else {
                    // The gift was taken back or eaten out while we walked to it.
                    *activity = Activity::Idle;
                    target.0 = None;
                    continue;
                };

                let distance = food_transform.translation.distance(transform.translation);

                if distance <= EATING_RANGE {
                    *activity = Activity::Eating(source);
                    target.0 = None;
                } else {
                    target.0 = Some(food_transform.translation);
                }
            }

            Activity::Eating(source) => {
                let Ok((food_transform, mut food_source)) = food.get_mut(source) else {
                    *activity = Activity::Idle;
                    continue;
                };

                // The meal can be carried away mid-bite — this is a god game,
                // the player will absolutely do that.
                if food_transform.translation.distance(transform.translation) > EATING_RANGE * 1.5 {
                    *activity = Activity::Idle;
                    continue;
                }

                let bite = (0.35 * dt).min(food_source.amount);
                food_source.amount -= bite;
                needs.hunger = (needs.hunger - bite * 0.9).max(0.0);
                target.0 = None;

                if needs.hunger <= 0.02 || food_source.amount <= 0.0 {
                    *activity = Activity::Idle;
                }
            }

            // The work, home, rites and witness systems steer these
            // themselves.
            Activity::Working
            | Activity::VisitingStore
            | Activity::TendingFire
            | Activity::Hauling
            | Activity::Praying
            | Activity::Marvelling
            | Activity::Sleeping
            | Activity::Mourning
            | Activity::Bearing
            | Activity::Chatting
            | Activity::Sheltering => {}
        }

        // Starvation shows in the body before it shows in any UI.
        if needs.hunger > 0.9 {
            motion.flail = motion.flail.max(0.05);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    /// The opening days belong to the village: no predator seeds inside the
    /// widest walk any trade will make.
    ///
    /// Seed 4242 dealt a pack inside ninety units and seven of ten founders
    /// died before the first hall was framed; three sibling seeds dealt no
    /// close pack and lost nobody. A founding that lives or dies on that draw
    /// is a coin toss, not an opening.
    #[test]
    fn no_wolf_opens_within_a_working_walk_of_the_square() {
        for flock in 0..40 {
            let (nearest, furthest) = founding_flock_range(Species::Wolf, flock);
            assert!(
                nearest > crate::villager::work::RANGE_REACH,
                "a founding wolf may seed {nearest} out, inside the {} a \
                 villager will range",
                crate::villager::work::RANGE_REACH,
            );
            assert!(furthest > nearest);
            // And prey still opens close, or the view from the first morning
            // is an empty stage.
            let (deer_near, deer_far) = founding_flock_range(Species::Deer, flock);
            assert_eq!(deer_near, 0.0);
            assert!(deer_far <= 210.0);
        }
    }

    /// Every village, on every world, is given stone it can build out of.
    ///
    /// The one that must not fail. The loose rock came off the ground when
    /// Brett asked for it — "there is still entirely too many rocks on the
    /// ground… lets go back to removing about 90% of the small rocks and add
    /// quarries" — and the quarries are what took its place. A seed that hands
    /// a village no quarry hands it no masonry, and therefore no hall, no
    /// granary and no shrine: a village that cannot grow up.
    ///
    /// This is the failure the mine already has, written down in its own
    /// comment, and the reason the stone was left strewn over the world for so
    /// long. It must not be inherited.
    #[test]
    fn every_village_is_given_stone_to_build_with() {
        for seed in [7u32, 99, 2024, 4242, 31337, 555, 8080, 1] {
            let terrain = Terrain::new(seed);
            let mut rng = Rng::stream(seed as u64, "quarry test");
            // Several sites per world, so this is a claim about villages and
            // not about one lucky spot on eight maps.
            for _ in 0..6 {
                let here = terrain.somewhere_inland();
                let center = Vec3::new(here.x, terrain.height_at(here.x, here.y), here.y);
                let sites = quarry_sites(&terrain, center, &mut rng);
                assert!(
                    !sites.is_empty(),
                    "seed {seed} founds a village at {center:?} with no quarry \
                     in reach - it can never raise a footing"
                );
                for at in &sites {
                    let reach = Vec2::new(at.x - center.x, at.z - center.z).length();
                    assert!(
                        reach <= QUARRY_FURTHEST + 1.0,
                        "a quarry sits {reach} away, past the errand a miner will make"
                    );
                    assert!(
                        terrain.is_walkable(at.x, at.z),
                        "a quarry was put where nobody can stand"
                    );
                    assert!(
                        at.y >= crate::terrain::WATER_LEVEL,
                        "a quarry was put under water"
                    );
                }
            }
        }
    }

    #[test]
    fn quarries_are_spread_rather_than_heaped_in_one_pit() {
        // Six faces in one place is one quarry with extra steps, and a village
        // that empties it has nothing. They have to be worth walking between.
        let terrain = Terrain::new(4242);
        let mut rng = Rng::stream(4242, "quarry spread");
        let here = terrain.somewhere_inland();
        let center = Vec3::new(here.x, terrain.height_at(here.x, here.y), here.y);
        let sites = quarry_sites(&terrain, center, &mut rng);
        for (i, a) in sites.iter().enumerate() {
            for b in &sites[i + 1..] {
                assert!(
                    a.distance(*b) >= QUARRY_APART,
                    "two quarries {} apart",
                    a.distance(*b)
                );
            }
        }
    }

    #[test]
    fn retellings_stack_and_witnesses_are_spared() {
        let mut story = Chronicle::default();
        story.hear(1, "Gayou", "the sky itself struck");
        story.hear(1, "Ziale", "the sky itself struck");
        story.hear(1, "Tiafez", "the sky itself struck");
        story.hear(2, "Kazi", "a stranger walked on water");
        assert_eq!(story.events.len(), 2, "same story, one line");
        assert_eq!(
            story.events[0].text,
            "heard from Gayou and 2 others that the sky itself struck",
        );
        assert_eq!(
            story.events[1].text,
            "heard from Kazi that a stranger walked on water",
        );
    }

    #[test]
    fn news_changes_hands_at_a_meeting() {
        let mut app = App::new();
        app.init_resource::<crate::debug::timings::Timings>();
        app.insert_resource(Time::<()>::default());
        app.init_resource::<crate::calendar::WorldClock>();
        app.insert_resource(SimRng(Rng::new(8)));
        app.add_message::<crate::ui::Say>();
        app.add_systems(Update, (meet_to_talk, hold_conversations).chain());

        // Bob saw the god throw someone; Sue stands close enough to meet.
        app.world_mut().spawn((
            Villager,
            Transform::from_xyz(0.0, 0.0, 0.0),
            Activity::Idle,
            MoveTarget::default(),
            Person::born("Bob".into(), "Teller".into()),
            crate::witness::Witnessed {
                recent: vec![crate::witness::Memory {
                    kind: crate::witness::DivineEventKind::Thrown,
                    whom: None,
                    divine: true,
                    day: 1,
                    of: crate::witness::SubjectClass::Person,
                }],
                total: 1,
                secondhand: 0,
                told: 0,
            },
            Chronicle::default(),
        ));
        let sue = app
            .world_mut()
            .spawn((
                Villager,
                Transform::from_xyz(1.4, 0.0, 0.0),
                Activity::Idle,
                MoveTarget::default(),
                Person::born("Sue".into(), "Hearer".into()),
                crate::witness::Witnessed::default(),
                Chronicle::default(),
            ))
            .id();

        for _ in 0..30 {
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_secs(9));
            app.update();
        }

        let heard = app.world().get::<crate::witness::Witnessed>(sue).unwrap();
        assert!(heard.secondhand > 0, "Sue never heard the story");
        assert!(heard.is_innocent(), "hearing is not seeing");
        let chronicle = app.world().get::<Chronicle>(sue).unwrap();
        assert!(
            chronicle
                .events
                .iter()
                .any(|e| e.text.contains("heard from Bob")),
            "the telling missed her chronicle: {:?}",
            chronicle.events,
        );
    }

    #[test]
    fn news_changes_hands_where_nobody_is_watching() {
        // The whole risk of aiming the village's talk at the camera: that a
        // story only spreads where the god happens to be looking, and a world
        // left to itself comes back frozen. What is gated is the BUBBLE. What
        // must not be gated is any of this.
        let mut app = App::new();
        app.init_resource::<crate::debug::timings::Timings>();
        app.insert_resource(Time::<()>::default());
        app.init_resource::<crate::calendar::WorldClock>();
        app.insert_resource(SimRng(Rng::new(8)));
        app.insert_resource(crate::attention::Attention::blind());
        app.add_message::<crate::ui::Say>();
        app.add_systems(Update, (meet_to_talk, hold_conversations).chain());

        app.world_mut().spawn((
            Villager,
            Transform::from_xyz(0.0, 0.0, 0.0),
            Activity::Idle,
            MoveTarget::default(),
            Person::born("Bob".into(), "Teller".into()),
            crate::witness::Witnessed {
                recent: vec![crate::witness::Memory {
                    kind: crate::witness::DivineEventKind::Thrown,
                    whom: None,
                    divine: true,
                    day: 1,
                    of: crate::witness::SubjectClass::Person,
                }],
                total: 1,
                secondhand: 0,
                told: 0,
            },
            Chronicle::default(),
        ));
        let sue = app
            .world_mut()
            .spawn((
                Villager,
                Transform::from_xyz(1.4, 0.0, 0.0),
                Activity::Idle,
                MoveTarget::default(),
                Person::born("Sue".into(), "Hearer".into()),
                crate::witness::Witnessed::default(),
                Chronicle::default(),
            ))
            .id();

        for _ in 0..30 {
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_secs(9));
            app.update();
        }

        let heard = app.world().get::<crate::witness::Witnessed>(sue).unwrap();
        assert!(
            heard.secondhand > 0,
            "the story stopped spreading the moment the camera looked away",
        );
        let chronicle = app.world().get::<Chronicle>(sue).unwrap();
        assert!(
            chronicle
                .events
                .iter()
                .any(|e| e.text.contains("heard from Bob")),
            "an unwatched telling still belongs in her chronicle: {:?}",
            chronicle.events,
        );
    }

    #[test]
    fn children_grow_into_adults_with_rebuilt_bodies() {
        let mut app = App::new();
        app.init_resource::<crate::debug::timings::Timings>();
        app.add_plugins(bevy::time::TimePlugin);
        app.init_resource::<Assets<Mesh>>();
        app.init_resource::<Assets<StandardMaterial>>();
        app.init_resource::<crate::calendar::WorldClock>();
        app.add_message::<crate::ui::Notice>();
        app.add_message::<crate::ui::Say>();
        app.insert_resource(SimRng(Rng::new(3)));
        app.add_systems(Startup, crate::creature::body::init_creature_assets);
        app.add_systems(Update, grow_up);
        app.update();

        let mut genome = CreatureGenome::random(Species::Human, &mut Rng::new(5));
        genome.age = Age::Child;
        let spawned = genome.clone();
        let entity = app
            .world_mut()
            .run_system_once(move |mut commands: Commands, assets: Res<CreatureAssets>| {
                spawn_creature(
                    &mut commands,
                    &assets,
                    spawned.clone(),
                    Vec3::ZERO,
                    0.0,
                    0.0,
                )
            })
            .expect("spawn system runs");
        app.world_mut().entity_mut(entity).insert((
            Person::born("Testling".into(), "Youngly".into()),
            Childhood { remaining: 0.0 },
        ));

        let limbs_before = app
            .world()
            .entity(entity)
            .get::<Children>()
            .map_or(0, |c| c.len());
        assert!(limbs_before > 0, "child spawned with no body");

        app.update();
        app.update();

        let world = app.world();
        let grown = world.entity(entity);
        assert_eq!(
            grown.get::<CreatureGenome>().unwrap().age,
            Age::Adult,
            "still a child",
        );
        assert!(
            grown.get::<Childhood>().is_none(),
            "childhood should be over",
        );
        let limbs_after = grown.get::<Children>().map_or(0, |c| c.len());
        assert!(limbs_after > 0, "came of age but has no body");
    }

    #[test]
    fn fertility_falls_with_every_child() {
        // Strictly decreasing, so each birth genuinely makes the next less
        // likely rather than plateauing after the first.
        let mut previous = fertility(0);
        assert!(
            (previous - 1.0).abs() < 1e-6,
            "the first child should be unhindered"
        );
        for borne in 1..12 {
            let now = fertility(borne);
            assert!(
                now < previous,
                "child {borne} should be less likely than the one before"
            );
            previous = now;
        }
    }

    #[test]
    fn a_large_family_stays_possible_but_unlikely() {
        // Not a hard cap: the seventh child is a rarity, not an impossibility,
        // so a remarkable family can still happen.
        assert!(fertility(6) > 0.0);
        assert!(fertility(6) < 0.15, "a seventh child should be rare");
        // And the early ones come readily enough to grow a village.
        assert!(fertility(1) > 0.5);
        assert!(fertility(2) > 0.3);
    }

    #[test]
    fn starvation_kills_on_schedule_and_food_mends() {
        // Standing at empty should kill in SECONDS_STARVING_TO_DIE; eating back
        // below half should mend at the slower rate. The asymmetry is deliberate:
        // dying is faster than healing.
        assert!(SECONDS_TO_MEND > SECONDS_STARVING_TO_DIE);

        let dt = 0.1;
        let mut harm: f32 = 0.0;
        let mut elapsed = 0.0;
        while harm < 1.0 {
            harm = (harm + dt / SECONDS_STARVING_TO_DIE).min(1.0);
            elapsed += dt;
        }
        assert!((elapsed - SECONDS_STARVING_TO_DIE).abs() < 1.0);
    }

    #[test]
    fn villages_grow_only_when_fed_and_roofed_by_the_cap() {
        let cap = 24;
        assert!(
            can_grow_to(2, 0.3, 20.0, cap),
            "a fed pair should be able to grow"
        );
        assert!(
            !can_grow_to(1, 0.0, 20.0, cap),
            "one villager cannot grow a village"
        );
        assert!(!can_grow_to(cap, 0.0, 999.0, cap), "the cap must hold");
        assert!(
            !can_grow_to(10, 0.2, 5.0, cap),
            "a thin larder must gate births before hunger does"
        );
        assert!(
            !can_grow_to(10, 0.9, 99.0, cap),
            "a starving village must not grow"
        );
        assert!(
            can_grow_to(cap, 0.0, 99.0, cap + 10),
            "a town hall must raise the roof",
        );
    }

    #[test]
    fn hunger_only_matters_past_the_threshold() {
        let fed = Needs {
            hunger: 0.0,
            ..default()
        };
        let peckish = Needs {
            hunger: HUNGRY_THRESHOLD - 0.01,
            ..default()
        };
        assert_eq!(food_utility(&fed), 0.0);
        assert_eq!(food_utility(&peckish), 0.0);
        assert!(
            food_utility(&Needs {
                hunger: 0.5,
                ..default()
            }) > 0.0
        );
    }

    #[test]
    fn hunger_outweighs_wandering_once_it_bites() {
        // Mild hunger should lose to wandering; real hunger should win.
        assert!(
            food_utility(&Needs {
                hunger: 0.4,
                ..default()
            }) < wander_utility()
        );
        assert!(
            food_utility(&Needs {
                hunger: 0.8,
                ..default()
            }) > wander_utility()
        );
    }

    #[test]
    fn food_utility_rises_monotonically() {
        let mut previous = -1.0;
        for i in 0..=100 {
            let score = food_utility(&Needs {
                hunger: i as f32 / 100.0,
                ..default()
            });
            assert!(score >= previous, "utility dipped at {i}");
            previous = score;
        }
    }

    #[test]
    fn starvation_takes_the_expected_time() {
        let mut needs = Needs::default();
        let dt = 1.0 / 60.0;
        let mut elapsed = 0.0;
        while needs.hunger < 1.0 && elapsed < SECONDS_TO_STARVE * 2.0 {
            needs.hunger = (needs.hunger + dt / SECONDS_TO_STARVE).min(1.0);
            elapsed += dt;
        }
        assert!((elapsed - SECONDS_TO_STARVE).abs() < 1.0, "took {elapsed}s");
    }

    /// A world with one villager in it, who has seen what the test says
    /// they have seen.
    fn a_witness(divine: bool) -> App {
        let mut app = App::new();
        app.init_resource::<crate::debug::timings::Timings>();
        app.insert_resource(crate::calendar::WorldClock::default());
        app.insert_resource(crate::WorldSeed(9));
        app.insert_resource(SettlementCulture {
            language: names::Language::random(&mut Rng::new(4)),
        });
        app.add_message::<crate::ui::Notice>();
        app.world_mut().spawn(Settlement {
            name: "Lavu".into(),
            founded: 1,
            banner_ramp: 0,
            sigil: 0,
        });
        app.world_mut().spawn((
            Person::born("Temewa".into(), "Kirap".into()),
            crate::witness::Witnessed {
                recent: vec![crate::witness::Memory {
                    kind: crate::witness::DivineEventKind::Smote,
                    whom: None,
                    divine,
                    day: 1,
                    of: crate::witness::SubjectClass::Person,
                }],
                total: 1,
                secondhand: 0,
                told: 0,
            },
            Chronicle::default(),
        ));
        app.add_systems(Update, name_the_god);
        app
    }

    #[test]
    fn the_god_is_named_by_whoever_first_reads_a_hand_into_it() {
        let mut app = a_witness(true);
        assert!(
            app.world().get_resource::<DivineName>().is_none(),
            "named before anybody had seen anything"
        );
        app.update();
        let named = app
            .world()
            .get_resource::<DivineName>()
            .expect("somebody who believed what they saw should have named it");
        assert!(!named.0.is_empty());
    }

    #[test]
    fn a_skeptic_names_nothing() {
        // The verdict is rolled per witness and per act. Somebody who
        // decided it was the weather has not met a god to name, however
        // much they saw.
        let mut app = a_witness(false);
        app.update();
        assert!(
            app.world().get_resource::<DivineName>().is_none(),
            "a skeptic named a god they do not believe in"
        );
    }

    #[test]
    fn the_founding_is_five_women_and_five_men_all_grown() {
        // Brett's rule, pinned: ten founders, an even split, nobody too
        // small to lift anything. The alternation below is the same one
        // the spawn loop uses, so a change to either shows up here.
        let mut rng = Rng::new(5);
        let mut women = 0;
        let mut men = 0;
        for i in 0..STARTING_POPULATION {
            let sex = if i % 2 == 0 { Sex::Female } else { Sex::Male };
            let genome = CreatureGenome::adult(Species::Human, sex, &mut rng);
            assert_eq!(genome.age, Age::Adult, "a founder must be grown");
            match genome.sex {
                Sex::Female => women += 1,
                Sex::Male => men += 1,
            }
        }
        assert_eq!((women, men), (5, 5));
        // And the hall they wake up in holds exactly all of them.
        assert_eq!(
            crate::villager::work::BuildingKind::Longhouse.sleeps(),
            STARTING_POPULATION,
            "the founding village must fit under its own roof"
        );
    }

    #[test]
    fn settlement_site_is_somewhere_people_could_live() {
        for seed in [1u64, 2, 3, 2024] {
            let terrain = Terrain::new(seed as u32);
            let mut rng = Rng::stream(seed, "settlement");
            let site = choose_settlement_site(&terrain, &mut rng);

            assert!(
                terrain.is_walkable(site.x, site.z),
                "seed {seed}: unwalkable"
            );
            assert!(
                !terrain.is_submerged(site.x, site.z),
                "seed {seed}: underwater"
            );
        }
    }

    #[test]
    fn settlement_site_has_room_around_it() {
        // Villagers spawn in a radius around the center; if almost none of that
        // area is walkable the settlement will be stacked on one tile.
        let terrain = Terrain::new(2024);
        let mut rng = Rng::stream(2024, "settlement");
        let site = choose_settlement_site(&terrain, &mut rng);

        let mut found = 0;
        for _ in 0..300 {
            if random_walkable_point(&terrain, &mut rng, site, 10.0).is_some() {
                found += 1;
            }
        }
        assert!(found > 250, "only {found}/300 spawn attempts found ground");
    }

    #[test]
    fn probe_what_country_the_founders_pick() {
        let mut coastal = 0;
        let mut inland = 0;
        let mut flats = 0.0;
        const WORLDS: u32 = 24;
        for seed in 0..WORLDS {
            let terrain = Terrain::new(seed);
            let mut rng = Rng::stream(seed as u64, "settlement");
            let at = choose_settlement_site(&terrain, &mut rng);
            // How much sea sits in the working ring, the same band the score
            // measures its shoreline over.
            let mut water = 0;
            let mut samples = 0;
            for step in 0..12 {
                let angle = step as f32 / 12.0 * std::f32::consts::TAU;
                for distance in [42.0, 50.0, 58.0] {
                    samples += 1;
                    if terrain.is_submerged(at.x + angle.cos() * distance, at.z + angle.sin() * distance) {
                        water += 1;
                    }
                }
            }
            let fraction = water as f32 / samples as f32;
            let slope = terrain.slope_at(at.x, at.z);
            flats += slope;
            if fraction > 0.05 {
                coastal += 1;
            } else {
                inland += 1;
            }
            println!(
                "PROBE seed {seed:3}: water {:.0}%  slope {:.3}  height {:6.1}",
                fraction * 100.0,
                slope,
                at.y
            );
        }
        println!(
            "PROBE TOTAL: {coastal} coastal / {inland} inland, mean slope {:.3}",
            flats / WORLDS as f32
        );
    }

    #[test]
    fn settlement_site_is_not_a_sandbar() {
        // Regression: scoring water proximity without requiring land put the
        // settlement on an islet a few meters across, with the whole population
        // packed onto it and nowhere to walk.
        for seed in [1u64, 2, 3, 7, 42, 2024, 20241101] {
            let terrain = Terrain::new(seed as u32);
            let mut rng = Rng::stream(seed, "settlement");
            let site = choose_settlement_site(&terrain, &mut rng);

            let mut land = 0;
            let mut samples = 0;
            for angle_step in 0..16 {
                let angle = angle_step as f32 / 16.0 * std::f32::consts::TAU;
                for distance in [4.0, 8.0, 12.0, 16.0] {
                    let x = site.x + angle.cos() * distance;
                    let z = site.z + angle.sin() * distance;
                    samples += 1;
                    if terrain.is_walkable(x, z) {
                        land += 1;
                    }
                }
            }

            let fraction = land as f32 / samples as f32;
            assert!(
                fraction >= 0.45,
                "seed {seed}: only {fraction:.2} of the settlement's surroundings are land",
            );
        }
    }
}
