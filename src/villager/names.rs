//! Name generation from phonotactic rules.
//!
//! Not a list of names, and not a list of syllables either — a phoneme inventory plus
//! rules about which sounds may sit next to which. Names are assembled one position
//! at a time and checked against those rules, so the space is combinatorial rather
//! than enumerated: tens of millions of possibilities out of a few dozen sounds.
//!
//! Each settlement also gets its own *language*: a weighted subset of the inventory,
//! fixed for that community. Two villages therefore sound different from each other
//! while sounding internally consistent, which is most of what makes a name feel like
//! it belongs to a people rather than to a random number generator.

use crate::rng::Rng;

/// Consonants that may open a syllable, grouped by how they behave in clusters.
const STOPS: &[&str] = &["p", "t", "k", "b", "d", "g"];
const FRICATIVES: &[&str] = &["f", "s", "v", "z", "th", "sh", "h"];
const NASALS: &[&str] = &["m", "n"];
const LIQUIDS: &[&str] = &["l", "r"];
const GLIDES: &[&str] = &["w", "y"];

/// Vowel nuclei. Diphthongs are included as units because they behave as one.
const VOWELS: &[&str] = &["a", "e", "i", "o", "u", "ae", "ei", "ou", "ia", "yr"];

/// The sound system of one community.
///
/// Holding this per settlement is what stops every village in the world sounding
/// identical. It is a *subset* of the full inventory — languages are defined as much
/// by the sounds they lack as the ones they have.
#[derive(Debug, Clone)]
pub struct Language {
    onsets: Vec<String>,
    nuclei: Vec<String>,
    codas: Vec<String>,
    /// Chance a syllable takes a coda at all. Low values give open, flowing names.
    coda_chance: f32,
    /// Chance of a third syllable.
    long_name_chance: f32,
    /// The vowel feminine names favour ending on.
    feminine_ending: String,
}

impl Language {
    /// Rolls a language: which sounds this people use and how they combine them.
    pub fn random(rng: &mut Rng) -> Self {
        let mut onsets = Vec::new();

        // Single consonants. Every language keeps most, but drops a few — the gaps
        // are what give it character.
        for group in [STOPS, FRICATIVES, NASALS, LIQUIDS, GLIDES] {
            for sound in group {
                if rng.chance(0.75) {
                    onsets.push((*sound).to_string());
                }
            }
        }

        // Clusters, built by rule rather than listed — but only the friendly
        // shapes. Earlier drafts allowed any fricative before a liquid, and the
        // result was villages full of Vliafzlos and Zlokese: technically
        // generable, practically unpronounceable.
        if rng.chance(0.7) {
            for first in STOPS.iter().chain(["f"].iter()) {
                for second in LIQUIDS {
                    // "tl" and "dl" are awkward in most languages; skip them.
                    if matches!(*first, "t" | "d") && *second == "l" {
                        continue;
                    }
                    if rng.chance(0.45) {
                        onsets.push(format!("{first}{second}"));
                    }
                }
            }
        }

        // "s" may precede an unvoiced stop, giving "sk", "st" and "sp".
        if rng.chance(0.5) {
            for stop in ["p", "t", "k"] {
                if rng.chance(0.5) {
                    onsets.push(format!("s{stop}"));
                }
            }
        }

        // Plain vowels are the backbone; diphthongs are seasoning. Too many
        // diphthongs against clusters and codas is what made names unreadable.
        let mut nuclei: Vec<String> = VOWELS
            .iter()
            .filter(|v| {
                if v.len() == 1 {
                    rng.chance(0.9)
                } else {
                    rng.chance(0.4)
                }
            })
            .map(|v| (*v).to_string())
            .collect();

        // Codas are drawn from the sounds that can close a syllable: no glides, and
        // clusters only of a liquid or nasal plus a stop.
        let mut codas = Vec::new();
        for group in [STOPS, FRICATIVES, NASALS, LIQUIDS] {
            for sound in group {
                if rng.chance(0.6) {
                    codas.push((*sound).to_string());
                }
            }
        }
        if rng.chance(0.6) {
            for first in LIQUIDS.iter().chain(NASALS.iter()) {
                for stop in STOPS {
                    if rng.chance(0.3) {
                        codas.push(format!("{first}{stop}"));
                    }
                }
            }
        }

        // Guarantee a workable inventory however the dice fell.
        if onsets.is_empty() {
            onsets.push("t".into());
        }
        if nuclei.is_empty() {
            nuclei.push("a".into());
        }
        if codas.is_empty() {
            codas.push("n".into());
        }

        // The vowel womanly names in this tongue end on.
        let feminine_ending = ["a", "e", "i"][rng.range_i(0, 2) as usize].to_string();

        Language {
            onsets,
            nuclei,
            codas,
            coda_chance: rng.range(0.2, 0.5),
            long_name_chance: rng.range(0.1, 0.35),
            feminine_ending,
        }
    }

