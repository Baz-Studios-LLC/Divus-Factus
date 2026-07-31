//! The world as a villager would tell it, right now.
//!
//! The simulation knows everything; a prompt can afford almost nothing. This
//! module is the digest between them: a few lines per settlement — where,
//! when, how the larder stands, who sleeps out — rebuilt on a slow tick and
//! shared by every prompt asked in that town. The teller reads it so that a
//! thought composed in a starving village in winter rain is not
//! interchangeable with one from a fat town in high summer.
//!
//! One rule, held hard: **every line here is read out of the simulation.**
//! Nothing is invented, nothing is embellished, and anything the sim cannot
//! vouch for is simply absent. The model's job is the words; the world's job
//! is the facts.

use std::collections::HashMap;

use bevy::prelude::*;

use crate::villager::home::Home;
use crate::villager::{MemberOf, Settlement, Villager};
use crate::weather::WeatherKind;

/// How a larder reads to the people eating from it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Larder {
    Empty,
    Thin,
    Enough,
    Full,
}

impl Larder {
    /// Banded on days of food in hand, matching the colony pressure's
    /// arithmetic of two food per mouth per day.
    pub fn of(food: f32, mouths: usize) -> Larder {
        if mouths == 0 {
            return Larder::Enough;
        }
        let days = food / (mouths as f32 * 2.0);
        if days < 0.4 {
            Larder::Empty
        } else if days < 1.5 {
            Larder::Thin
        } else if days < 4.0 {
            Larder::Enough
        } else {
            Larder::Full
        }
    }

    fn phrase(self) -> &'static str {
        match self {
            Larder::Empty => "the larder is all but empty",
            Larder::Thin => "the larder runs thin",
            Larder::Enough => "the larder holds, for now",
            Larder::Full => "the stores stand full",
        }
    }
}

/// One settlement's now: the facts a prompt gets to lean on.
#[derive(Clone, Debug)]
pub struct PlaceNow {
    pub name: String,
    pub season: &'static str,
    pub sky: &'static str,
    pub roofless: usize,
    pub larder: Larder,
    /// What has lately happened, in the notices' own words — a wedding, a
    /// wolf slain, ground broken. The same lines the player reads, so what
    /// the village talks about is what the village announced.
    pub lately: Vec<String>,
}

impl PlaceNow {
    /// The prompt lines, in the same field-per-line register the teller
    /// speaks everywhere else.
    pub fn lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("where you live: the village of {}", self.name),
            format!("the time: {}, {}", self.season, self.sky),
            format!("the village: {}", self.larder.phrase()),
        ];
        if self.roofless > 0 {
            lines.push(format!(
                "the village also: {} still sleep without a roof",
                count_word(self.roofless)
            ));
        }
        for happening in &self.lately {
            lines.push(format!("lately: {happening}"));
        }
        lines
    }
}

/// Small counts as a villager would say them; past that, "many".
fn count_word(n: usize) -> &'static str {
    match n {
        1 => "one",
        2 => "two",
        3 => "three",
        4 => "four",
        5 => "five",
        6 => "six",
        7 => "seven",
        8 => "eight",
        _ => "many",
    }
}

/// How the sky reads from under it.
fn sky_phrase(kind: Option<WeatherKind>) -> &'static str {
    match kind {
        Some(WeatherKind::Rain) => "rain falling",
        Some(WeatherKind::Storm) => "a storm overhead",
        Some(WeatherKind::Overcast) => "the sky grey and low",
        Some(WeatherKind::Clear) | None => "the sky clear",
    }
}

/// Every settlement's now, keyed by the settlement entity.
#[derive(Resource, Default)]
pub struct WorldNow {
    pub places: HashMap<Entity, PlaceNow>,
}

