//! What people say when nobody asked.
//!
//! Every idle thought is picked from the corpus by the tags of the moment —
//! what is true about this person right now: their belly, their bed, their
//! marriage, their trade, their god. The sim describes the moment; the
//! corpus finds the words.

use bevy::prelude::*;

/// The last handful of things out of this person's mouth — thoughts,
/// tellings, answers, prayers and shouts alike.
///
/// The chronicle keeps a whole life of DEEDS, and what they were told by
/// others; nothing kept what they themselves said, so every line went to
/// a bubble and vanished. This is deliberately a short tail rather than a
/// full log: two hundred repetitions of "I should fix my boots" would
/// bury the chronicle's signal, and the question worth answering on a
/// person's page is what they are like NOW.
#[derive(Component, Default)]
pub struct RecentlySaid(pub Vec<Utterance>);

pub struct Utterance {
    pub day: u32,
    pub text: String,
    /// Kept to themselves rather than said out loud.
    pub thought: bool,
}

/// How many are worth keeping.
const KEPT: usize = 6;

/// Everything anybody says passes through `Say`, so one reader catches
/// every channel there is without each of them remembering to report.
pub(super) fn remember_what_was_said(
    clock: Res<crate::calendar::WorldClock>,
    mut said: MessageReader<crate::ui::Say>,
    mut mouths: Query<&mut RecentlySaid>,
) {
    for spoken in said.read() {
        let Ok(mut kept) = mouths.get_mut(spoken.speaker) else {
            continue;
        };
        kept.0.push(Utterance {
            day: clock.day(),
            text: spoken.text.clone(),
            thought: spoken.thought,
        });
        let over = kept.0.len().saturating_sub(KEPT);
        kept.0.drain(..over);
    }
}

use crate::creature::{Corpse, Held, Vitality};

use super::{Activity, Morale, Needs, SimRng, Villager, home, work};

/// Gives an idle soul a thought, every little while.
///
/// One head per tick, at most once a minute each, tagged with everything
/// true about them right now. Everyone thinks, watched or not - the pick
/// is free, and `show_musings` keeps unwatched thoughts to itself.
#[allow(clippy::type_complexity)]
pub(super) fn muse_the_watched(
    time: Res<Time>,
    mut since_last: Local<f32>,
    mut last_mused: Local<std::collections::HashMap<Entity, f32>>,
    mut clock: Local<f32>,
    tongue: Option<ResMut<crate::telling::Tongue>>,
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
            ),
        ),
        (With<Villager>, Without<Corpse>, Without<Held>),
    >,
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

    // The idle, not asked recently. Everyone thinks, watched or not: the
    // pick is free, and the showing already keeps unwatched thoughts to
    // itself.
    let candidates: Vec<_> = villagers
        .iter()
        .filter(|(entity, _, _, _, activity, ..)| {
            matches!(**activity, Activity::Idle | Activity::Wandering)
                && last_mused.get(entity).is_none_or(|at| *clock - at > 60.0)
        })
        .collect();
    if candidates.is_empty() {
        return;
    }
    let (entity, _, needs, _, _, person, extras) =
        candidates[(rng.0.f32() * candidates.len() as f32) as usize % candidates.len()];
    let (vitality, faith, _witnessed, spouse, house, vocation) = extras;

    // The corpus reads these as tags: the deeper the moment's true
    // description, the sharper the line it can reach for.
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
    body.push(if house.is_some() {
        "housed"
    } else {
        "roofless"
    });
    if spouse.is_some() {
        body.push("married");
    }

    last_mused.insert(entity, *clock);
    debug!("{} is asked for words", person.name);
    tongue.muse(crate::telling::Musing {
        who: entity,
        voice: vocation.copied(),
        faith: crate::telling::FaithBand::of(faith.map_or(0.3, |f| f.trust)),
        body,
        heard: None,
        aloud: false,
    });
}

/// Shows the words that have come back, over the heads they belong to.
///
/// A musing is a THOUGHT and shows as one, however crowded the square:
/// standing near someone is not talking to them, and a village of people
/// announcing their inner lives to the wind read as exactly that. The voice
/// belongs to conversations and to true cries, which arrive marked aloud.
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
    let started = std::time::Instant::now();
    let mut showed = 0u32;
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
        let (line, aloud) = line;
        info!(
            "a watched head finds its own words{}: {line}",
            if aloud { ", aloud" } else { "" }
        );
        say.write(crate::ui::Say {
            speaker: who,
            text: line.replace("the god", god),
            thought: !aloud,
            prayer: false,
        });
        showed += 1;
    }
    if showed > 0 {
        let ms = started.elapsed().as_secs_f32() * 1000.0;
        if ms > 2.0 {
            info!("scrub: show_musings took {ms:.1}ms");
        }
    }
}