    /// Builds one name with no particular gender — places, gods, the language
    /// speaking about itself.
    pub fn name(&self, rng: &mut Rng) -> String {
        self.build(rng, 0.0)
    }

    /// Builds a name for a person. The same language, inflected: masculine
    /// names lean on closed, consonant endings; feminine names end open, on
    /// the language's chosen vowel.
    pub fn name_for(&self, sex: crate::creature::genome::Sex, rng: &mut Rng) -> String {
        match sex {
            crate::creature::genome::Sex::Male => self.build(rng, 0.3),
            crate::creature::genome::Sex::Female => {
                let mut name = self.build(rng, -1.0);
                if name
                    .chars()
                    .last()
                    .is_some_and(|c| !"aeiouy".contains(c.to_ascii_lowercase()))
                {
                    name.push_str(&self.feminine_ending);
                }
                name
            }
        }
    }

    /// Assembles a name; `final_coda_bias` shifts how readily it ends closed.
    fn build(&self, rng: &mut Rng, final_coda_bias: f32) -> String {
        let syllables = if rng.chance(self.long_name_chance) {
            3
        } else {
            2
        };

        let mut name = String::new();
        let mut previous_coda: Option<&str> = None;
        // One cluster per name. Two is a tongue-twister every time.
        let mut cluster_spent = false;

        for index in 0..syllables {
            let onset = self.choose_onset(rng, previous_coda, !cluster_spent);
            if onset.len() > 1 {
                cluster_spent = true;
            }
            name.push_str(&onset);
            let nucleus = rng.pick(&self.nuclei);
            name.push_str(nucleus);

            // The last syllable takes a coda more readily; names ending on a vowel
            // every time sound uniformly soft. A diphthong already fills the
            // mouth — it takes no coda after it.
            let wants_coda = if nucleus.len() > 1 {
                false
            } else if index == syllables - 1 {
                rng.chance((self.coda_chance + 0.2 + final_coda_bias).clamp(0.0, 1.0))
            } else {
                rng.chance(self.coda_chance * 0.5)
            };

            previous_coda = if wants_coda {
                let coda = rng.pick(&self.codas);
                name.push_str(coda);
                Some(coda)
            } else {
                None
            };
        }

        capitalise(&name)
    }

    /// Picks an onset that does not collide with the previous syllable's coda.
    ///
    /// A name like "Tarrus" reads fine; "Tarrrus" does not. Rejecting the collision
    /// rather than forbidding sounds outright keeps the inventory intact.
    fn choose_onset(
        &self,
        rng: &mut Rng,
        previous_coda: Option<&str>,
        allow_cluster: bool,
    ) -> String {
        for _ in 0..8 {
            let onset = rng.pick(&self.onsets);
            if !allow_cluster && onset.len() > 1 {
                continue;
            }
            let Some(coda) = previous_coda else {
                return onset.clone();
            };

            // No repeated sound across the boundary, and no stacking three
            // consonants where a cluster meets a cluster.
            let doubled = coda.ends_with(&onset[..1]);
            let crowded = coda.len() + onset.len() > 3;
            if !doubled && !crowded {
                return onset.clone();
            }
        }
        // Fall back to a vowel-initial syllable rather than forcing a bad join.
        String::new()
    }
}

