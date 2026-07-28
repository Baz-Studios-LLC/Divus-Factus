//! What people say when nobody asked.
//!
//! Small talk is the simulation showing through: every line a villager
//! offers is grounded in something true about them right now — their belly,
//! their bed, their marriage, their god, their trade, the hour. The picker
//! is a weighted draw over every line their state makes available, so the
//! pressing things come up more, but nobody is reduced to one complaint.

use bevy::prelude::*;

use crate::creature::genome::{Age, CreatureGenome};
use crate::creature::{Corpse, Held, Vitality};
use crate::rng::Rng;

use super::{Activity, EARSHOT, Morale, Needs, SimRng, Spouse, Villager, home, work};

/// One candidate line with how loudly it asks to be said.
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
) -> Vec<Line> {
    let mut pool: Vec<Line> = Vec::with_capacity(24);
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
            ],
            3.0,
        );
    } else if needs.hunger > 0.35 {
        add(&["I could do with a bite", "is there bread left"], 1.5);
    } else if needs.hunger < 0.15 {
        add(&["a full belly makes light work", "I ate well today"], 1.0);
    }
    if needs.rest > 0.75 {
        add(
            &[
                "I could sleep where I stand",
                "my feet are done",
                "this day has been three days long",
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
                ],
                1.0,
            );
        } else {
            add(
                &[
                    "someone to come home to, that is all I ask",
                    "I saw them across the square again",
                    "perhaps at the fire tonight",
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
            ],
            1.5,
        ),
        Some(Age::Child) => add(
            &[
                "when I am grown I will hunt",
                "I found a beetle this morning",
                "race you to the banner",
                "why is the sky blue and the sea not",
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
            ],
            1.5,
        ),
        Some(trust) if trust < 0.2 => add(
            &[
                "the sky is empty, I think",
                "prayers cost nothing and buy the same",
                "we are on our own out here",
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
                ],
                2.0,
            );
        }
        if witnessed.secondhand > 2 {
            add(
                &[
                    "everyone has a story of the god now",
                    "the tales grow with each telling",
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
            ],
            trade_weight,
        ),
        Some(work::Vocation::Fisher) => add(
            &[
                "the water was kind today",
                "you learn patience, or you learn hunger",
            ],
            trade_weight,
        ),
        Some(work::Vocation::Farmer) => add(
            &[
                "the rows are coming up well",
                "good soil forgives a bad farmer",
            ],
            trade_weight,
        ),
        Some(work::Vocation::Mason) => add(
            &["good stone, this", "a foundation outlasts its house"],
            trade_weight,
        ),
        Some(work::Vocation::Carpenter) => add(
            &["that frame will outlive us all", "measure long, cut once"],
            trade_weight,
        ),
        Some(work::Vocation::Hunter) => add(
            &[
                "there are tracks by the ridge",
                "the herd moved east in the night",
            ],
            trade_weight,
        ),
        Some(work::Vocation::Miner) => add(
            &["the rock rings true", "stone keeps its secrets deep"],
            trade_weight,
        ),
        Some(work::Vocation::Gatherer) => add(
            &[
                "the bushes are heavy this week",
                "you take what the land offers",
            ],
            trade_weight,
        ),
        Some(work::Vocation::Cook) => add(
            &[
                "the pot wants more than roots",
                "everyone is brave until supper is late",
            ],
            trade_weight,
        ),
        Some(work::Vocation::Healer) => add(
            &["rest is the best medicine", "half my work is listening"],
            trade_weight,
        ),
        Some(work::Vocation::Priest) => add(
            &["all of it means something", "the god hears more than words"],
            trade_weight,
        ),
        Some(work::Vocation::Explorer) => add(
            &[
                "the world is wider than the fog",
                "the cairns are not the end of anything",
            ],
            trade_weight,
        ),
        None => {}
    }

    // The hour.
    if (0.25..0.4).contains(&hour) {
        add(
            &["the dew is cold this morning", "a new day, same work"],
            0.8,
        );
    } else if (0.6..0.75).contains(&hour) {
        add(&["the light goes gold", "nearly time for the fire"], 0.8);
    } else if !(0.25..0.75).contains(&hour) {
        add(&["the stars are out", "quiet, now"], 0.8);
    }

    // The grain of the person, confessing itself.
    if let Some(manner) = manner {
        for line in manner.lines() {
            add(&[line], 1.3);
        }
    }

    // Always something, so the draw never comes up empty.
    add(
        &["a fine enough day", "so it goes", "well, back to it"],
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

/// The ordinary murmur of a village: someone, every little while, says or
/// thinks what their day actually is. Speech when someone is near enough to
/// hear; a private thought when alone, which only the god gets to read.
#[allow(clippy::type_complexity)]
pub(super) fn small_talk(
    time: Res<Time>,
    mut since_last: Local<f32>,
    name: Option<Res<super::DivineName>>,
    clock: Res<crate::calendar::WorldClock>,
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
    let (speaker, at, needs, morale, activity, extras) =
        all[(rng.0.f32() * all.len() as f32) as usize % all.len()];
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
    );
    let Some(line) = draw(&pool, &mut rng.0) else {
        return;
    };

    // A thought if alone, said aloud if anyone is close enough to hear it.
    let heard = all.iter().any(|(other, other_at, ..)| {
        *other != speaker && other_at.translation.distance(at.translation) < EARSHOT
    });
    let god = name.as_ref().map_or("the god", |n| n.0.as_str());
    say.write(crate::ui::Say {
        speaker,
        text: line.replace("the god", god),
        thought: !heard,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

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
        );
        assert!(
            pool.len() > 12,
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
            seen.len() > 8,
            "two hundred draws should span many lines, got {}",
            seen.len()
        );
    }
}
