//! What people say when nobody asked.
//!
//! Small talk is the simulation showing through: every line a villager
//! offers is grounded in something true about them right now — their belly,
//! their bed, their marriage, their god, their trade, the hour, the sky. The
//! picker is a weighted draw over every line their state makes available, so
//! the pressing things come up more, but nobody is reduced to one complaint.
//! And nobody repeats themselves while they still have something fresh to
//! say: each person remembers their own recent lines, and the village
//! remembers what anyone said lately, so the same proverb never does two
//! laps of the square in a morning.

use bevy::prelude::*;

use crate::creature::genome::{Age, CreatureGenome};
use crate::creature::{Corpse, Held, Vitality};
use crate::rng::Rng;
use crate::weather::WeatherKind;

use super::{Activity, EARSHOT, Morale, Needs, SimRng, Spouse, Villager, home, work};

/// One candidate line with how loudly it asks to be said.
#[derive(Clone, Copy)]
struct Line(&'static str, f32);

/// Gathers every line this person's state can truthfully offer.
#[allow(clippy::too_many_arguments)]
fn candidates(
    needs: &Needs,
    morale: &Morale,
    manner: Option<&super::traits::Traits>,
    vitality: Option<&Vitality>,
    faith: Option<&super::belief::Faith>,
    witnessed: Option<&crate::witness::Witnessed>,
    genome: Option<&CreatureGenome>,
    spouse: bool,
    housed: bool,
    vocation: Option<&work::Vocation>,
    activity: &Activity,
    hour: f32,
    sky: Option<WeatherKind>,
) -> Vec<Line> {
    let mut pool: Vec<Line> = Vec::with_capacity(64);
    let mut add = |lines: &[&'static str], weight: f32| {
        for line in lines {
            pool.push(Line(line, weight));
        }
    };

    // The body speaks loudest.
    if needs.hunger > 0.6 {
        add(
            &[
                "my belly aches",
                "when did I last eat",
                "I could eat the whole harvest",
                "hunger makes the day long",
                "I would trade my boots for bread",
                "my stomach growls louder than I talk",
                "even the gulls eat better than me",
                "thin soup and thinner hope",
            ],
            3.0,
        );
    } else if needs.hunger > 0.35 {
        add(
            &[
                "I could do with a bite",
                "is there bread left",
                "something warm would go down well",
                "a heel of bread would mend this morning",
                "my thoughts keep drifting to the pot",
            ],
            1.5,
        );
    } else if needs.hunger < 0.15 {
        add(
            &[
                "a full belly makes light work",
                "I ate well today",
                "no finer feeling than enough",
                "I may never be hungry again",
            ],
            1.0,
        );
    }
    if needs.rest > 0.75 {
        add(
            &[
                "I could sleep where I stand",
                "my feet are done",
                "this day has been three days long",
                "my eyelids weigh like millstones",
                "one more hour, then I drop",
                "even my yawns are yawning",
            ],
            3.0,
        );
    }
    if vitality.is_some_and(|v| v.harm > 0.3) {
        add(
            &[
                "this wound is slow to mend",
                "I ache where it caught me",
                "the healer said rest, as if I could",
                "it throbs when the weather turns",
                "I will carry this scar a while",
                "worse has healed, so will this",
            ],
            3.0,
        );
    }

    // The roof, or the lack of one.
    if housed {
        add(
            &[
                "our roof held through the wind",
                "home before dark, with luck",
                "there is no sound like your own door",
                "four walls turn a night into a rest",
                "the hearth will be warm by now",
            ],
            0.8,
        );
    } else if genome.is_none_or(|g| g.age == Age::Adult) {
        add(
            &[
                "another night under the open sky",
                "a roof of my own, someday",
                "the fire is warm, but it is not a home",
                "I envy the housed their walls",
                "the stars are poor company in the rain",
                "I know every root in this ground by my back",
                "timber and hands, that is all a house is",
                "someday a door will close behind me",
            ],
            2.0,
        );
    }

    // The heart.
    if morale.spirits < 0.35 {
        add(
            &[
                "these are heavy days",
                "I hardly remember laughing",
                "the days blur into one",
                "something has to change",
                "I carry more than I can name",
                "even the birds sound tired",
                "I smile so no one asks",
                "tomorrow had better be kinder",
            ],
            2.5,
        );
    } else if morale.spirits > 0.7 {
        add(
            &[
                "a fine day to be alive",
                "I could sing, almost",
                "no complaints today, none",
                "life is good here",
                "I woke up glad and stayed that way",
                "we are luckier than we know",
            ],
            1.5,
        );
    }

    // Love, present or wanted.
    if genome.is_none_or(|g| g.age == Age::Adult) {
        if spouse {
            add(
                &[
                    "my love waits at home",
                    "we were wed under this same sky",
                    "marriage is work - good work",
                    "I still catch myself smiling at them",
                    "two are stronger than one and one",
                    "I married better than I deserved",
                ],
                1.0,
            );
        } else {
            add(
                &[
                    "someone to come home to, that is all I ask",
                    "I saw them across the square again",
                    "perhaps at the fire tonight",
                    "love finds the patient, they say",
                    "I rehearsed a greeting all morning",
                ],
                1.2,
            );
        }
    }

    // Age has its own voice.
    match genome.map(|g| g.age) {
        Some(Age::Elder) => add(
            &[
                "my bones know the weather",
                "I have seen all this before",
                "the young walk so fast now",
                "we built this from nothing, you know",
                "I remember when this was all trees",
                "my knees preach patience",
                "half my friends live in the ground now",
            ],
            1.5,
        ),
        Some(Age::Child) => add(
            &[
                "when I am grown I will hunt",
                "I found a beetle this morning",
                "race you to the banner",
                "why is the sky blue and the sea not",
                "I can hold my breath forever, watch",
                "do fish sleep",
                "I am not tired, I am NOT",
                "when I am big I will build the tallest house",
            ],
            2.0,
        ),
        _ => {}
    }

    // The god, believed or doubted.
    match faith.map(|f| f.trust) {
        Some(trust) if trust > 0.55 => add(
            &[
                "the god watches over us",
                "I felt something near this morning",
                "the god provides, in the end",
                "we are not alone, I am sure of it",
                "say what you like, the god listens",
                "there is a hand over this village",
            ],
            1.5,
        ),
        Some(trust) if trust < 0.2 => add(
            &[
                "the sky is empty, I think",
                "prayers cost nothing and buy the same",
                "we are on our own out here",
                "I have watched prayers go unanswered",
                "luck wears a god's name here",
                "show me, then I will kneel",
            ],
            1.5,
        ),
        _ => {}
    }
    if let Some(witnessed) = witnessed {
        if !witnessed.recent.is_empty() {
            add(
                &[
                    "I know what I saw",
                    "I still see it when I close my eyes",
                    "no one believes me who was not there",
                    "my hands have not stopped shaking",
                    "I will tell my grandchildren about that",
                ],
                2.0,
            );
        }
        if witnessed.secondhand > 2 {
            add(
                &[
                    "everyone has a story of the god now",
                    "the tales grow with each telling",
                    "I heard it thirdhand and still shivered",
                    "half the square swears they saw it",
                ],
                1.0,
            );
        }
    }

    // The trade, while at it or proud of it.
    let trade_weight = if *activity == Activity::Working {
        2.0
    } else {
        0.8
    };
    match vocation {
        Some(work::Vocation::Forester) => add(
            &[
                "the woods give good timber this season",
                "a tree tells you where it wants to fall",
                "sawdust in my beard again",
                "you learn a forest by its silences",
                "my axe knows the way home",
            ],
            trade_weight,
        ),
        Some(work::Vocation::Fisher) => add(
            &[
                "the water was kind today",
                "you learn patience, or you learn hunger",
                "the fish know my shadow by now",
                "a red dawn keeps me ashore",
                "nets mend faster with company",
            ],
            trade_weight,
        ),
        Some(work::Vocation::Farmer) => add(
            &[
                "the rows are coming up well",
                "good soil forgives a bad farmer",
                "rain does half my work",
                "every seed is a small promise",
                "the crows and I have an arrangement",
            ],
            trade_weight,
        ),
        Some(work::Vocation::Mason) => add(
            &[
                "good stone, this",
                "a foundation outlasts its house",
                "stone is honest, it never pretends",
                "my hands are maps of every wall here",
                "measure the stone twice, your fingers once",
            ],
            trade_weight,
        ),
        Some(work::Vocation::Carpenter) => add(
            &[
                "that frame will outlive us all",
                "measure long, cut once",
                "a true joint needs no nail",
                "wood remembers every hurry",
                "I dream in beams and pegs",
            ],
            trade_weight,
        ),
        Some(work::Vocation::Hunter) => add(
            &[
                "there are tracks by the ridge",
                "the herd moved east in the night",
                "the wind was wrong all morning",
                "you eat what you outwit",
                "still air, good hunting",
            ],
            trade_weight,
        ),
        Some(work::Vocation::Miner) => add(
            &[
                "the rock rings true",
                "stone keeps its secrets deep",
                "daylight looks strange after the seam",
                "you listen to rock, or it surprises you",
                "there is iron under this hill, I can smell it",
            ],
            trade_weight,
        ),
        Some(work::Vocation::Gatherer) => add(
            &[
                "the bushes are heavy this week",
                "you take what the land offers",
                "my basket knows the good spots",
                "berries hide from the impatient",
                "the sweet ones grow past the thorns",
            ],
            trade_weight,
        ),
        Some(work::Vocation::Cook) => add(
            &[
                "the pot wants more than roots",
                "everyone is brave until supper is late",
                "salt would change our lives",
                "a good broth forgives a hard day",
                "I season by argument",
            ],
            trade_weight,
        ),
        Some(work::Vocation::Healer) => add(
            &[
                "rest is the best medicine",
                "half my work is listening",
                "willow bark and patience",
                "wash it, wrap it, leave it be",
                "everyone heals at their own pace",
            ],
            trade_weight,
        ),
        Some(work::Vocation::Priest) => add(
            &[
                "all of it means something",
                "the god hears more than words",
                "doubt loudly, believe quietly",
                "I keep the litany, the litany keeps me",
                "every grave teaches the same sermon",
            ],
            trade_weight,
        ),
        Some(work::Vocation::Explorer) => add(
            &[
                "the world is wider than the fog",
                "the cairns are not the end of anything",
                "my feet itch past every marker",
                "somewhere out there is the answer",
                "maps lie, legs do not",
            ],
            trade_weight,
        ),
        Some(work::Vocation::Guard) => add(
            &[
                "quiet watch is good watch",
                "I saw shapes at the treeline",
                "the spear is mostly for leaning on",
                "wolves test the roads at dusk",
                "sleep well - that is the whole job",
            ],
            trade_weight,
        ),
        None => {}
    }

    // The hour.
    if (0.25..0.4).contains(&hour) {
        add(
            &[
                "the dew is cold this morning",
                "a new day, same work",
                "the light comes up kind today",
                "first one up sees the world unwrapped",
            ],
            0.8,
        );
    } else if (0.6..0.75).contains(&hour) {
        add(
            &[
                "the light goes gold",
                "nearly time for the fire",
                "the day folds itself away",
                "supper smoke on the wind",
            ],
            0.8,
        );
    } else if !(0.25..0.75).contains(&hour) {
        add(
            &[
                "the stars are out",
                "quiet, now",
                "the village breathes slower at night",
                "night makes every sound a story",
            ],
            0.8,
        );
    }

    // The sky over everyone's head is everyone's business.
    match sky {
        Some(WeatherKind::Rain) => add(
            &[
                "this rain finds every seam",
                "wet to the bone again",
                "the fields drink well today",
                "my boots may never dry",
                "smell that - wet earth",
            ],
            1.6,
        ),
        Some(WeatherKind::Storm) => add(
            &[
                "no one should be out in this",
                "that sky is furious",
                "count between the flash and the roar",
                "the wind wants my roof",
                "storms make believers, they say",
            ],
            2.2,
        ),
        Some(WeatherKind::Overcast) => add(
            &[
                "grey as an old kettle up there",
                "rain before supper, mark me",
                "the sun owes us a debt",
                "a lid on the sky all day",
            ],
            0.9,
        ),
        Some(WeatherKind::Clear) => add(
            &[
                "not a cloud anywhere",
                "a sky like this forgives a lot",
                "good drying weather",
                "the sea is glass today",
            ],
            0.7,
        ),
        None => {}
    }

    // The grain of the person, confessing itself.
    if let Some(manner) = manner {
        for line in manner.lines() {
            add(&[line], 1.3);
        }
    }

    // Always something, so the draw never comes up empty.
    add(
        &[
            "a fine enough day",
            "so it goes",
            "well, back to it",
            "the days keep coming",
            "same village, new morning",
            "you get used to it",
            "no news is its own news",
        ],
        0.4,
    );

    pool
}

/// Draws one line from the weighted pool.
fn draw(pool: &[Line], rng: &mut Rng) -> Option<&'static str> {
    let total: f32 = pool.iter().map(|line| line.1).sum();
    if total <= 0.0 {
        return None;
    }
    let mut roll = rng.f32() * total;
    for line in pool {
        roll -= line.1;
        if roll <= 0.0 {
            return Some(line.0);
        }
    }
    pool.last().map(|line| line.0)
}