fn capitalise(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::from("Ael"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::creature::genome::Sex;

    #[test]
    fn names_never_pile_up_consonants() {
        let mut rng = Rng::new(7);
        for _ in 0..30 {
            let language = Language::random(&mut rng);
            for _ in 0..60 {
                let name = language.name(&mut rng);
                let mut run = 0;
                let mut worst = 0;
                for c in name.to_ascii_lowercase().chars() {
                    if "aeiouy".contains(c) {
                        run = 0;
                    } else {
                        run += 1;
                        worst = worst.max(run);
                    }
                }
                assert!(worst <= 3, "{name} is a tongue-twister");
            }
        }
    }

    #[test]
    fn the_sexes_sound_different() {
        let mut rng = Rng::new(9);
        let language = Language::random(&mut rng);
        let open = |name: String| {
            name.chars()
                .last()
                .is_some_and(|c| "aeiouy".contains(c.to_ascii_lowercase()))
        };

        let feminine_open = (0..120)
            .filter(|_| open(language.name_for(Sex::Female, &mut rng)))
            .count();
        let masculine_open = (0..120)
            .filter(|_| open(language.name_for(Sex::Male, &mut rng)))
            .count();

        assert!(feminine_open >= 118, "feminine names should end open");
        assert!(
            masculine_open < feminine_open,
            "masculine names should close more often",
        );
    }
    use std::collections::HashSet;

    #[test]
    fn names_are_stable_for_a_seed() {
        let language = Language::random(&mut Rng::new(42));
        let a = language.name(&mut Rng::new(7));
        let b = language.name(&mut Rng::new(7));
        assert_eq!(a, b);
    }

    #[test]
    fn names_are_pronounceable_shapes() {
        let mut rng = Rng::new(1);
        for seed in 0..40 {
            let language = Language::random(&mut Rng::new(seed));
            for _ in 0..200 {
                let name = language.name(&mut rng);

                assert!(name.len() >= 2, "{name} is too short");
                assert!(name.len() <= 16, "{name} is too long");
                assert!(name.chars().all(|c| c.is_ascii_alphabetic()), "{name}");
                assert!(name.chars().next().is_some_and(|c| c.is_uppercase()));

                // No sound repeated three times running, which is the main way
                // rule-built names go wrong.
                let lower = name.to_lowercase();
                let bytes = lower.as_bytes();
                for i in 2..bytes.len() {
                    assert!(
                        !(bytes[i] == bytes[i - 1] && bytes[i] == bytes[i - 2]),
                        "{name} has a tripled letter",
                    );
                }
            }
        }
    }

    #[test]
    fn the_name_space_is_enormous() {
        // The point of rules over a list: the space should be combinatorial. A
        // hundred thousand draws from one language should barely repeat.
        let language = Language::random(&mut Rng::new(5));
        let mut rng = Rng::new(11);
        let mut seen = HashSet::new();
        let draws = 100_000;

        for _ in 0..draws {
            seen.insert(language.name(&mut rng));
        }

        assert!(
            seen.len() > draws * 6 / 10,
            "only {} distinct names in {draws} draws",
            seen.len(),
        );
    }

    #[test]
    fn different_peoples_sound_different() {
        // Two settlements should not produce interchangeable names.
        let mut rng = Rng::new(3);
        let first = Language::random(&mut Rng::new(100));
        let second = Language::random(&mut Rng::new(200));

        let a: HashSet<String> = (0..400).map(|_| first.name(&mut rng)).collect();
        let b: HashSet<String> = (0..400).map(|_| second.name(&mut rng)).collect();

        let shared = a.intersection(&b).count();
        assert!(
            shared * 20 < a.len(),
            "{shared} names shared between two languages",
        );
    }

    #[test]
    fn a_language_always_has_a_usable_inventory() {
        // However the dice fall, a language must be able to build a name.
        for seed in 0..500 {
            let language = Language::random(&mut Rng::new(seed));
            assert!(!language.onsets.is_empty());
            assert!(!language.nuclei.is_empty());
            assert!(!language.codas.is_empty());
            assert!(!language.name(&mut Rng::new(seed)).is_empty());
        }
    }

    #[test]
    fn a_village_rarely_holds_two_of_the_same_name() {
        let language = Language::random(&mut Rng::new(4));
        let mut rng = Rng::new(7);
        let mut collisions = 0;

        for _ in 0..300 {
            let mut seen = HashSet::new();
            for _ in 0..12 {
                if !seen.insert(language.name(&mut rng)) {
                    collisions += 1;
                }
            }
        }
        assert!(
            collisions < 30,
            "{collisions} duplicates across 300 villages"
        );
    }
}
