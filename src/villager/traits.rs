//! Traits: the grain of a person, for better and worse.
//!
//! A trait earns its place only if systems read it. Every one here bends a
//! real mechanic — how fast they work, how hard belief lands, how quickly
//! the heart mends, how often they talk, how much they eat — so two
//! villagers with the same job and the same day live it differently. The
//! inspector speaks them as manner ("diligent, and a glutton"), and small
//! talk lets them confess themselves.

use bevy::prelude::*;

use crate::rng::Rng;

/// One inclination, virtue or flaw.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum Trait {
    // Virtues.
    Diligent,
    Devout,
    Cheerful,
    Chatty,
    Hardy,
    // Flaws.
    Slothful,
    Skeptic,
    Gloomy,
    Quiet,
    Glutton,
}

impl Trait {
    pub fn word(self) -> &'static str {
        match self {
            Trait::Diligent => "diligent",
            Trait::Devout => "devout",
            Trait::Cheerful => "cheerful",
            Trait::Chatty => "chatty",
            Trait::Hardy => "hardy",
            Trait::Slothful => "slothful",
            Trait::Skeptic => "a skeptic",
            Trait::Gloomy => "gloomy",
            Trait::Quiet => "quiet",
            Trait::Glutton => "a glutton",
        }
    }
}

/// How a manner bends someone's WORDS, as against the mechanics it bends.
///
/// Deliberately coarser than the trait list, for two reasons. The teller keys
/// its cache on the shape of a telling, and anything the prompt mentions has to
/// be part of that key — put all ten traits in and the shape space multiplies
/// by thirty, the cache never fills, and everyone falls back to written lines.
/// And most traits say nothing about how a person describes a bolt of
/// lightning: whether they are diligent or slothful does not come into it,
/// while whether they expect the worst comes into all of it.
///
/// Devout and Skeptic are absent on purpose — they already reach the teller as
/// [`crate::telling::FaithBand`], and saying it twice would only make the
/// prompt longer.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Bearing {
    /// Quiet: says as little as will do.
    Terse,
    /// Gloomy: expects the worst, and mentions it.
    Bleak,
    /// Cheerful: finds the good in it.
    Bright,
    /// Nothing in the manner that bends the words.
    #[default]
    Plain,
}

impl Bearing {
    /// How the teller is told about it. `None` for a manner that changes
    /// nothing, so the plainly-spoken get a shorter prompt rather than a line
    /// telling the model to ignore something.
    pub fn word(self) -> Option<&'static str> {
        match self {
            Bearing::Terse => Some("says as little as will do"),
            Bearing::Bleak => Some("expects the worst of everything"),
            Bearing::Bright => Some("looks for the good in everything"),
            Bearing::Plain => None,
        }
    }
}

/// The traits a person carries: at most one virtue and one flaw, rolled at
/// birth and kept for life.
#[derive(Component, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Traits(pub Vec<Trait>);

impl Traits {
    pub fn roll(rng: &mut Rng) -> Self {
        const VIRTUES: [Trait; 5] = [
            Trait::Diligent,
            Trait::Devout,
            Trait::Cheerful,
            Trait::Chatty,
            Trait::Hardy,
        ];
        const FLAWS: [Trait; 5] = [
            Trait::Slothful,
            Trait::Skeptic,
            Trait::Gloomy,
            Trait::Quiet,
            Trait::Glutton,
        ];
        let mut traits = Vec::with_capacity(2);
        if rng.chance(0.55) {
            traits.push(*rng.pick(&VIRTUES));
        }
        if rng.chance(0.55) {
            let flaw = *rng.pick(&FLAWS);
            // A person is not their own opposite.
            let clashes = matches!(
                (traits.first(), flaw),
                (Some(Trait::Diligent), Trait::Slothful)
                    | (Some(Trait::Devout), Trait::Skeptic)
                    | (Some(Trait::Cheerful), Trait::Gloomy)
                    | (Some(Trait::Chatty), Trait::Quiet)
            );
            if !clashes {
                traits.push(flaw);
            }
        }
        Traits(traits)
    }

    pub fn has(&self, wanted: Trait) -> bool {
        self.0.contains(&wanted)
    }

    /// The manner line the inspector shows: "diligent, and a glutton".
    pub fn describe(&self) -> String {
        match self.0.as_slice() {
            [] => "unremarkable".to_string(),
            [one] => one.word().to_string(),
            [a, b] => format!("{}, and {}", a.word(), b.word()),
            more => more.iter().map(|t| t.word()).collect::<Vec<_>>().join(", "),
        }
    }

    /// Multiplier on a work cycle: the diligent finish sooner.
    pub fn work_pace(&self) -> f32 {
        if self.has(Trait::Diligent) {
            0.8
        } else if self.has(Trait::Slothful) {
            1.3
        } else {
            1.0
        }
    }