/// Draws a line nobody is tired of: first avoiding everything this person
/// has said lately AND everything the village has heard lately, then
/// relaxing one ring at a time. Only a pool with no fresh line left in it
/// permits a repeat.
fn pick_fresh(
    pool: &[Line],
    said_by_them: &[&'static str],
    heard_lately: &[&'static str],
    rng: &mut Rng,
) -> Option<&'static str> {
    let strict: Vec<Line> = pool
        .iter()
        .filter(|l| !said_by_them.contains(&l.0) && !heard_lately.contains(&l.0))
        .copied()
        .collect();
    if let Some(line) = draw(&strict, rng) {
        return Some(line);
    }
    let theirs: Vec<Line> = pool
        .iter()
        .filter(|l| !said_by_them.contains(&l.0))
        .copied()
        .collect();
    if let Some(line) = draw(&theirs, rng) {
        return Some(line);
    }
    draw(pool, rng)
}

/// The ordinary murmur of a village: someone, every little while, says or
/// thinks what their day actually is. Speech when someone is near enough to
/// hear; a private thought when alone, which only the god gets to read.
#[allow(clippy::type_complexity)]
pub(super) fn small_talk(
    time: Res<Time>,
    mut since_last: Local<f32>,
    mut said_before: Local<std::collections::HashMap<Entity, Vec<&'static str>>>,
    mut heard_lately: Local<Vec<&'static str>>,
    name: Option<Res<super::DivineName>>,
    clock: Res<crate::calendar::WorldClock>,
    weather: Option<Res<crate::weather::Weather>>,
    attention: Option<Res<crate::attention::Attention>>,
    tongue: Option<Res<crate::telling::Tongue>>,
    mut rng: ResMut<SimRng>,
    mut say: MessageWriter<crate::ui::Say>,
    villagers: Query<
        (
            Entity,
            &Transform,
            &Needs,
            &Morale,
            &Activity,
            (
                Option<&Vitality>,
                Option<&super::belief::Faith>,
                Option<&crate::witness::Witnessed>,
                Option<&CreatureGenome>,
                Option<&Spouse>,
                Option<&home::Home>,
                Option<&work::Vocation>,
                Option<&super::traits::Traits>,
            ),
        ),
        (With<Villager>, Without<Corpse>, Without<Held>),
    >,
) {
    *since_last += time.delta_secs();
    if *since_last < 7.0 {
        return;
    }
    *since_last = 0.0;
    if !rng.0.chance(0.6) {
        return;
    }

    let all: Vec<_> = villagers.iter().collect();
    if all.is_empty() {
        return;
    }
    // The murmur is drawn from the people the god can actually see. Four towns
    // in, most of the world is off the frame at any moment, and a line said
    // there is composed, weighted, spent out of the freshness rings and then
    // thrown away unheard. Nothing about the simulation is gated here — small
    // talk has no consequences — only who is worth drawing.
    //
    // And when the teller is on, the closely watched are not drawn at all:
    // their words come composed, from `muse_the_watched`, and a written line
    // on top of a composed one would have them talking over themselves. The
    // written murmur keeps the MIDDLE distance alive — figures whose bubbles
    // are legible but who are too small to spend a model's time on.
    let composing = tongue.is_some();
    let seen: Vec<usize> = (0..all.len())
        .filter(|&i| {
            let regard = crate::attention::regard(attention.as_deref(), all[i].1.translation);
            regard.worth_saying() && !(composing && regard.worth_composing())
        })
        .collect();
    if seen.is_empty() {
        return;
    }
    let (speaker, at, needs, morale, activity, extras) =
        all[seen[(rng.0.f32() * seen.len() as f32) as usize % seen.len()]];
    let (vitality, faith, witnessed, genome, spouse, house, vocation, manner) = extras;

    let pool = candidates(
        needs,
        morale,
        manner,
        vitality,
        faith,
        witnessed,
        genome,
        spouse.is_some(),
        house.is_some(),
        vocation,
        activity,
        clock.time_of_day(),
        weather.as_ref().map(|w| w.kind()),
    );
    let mine = said_before.entry(speaker).or_default();
    let Some(line) = pick_fresh(&pool, mine, &heard_lately, &mut rng.0) else {
        return;
    };
    mine.push(line);
    if mine.len() > 10 {
        mine.remove(0);
    }
    heard_lately.push(line);
    if heard_lately.len() > 12 {
        heard_lately.remove(0);
    }

    // A thought if alone, said aloud if anyone is close enough to hear it.
    let heard = all.iter().any(|(other, other_at, ..)| {
        *other != speaker && other_at.translation.distance(at.translation) < EARSHOT
    });
    let god = name.as_ref().map_or("the god", |n| n.0.as_str());
    say.write(crate::ui::Say {
        speaker,
        text: line.replace("the god", god),
        thought: !heard,
        own_words: false,
    });
}

/// The one thing pressing on someone right now, for a thought to circle.
///
/// The sim chooses the topic and the model finds the words — never the other
/// way round. Priority runs body, then roof, then what they saw, then heart,
/// then the ordinary day; ties inside a band go to the dice so a hungry
/// village does not think in unison.
fn pressing_matter(
    needs: &Needs,
    hurt: bool,
    housed: bool,
    saw: Option<&str>,
    spirits: f32,
    spouse: bool,
    sky: Option<WeatherKind>,
    rng: &mut Rng,
) -> String {
    let mut matters: Vec<&str> = Vec::new();
    if needs.hunger > 0.6 {
        matters.push("the empty belly");
    }
    if needs.rest > 0.75 {
        matters.push("how long this day has been");
    }
    if hurt {
        matters.push("the wound, still mending");
    }
    if !matters.is_empty() {
        return (*rng.pick(&matters)).to_string();
    }
    if let Some(what) = saw {
        // What they saw the god do outweighs the ordinary worries, phrased
        // as the memory holds it: "saw lightning called down".
        return format!("what you {what}");
    }
    if !housed {
        matters.push("no roof of your own yet");
    }
    if spirits < 0.35 {
        matters.push("how heavy the days feel");
    } else if spirits > 0.7 {
        matters.push("how good the day feels");
    }
    if spouse {
        matters.push("the one waiting at home");
    } else {
        matters.push("having no one to come home to");
    }
    matters.push(match sky {
        Some(WeatherKind::Storm) => "the storm overhead",
        Some(WeatherKind::Rain) => "the rain coming down",
        Some(WeatherKind::Overcast) => "the grey sky",
        Some(WeatherKind::Clear) | None => "the day itself",
    });
    (*rng.pick(&matters)).to_string()
}

/// Asks the teller, ahead of time, for the thoughts of whoever the god is
/// actually watching.
///
/// A thought has no listener, so it has no deadline: it can be composed a
/// few seconds before it is shown, which is what makes per-person generation
/// affordable at all. Only the closely watched are asked for, only while
/// idle, and each head at most once a minute.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn muse_the_watched(
    time: Res<Time>,
    mut since_last: Local<f32>,
    mut last_mused: Local<std::collections::HashMap<Entity, f32>>,
    mut clock: Local<f32>,
    tongue: Option<ResMut<crate::telling::Tongue>>,
    attention: Option<Res<crate::attention::Attention>>,
    now: Option<Res<crate::now::WorldNow>>,
    mut rng: ResMut<SimRng>,
    villagers: Query<
        (
            Entity,
            &Transform,
            &Needs,
            &Morale,
            &Activity,
            &super::Person,
            (
                Option<&Vitality>,
                Option<&super::belief::Faith>,
                Option<&crate::witness::Witnessed>,
                Option<&super::Spouse>,
                Option<&home::Home>,
                Option<&work::Vocation>,
                Option<&super::traits::Traits>,
                Option<&super::MemberOf>,
            ),
        ),
        (With<Villager>, Without<Corpse>, Without<Held>),
    >,
    names: Query<&super::Person>,
) {
    let Some(mut tongue) = tongue else {
        return;
    };
    *clock += time.delta_secs();
    *since_last += time.delta_secs();
    if *since_last < 5.0 {
        return;
    }
    // The murmur's own odds, so composed talk keeps the easy, uneven pace
    // the written murmur always had rather than arriving on a metronome.
    if !rng.0.chance(0.75) {
        *since_last = 0.0;
        return;
    }
    *since_last = 0.0;

    // The watched and idle, not asked recently.
    let candidates: Vec<_> = villagers
        .iter()
        .filter(|(entity, at, _, _, activity, ..)| {
            matches!(**activity, Activity::Idle | Activity::Wandering)
                && crate::attention::regard(attention.as_deref(), at.translation).worth_composing()
                && last_mused.get(entity).is_none_or(|at| *clock - at > 60.0)
        })
        .collect();
    if candidates.is_empty() {
        return;
    }
    let (entity, _, needs, morale, _, person, extras) =
        candidates[(rng.0.f32() * candidates.len() as f32) as usize % candidates.len()];
    let (vitality, faith, witnessed, spouse, house, vocation, manner, member) = extras;

    // The place, from the settlement's digest; a person of no town thinks
    // with no place lines, which is honest.
    let place = member
        .and_then(|member| now.as_ref()?.places.get(&member.0).cloned())
        .map(|place| place.lines())
        .unwrap_or_default();

    let mut body: Vec<&'static str> = Vec::new();
    if needs.hunger > 0.6 {
        body.push("hungry");
    }
    if needs.rest > 0.75 {
        body.push("worn out");
    }
    if vitality.is_some_and(|v| v.harm > 0.3) {
        body.push("hurt");
    }

    let saw = witnessed
        .and_then(|w| w.recent.first())
        .map(|memory| memory.kind.describe());
    let mind = pressing_matter(
        needs,
        vitality.is_some_and(|v| v.harm > 0.3),
        house.is_some(),
        saw,
        morale.spirits,
        spouse.is_some(),
        None,
        &mut rng.0,
    );

    // Every name this thought is allowed to say: their own, their place,
    // their spouse, and whoever their freshest memory is about.
    let mut known: Vec<String> = vec![person.name.clone()];
    if let Some(place_name) = member
        .and_then(|member| now.as_ref()?.places.get(&member.0))
        .map(|p| p.name.clone())
    {
        known.push(place_name);
    }
    if let Some(dear) = spouse.and_then(|s| names.get(s.0).ok()) {
        known.push(dear.name.clone());
    }
    if let Some(whom) = witnessed
        .and_then(|w| w.recent.first())
        .and_then(|m| m.whom.as_ref())
    {
        known.push(whom.name.clone());
    }

    last_mused.insert(entity, *clock);
    tongue.muse(crate::telling::Musing {
        who: entity,
        voice: vocation.copied(),
        bearing: manner.map(|m| m.bearing()).unwrap_or_default(),
        faith: crate::telling::FaithBand::of(faith.map_or(0.3, |f| f.trust)),
        body,
        place,
        mind,
        heard: None,
        known,
    });
}