/// Rebuilds the digest. Slow on purpose: nothing in it moves faster than a
/// larder drains, and every prompt in the town shares the same few lines.
fn take_stock(
    time: Res<Time>,
    mut since_last: Local<f32>,
    mut lately: Local<std::collections::VecDeque<String>>,
    mut notices: MessageReader<crate::ui::Notice>,
    mut now: ResMut<WorldNow>,
    clock: Res<crate::calendar::WorldClock>,
    weather: Option<Res<crate::weather::Weather>>,
    towns: Query<(Entity, &Settlement, &crate::villager::work::Stockpile)>,
    folk: Query<(&MemberOf, Option<&Home>), (With<Villager>, Without<crate::creature::Corpse>)>,
) {
    // Notices are read every frame — a reader that only wakes on the slow
    // tick would miss everything announced in between. Digits are kept out
    // of prompts, and the fanfares (foundings, namings) are already carried
    // by the place lines themselves.
    for notice in notices.read() {
        if notice.text.chars().any(|c| c.is_ascii_digit()) {
            continue;
        }
        lately.push_back(notice.text.to_lowercase());
        while lately.len() > 3 {
            lately.pop_front();
        }
    }

    *since_last += time.delta_secs();
    // First pass runs immediately, so no prompt ever sees an empty world.
    if *since_last < 10.0 && !now.places.is_empty() {
        return;
    }
    *since_last = 0.0;

    let mut mouths: HashMap<Entity, usize> = HashMap::new();
    let mut roofless: HashMap<Entity, usize> = HashMap::new();
    for (member, home) in &folk {
        *mouths.entry(member.0).or_default() += 1;
        if home.is_none() {
            *roofless.entry(member.0).or_default() += 1;
        }
    }

    now.places.clear();
    for (town, settlement, store) in &towns {
        let fed = mouths.get(&town).copied().unwrap_or(0);
        now.places.insert(
            town,
            PlaceNow {
                name: settlement.name.clone(),
                season: clock.season().name(),
                sky: sky_phrase(weather.as_ref().map(|w| w.kind())),
                roofless: roofless.get(&town).copied().unwrap_or(0),
                larder: Larder::of(store.food(), fed),
                lately: lately.iter().cloned().collect(),
            },
        );
    }
}

pub struct NowPlugin;

impl Plugin for NowPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WorldNow>().add_systems(
            Update,
            take_stock.run_if(resource_exists::<crate::calendar::WorldClock>),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_larder_reads_in_days_not_absolutes() {
        // Ten food is a feast for one mouth and a crisis for twenty.
        assert_eq!(Larder::of(10.0, 1), Larder::Full);
        assert_eq!(Larder::of(10.0, 20), Larder::Empty);
        assert_eq!(Larder::of(30.0, 12), Larder::Thin);
        assert_eq!(Larder::of(60.0, 12), Larder::Enough);
        // An empty town is not starving; it is empty.
        assert_eq!(Larder::of(0.0, 0), Larder::Enough);
    }

    #[test]
    fn the_lines_carry_only_what_is_true() {
        let mut place = PlaceNow {
            name: "Fribe".into(),
            season: "winter",
            sky: "a storm overhead",
            roofless: 0,
            larder: Larder::Thin,
            lately: vec!["fithzu tilled a new field".into()],
        };
        let lines = place.lines().join("\n");
        assert!(lines.contains("the village of Fribe"));
        assert!(lines.contains("winter, a storm overhead"));
        assert!(lines.contains("the larder runs thin"));
        // Nobody roofless: the line simply is not there, rather than a line
        // reporting that there is nothing to report.
        assert!(!lines.contains("without a roof"));

        place.roofless = 3;
        assert!(
            place
                .lines()
                .join("\n")
                .contains("three still sleep without a roof")
        );
    }

    #[test]
    fn no_digit_ever_reaches_a_prompt() {
        // The admissibility gate rejects digits in the model's OUTPUT; a
        // digit in the input teaches the model to produce them.
        for n in [1, 5, 9, 40] {
            let place = PlaceNow {
                name: "Fribe".into(),
                season: "spring",
                sky: "the sky clear",
                roofless: n,
                larder: Larder::Enough,
                lately: vec![],
            };
            for line in place.lines() {
                assert!(
                    !line.chars().any(|c| c.is_ascii_digit()),
                    "a digit slipped into: {line}"
                );
            }
        }
    }
}