    /// Multiplier on every faith movement, up or down.
    pub fn conviction(&self) -> f32 {
        if self.has(Trait::Devout) {
            1.5
        } else if self.has(Trait::Skeptic) {
            0.5
        } else {
            1.0
        }
    }

    /// Multiplier on spirits recovery.
    pub fn brightness(&self) -> f32 {
        if self.has(Trait::Cheerful) {
            1.5
        } else if self.has(Trait::Gloomy) {
            0.6
        } else {
            1.0
        }
    }

    /// Chance multiplier on choosing to tell a story.
    pub fn talkativeness(&self) -> f32 {
        if self.has(Trait::Chatty) {
            1.8
        } else if self.has(Trait::Quiet) {
            0.35
        } else {
            1.0
        }
    }

    /// Multiplier on the day's wear (rest drain).
    pub fn endurance(&self) -> f32 {
        if self.has(Trait::Hardy) { 0.75 } else { 1.0 }
    }

    /// Multiplier on food drawn per meal.
    pub fn appetite(&self) -> f32 {
        if self.has(Trait::Glutton) { 1.4 } else { 1.0 }
    }

    /// Which way this manner bends the person's words.
    ///
    /// Brevity is tested first because it governs the delivery of whatever else
    /// is true: a quiet cheerful person is short about the good news.
    pub fn bearing(&self) -> Bearing {
        if self.has(Trait::Quiet) {
            Bearing::Terse
        } else if self.has(Trait::Gloomy) {
            Bearing::Bleak
        } else if self.has(Trait::Cheerful) {
            Bearing::Bright
        } else {
            Bearing::Plain
        }
    }

    /// Small-talk lines this manner offers.
    pub fn lines(&self) -> Vec<&'static str> {
        let mut lines = Vec::new();
        for t in &self.0 {
            lines.extend_from_slice(match t {
                Trait::Diligent => &[
                    "idle hands itch",
                    "the work does not do itself",
                    "done beats perfect",
                    "I will rest when the pile is stacked",
                    "sweat now, sleep well later",
                ][..],
                Trait::Devout => &[
                    "I said my thanks this morning",
                    "nothing happens unwatched",
                    "every meal is a small mercy",
                    "I keep the god's name out of my complaints",
                    "first fruits first, always",
                ],
                Trait::Cheerful => &[
                    "could be worse, could be raining",
                    "smile, it costs nothing",
                    "every day has one good thing, find it",
                    "laughter keeps the cold out",
                    "I woke on the right side of the ground",
                ],
                Trait::Chatty => &[
                    "did you hear about the fisher",
                    "stop me if I told you this",
                    "I only repeat what I hear, mostly",
                    "a secret is just a story with buttons",
                    "come, talk, the work will wait",
                ],
                Trait::Hardy => &[
                    "cold never hurt anyone",
                    "I have walked further on less",
                    "blisters are just opinions",
                    "I have slept on stones and thanked them",
                    "weather is a mood, ignore it",
                ],
                Trait::Slothful => &[
                    "it will keep until tomorrow",
                    "why stand when you can sit",
                    "hurry is a kind of greed",
                    "rest is also work, thankless work",
                    "the best ideas come lying down",
                ],
                Trait::Skeptic => &[
                    "I believe what I can bite",
                    "coincidence wears many masks",
                    "a story grows a head taller each telling",
                    "lightning is lightning",
                    "ask who profits from the miracle",
                ],
                Trait::Gloomy => &[
                    "it will probably rain",
                    "good times never last",
                    "every roof leaks eventually",
                    "I expect little and am rarely wrong",
                    "the winter will be long, I feel it",
                ],
                Trait::Quiet => &[
                    "mm",
                    "the wind says enough",
                    "words wear out, silence does not",
                    "some things are better unsaid",
                    "hm",
                ],
                Trait::Glutton => &[
                    "I think about supper all day",
                    "one more helping, then",
                    "I dream of bread more than glory",
                    "the smell from the kitchen is torture",
                    "share is a strong word",
                ],
            });
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_one_is_their_own_opposite() {
        let mut rng = Rng::new(41);
        for _ in 0..500 {
            let traits = Traits::roll(&mut rng);
            assert!(traits.0.len() <= 2);
            let clash = (traits.has(Trait::Diligent) && traits.has(Trait::Slothful))
                || (traits.has(Trait::Devout) && traits.has(Trait::Skeptic))
                || (traits.has(Trait::Cheerful) && traits.has(Trait::Gloomy))
                || (traits.has(Trait::Chatty) && traits.has(Trait::Quiet));
            assert!(!clash, "rolled a contradiction: {:?}", traits.0);
        }
    }
}