/// Shows the words that have come back, over the heads they belong to.
///
/// The same rule the written murmur has always kept: a thought if alone, said
/// aloud if anyone stands close enough to hear — the composed line does not
/// know or care which it will be.
#[allow(clippy::type_complexity)]
pub(super) fn show_musings(
    tongue: Option<ResMut<crate::telling::Tongue>>,
    attention: Option<Res<crate::attention::Attention>>,
    name: Option<Res<super::DivineName>>,
    mut say: MessageWriter<crate::ui::Say>,
    thinkers: Query<(Entity, &Transform), (With<Villager>, Without<Corpse>, Without<Held>)>,
) {
    let Some(mut tongue) = tongue else {
        return;
    };
    let god = name.as_ref().map_or("the god", |n| n.0.as_str());
    for who in tongue.mused_heads() {
        // Composed a moment ago, but the moment may have moved on: whoever
        // died, was seized, or wandered out of regard thinks it silently.
        let Ok((_, at)) = thinkers.get(who) else {
            tongue.take_musing(who);
            continue;
        };
        if !crate::attention::regard(attention.as_deref(), at.translation).worth_saying() {
            tongue.take_musing(who);
            continue;
        }
        let Some(line) = tongue.take_musing(who) else {
            continue;
        };
        let heard = thinkers.iter().any(|(other, other_at)| {
            other != who && other_at.translation.distance(at.translation) < EARSHOT
        });
        info!(
            "a watched head finds its own words{}: {line}",
            if heard { ", aloud" } else { "" }
        );
        say.write(crate::ui::Say {
            speaker: who,
            text: line.replace("the god", god),
            thought: !heard,
            own_words: true,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pressing_matter_is_the_body_first() {
        let mut rng = Rng::new(5);
        let starving = Needs {
            hunger: 0.9,
            rest: 0.2,
        };
        assert_eq!(
            pressing_matter(&starving, false, true, None, 0.5, true, None, &mut rng),
            "the empty belly"
        );
        // A miracle outweighs the ordinary worries, but not the body.
        let fine = Needs {
            hunger: 0.1,
            rest: 0.1,
        };
        assert_eq!(
            pressing_matter(
                &fine,
                false,
                true,
                Some("saw lightning called down"),
                0.5,
                true,
                None,
                &mut rng
            ),
            "what you saw lightning called down"
        );
        assert_eq!(
            pressing_matter(
                &starving,
                false,
                true,
                Some("saw lightning called down"),
                0.5,
                true,
                None,
                &mut rng
            ),
            "the empty belly"
        );
    }

    #[test]
    fn an_ordinary_day_still_finds_a_matter() {
        // Nothing pressing at all: the draw must still land somewhere, and
        // somewhere true — the spouse, the sky, the day.
        let mut rng = Rng::new(11);
        let fine = Needs {
            hunger: 0.1,
            rest: 0.1,
        };
        for _ in 0..20 {
            let matter = pressing_matter(&fine, false, true, None, 0.5, false, None, &mut rng);
            assert!(!matter.is_empty());
        }
    }

    #[test]
    fn a_hard_day_still_speaks_with_more_than_one_voice() {
        // The regression this module exists to prevent: a homeless, tired,
        // hungry villager once said exactly one thing, forever.
        let needs = Needs {
            hunger: 0.7,
            rest: 0.8,
        };
        let morale = Morale { spirits: 0.3 };
        let pool = candidates(
            &needs,
            &morale,
            None,
            None,
            None,
            None,
            None,
            false,
            false,
            None,
            &Activity::Idle,
            0.5,
            Some(WeatherKind::Rain),
        );
        assert!(
            pool.len() > 25,
            "a person in a hard state has many true things to say, found {}",
            pool.len()
        );

        let mut rng = Rng::new(77);
        let mut seen = std::collections::HashSet::new();
        for _ in 0..200 {
            if let Some(line) = draw(&pool, &mut rng) {
                seen.insert(line);
            }
        }
        assert!(
            seen.len() > 12,
            "two hundred draws should span many lines, got {}",
            seen.len()
        );
    }

    #[test]
    fn nobody_repeats_while_fresh_lines_remain() {
        let pool = [Line("first", 1.0), Line("second", 1.0), Line("third", 1.0)];
        let mut rng = Rng::new(9);
        // Two of three already said: a hundred draws must all land on the
        // third, never a repeat.
        for _ in 0..100 {
            let line = pick_fresh(&pool, &["first"], &["second"], &mut rng);
            assert_eq!(line, Some("third"));
        }
        // Everything already said: better a repeat than silence.
        let line = pick_fresh(&pool, &["first", "second", "third"], &[], &mut rng);
        assert!(line.is_some());
    }
}
