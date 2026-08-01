//! What people say when nobody asked.
//!
//! Every word here is composed for its speaker in the moment — the sim picks
//! the topic from what is true about them right now (their belly, their bed,
//! their marriage, their god, the sky, what lately happened), and the teller
//! finds the words. The written murmur that once served this module retired
//! when the no-canned rule landed: a moment nothing was composed for is a
//! quiet moment.

use bevy::prelude::*;

use crate::creature::{Corpse, Held, Vitality};
use crate::rng::Rng;
use crate::weather::WeatherKind;

use super::{Activity, Morale, Needs, SimRng, Villager, home, work};

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
        aloud: false,
        prayer: false,
        known,
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
        let (line, aloud, prayer) = line;
        info!(
            "a watched head finds its own words{}: {line}",
            if aloud { ", aloud" } else { "" }
        );
        say.write(crate::ui::Say {
            speaker: who,
            text: line.replace("the god", god),
            thought: !aloud,
            prayer,
            own_words: true,
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
}
