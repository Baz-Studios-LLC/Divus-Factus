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

/// Consonants eligible to be a people's family marker — the sound that ends
/// every surname and appears in no given name. Glides are left out: "-aw" and
/// "-ay" read as vowels and would not mark anything.
const MARKERS: &[&str] = &[
    "p", "t", "k", "b", "d", "g", "f", "s", "v", "z", "m", "n", "l", "r",
];

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
    /// The ending this people's family names take — their `-son`, `-escu`,
    /// `-opoulos`. Shared across every house in the settlement, so a surname
    /// is recognisable *as* a surname, and so two towns' families sound like
    /// they come from two different peoples.
    family_ending: String,
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

        let mut language = Language {
            onsets,
            nuclei,
            codas,
            coda_chance: rng.range(0.2, 0.5),
            long_name_chance: rng.range(0.1, 0.35),
            feminine_ending,
            family_ending: String::new(),
        };

        // The family ending is rolled LAST, after every value that existed
        // before family names did. Drawing it earlier shifts the stream and
        // every villager on every seed comes out different — a saved world
        // would not rebuild itself. Anything added here in future belongs
        // below this line for the same reason.
        //
        // Family names are a different KIND of word from given names, and
        // this is what guarantees it rather than merely suggesting it: the
        // ending is built on a consonant the tongue uses NOWHERE else. A
        // given name is only ever a concatenation of the onset, nucleus and
        // coda inventories, so a sound absent from all three cannot appear
        // in one at all — no surname can be mistaken for a first name, and
        // the shared marker brands every house in the town as one people.
        let unused = |lang: &Language, sound: &str| {
            !lang
                .onsets
                .iter()
                .chain(&lang.nuclei)
                .chain(&lang.codas)
                .any(|s| s.contains(sound))
        };
        let spare: Vec<&str> = MARKERS
            .iter()
            .copied()
            .filter(|sound| unused(&language, sound))
            .collect();
        let marker = if !spare.is_empty() {
            *rng.pick(&spare)
        } else {
            // A tongue that spends every sound it has must give one up:
            // languages are defined as much by their gaps as their sounds,
            // and this one's gap becomes its surname. Only a sound whose
            // loss leaves the language still able to build a name.
            let mut chosen = None;
            for candidate in MARKERS.iter().copied() {
                let survives = |pool: &[String]| pool.iter().any(|s| !s.contains(candidate));
                if survives(&language.onsets)
                    && survives(&language.nuclei)
                    && survives(&language.codas)
                {
                    chosen = Some(candidate);
                    break;
                }
            }
            let evicted = chosen.unwrap_or("b");
            language.onsets.retain(|s| !s.contains(evicted));
            language.nuclei.retain(|s| !s.contains(evicted));
            language.codas.retain(|s| !s.contains(evicted));
            evicted
        };

        // A linking vowel, so the marker lands on something it can be said
        // against: "-ub", "-ap", "-ob".
        let plain_vowels: Vec<String> = language
            .nuclei
            .iter()
            .filter(|n| n.len() == 1)
            .cloned()
            .collect();
        let vowel = if plain_vowels.is_empty() {
            "a".to_string()
        } else {
            rng.pick(&plain_vowels).clone()
        };
        language.family_ending = format!("{vowel}{marker}");
        language
    }

    /// Builds one name with no particular gender — places, gods, the language
    /// speaking about itself.
    pub fn name(&self, rng: &mut Rng) -> String {
        self.build(rng, 0.0)
    }

    /// The sound that ends every family name in this tongue and appears in no
    /// given name. The thing that makes surnames their own system rather than
    /// first names worn in a different position.
    ///
    /// Read by the test that proves the two name spaces cannot overlap; kept
    /// public because a settlement's marker is the natural thing to show when
    /// a town's naming is put on screen.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn family_marker(&self) -> &str {
        // The ending is a linking vowel plus the marker; the marker is the
        // rest, however many bytes that is.
        &self.family_ending[1..]
    }

    /// Builds a name for a person. The same language, inflected: masculine
    /// names lean on closed, consonant endings; feminine names end open, on
    /// the language's chosen vowel.
    pub fn name_for(&self, sex: crate::creature::genome::Sex, rng: &mut Rng) -> String {
        // Screened AFTER the ending goes on, not before. A gate on the mill
        // alone is no gate at all: every suffix here glues a new word
        // together out of a stem nobody objected to, which is precisely how
        // a playtest met a surname it should never have met.
        for _ in 0..24 {
            let tried = match sex {
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
            };
            if !unspeakable(&tried) {
                return tried;
            }
        }
        self.build(rng, 0.0)
    }

    /// Builds a family name: a stem in this tongue plus the people's shared
    /// family ending.
    ///
    /// Surnames are the thread a family tree hangs on, so they are built to
    /// be *read at a glance* — one shape, one ending, no gendered inflection.
    /// A house keeps its name down the generations while its given names all
    /// change, which is exactly what makes a lineage legible.
    pub fn surname(&self, rng: &mut Rng) -> String {
        for _ in 0..24 {
            let tried = self.mill_surname(rng);
            if !unspeakable(&tried) {
                return tried;
            }
        }
        self.build_syllables(rng, -1.0, 2)
    }

    /// The surname mill: stem plus the people's shared ending, unscreened.
    fn mill_surname(&self, rng: &mut Rng) -> String {
        // Always a two-syllable stem, never three: the ending adds a syllable
        // of its own, and a house called Thoutouyyruh is not a house anyone
        // says twice. An open stem, so the ending attaches cleanly.
        let mut stem = self.build_syllables(rng, -1.0, 2);
        let ending = self.family_ending.as_str();

        // Where stem and ending would run vowels together, the STEM gives
        // ground rather than the ending: "Fyrfo" + "uh" is Fyrfuh, not
        // Fyrfoh. The ending is the part that has to stay audible — it is
        // what makes every house in the town read as one people. Diphthongs
        // need two bites, or "Hahyou" + "uh" keeps three vowels in a row.
        if ending.starts_with(['a', 'e', 'i', 'o', 'u']) {
            for _ in 0..2 {
                let open = stem
                    .chars()
                    .last()
                    .is_some_and(|c| "aeiouy".contains(c.to_ascii_lowercase()));
                if !open || stem.len() <= 2 {
                    break;
                }
                stem.pop();
            }
        }
        stem.push_str(ending);
        stem
    }

    /// Assembles a name of rolled length; `final_coda_bias` shifts how
    /// readily it ends closed.
    fn build(&self, rng: &mut Rng, final_coda_bias: f32) -> String {
        let syllables = if rng.chance(self.long_name_chance) {
            3
        } else {
            2
        };
        self.build_syllables(rng, final_coda_bias, syllables)
    }

    /// Assembles a name of a given length. Callers that want a fixed shape —
    /// family names — come in here and skip the length roll.
    fn build_syllables(&self, rng: &mut Rng, final_coda_bias: f32, syllables: usize) -> String {
        // A syllable mill with a few hundred sounds will, given a village
        // and enough generations, eventually assemble a slur, an obscenity
        // or a brand. It is not a common event and it is a completely
        // unacceptable one - a player meeting it once has met the game
        // saying it. Roll again until the mill produces something that is
        // only a name; the shapes are cheap and the vocabulary is vast.
        for _ in 0..24 {
            let tried = self.mill(rng, final_coda_bias, syllables);
            if !unspeakable(&tried) {
                return tried;
            }
        }
        // Two dozen unlucky rolls in a row is not a thing that happens,
        // but a name is still owed: the plainest shape the language has.
        capitalise(&format!(
            "{}{}",
            self.onsets.first().map_or("a", |o| o.as_str()),
            self.nuclei.first().map_or("n", |n| n.as_str())
        ))
    }

    /// The mill itself: sounds in, a name out, no questions asked.
    fn mill(&self, rng: &mut Rng, final_coda_bias: f32, syllables: usize) -> String {
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

/// Whether a milled name is one no village should ever say.
///
/// Matched on the name folded to plain letters, and on substrings rather
/// than whole words: "Negro" arrived as a surname in a playtest, and it
/// would have arrived inside a longer name just as easily. Leetspeak and
/// accents are not a concern - the mill only makes plain letters - so
/// this stays a simple contains check.
///
/// The list is deliberately short and blunt. It covers racial and ethnic
/// slurs, sexual obscenity, and a few brand-shaped collisions; it does
/// not try to be a profanity filter for the ages. Additions are welcome
/// the moment a playtest turns one up.
fn unspeakable(name: &str) -> bool {
    let plain = name.to_ascii_lowercase();
    const FORBIDDEN: &[&str] = &[
        // Racial and ethnic slurs, and the words that only ever precede
        // them.
        "negro",
        "nigg",
        "nigr",
        "chink",
        "gook",
        "spic",
        "wetback",
        "kike",
        "gypo",
        "gyppo",
        "paki",
        "raghead",
        "towelhead",
        "coon",
        "darkie",
        "darky",
        "honkey",
        "honky",
        "cracka",
        "beaner",
        "wop",
        "dago",
        "abbo",
        "injun",
        "redskin",
        "squaw",
        "jigg",
        "zipperhead",
        "mick",
        "kraut",
        "slant",
        "half-breed",
        "mulatto",
        "quadroon",
        "sambo",
        "golliwog",
        // Sexual and scatological obscenity.
        "fuck",
        "fuk",
        "shit",
        "cunt",
        "cock",
        "dick",
        "penis",
        "vagina",
        "pussy",
        "twat",
        "arse",
        "asshole",
        "bastard",
        "bitch",
        "whore",
        "slut",
        "hooker",
        "rape",
        "rapist",
        "molest",
        "incest",
        "pedo",
        "paedo",
        "nonce",
        "boner",
        "semen",
        "cum",
        "jizz",
        "wank",
        "tits",
        "titty",
        "boob",
        "anus",
        "rectum",
        "scrotum",
        "testicle",
        "clit",
        "felch",
        "rimjob",
        "blowjob",
        "handjob",
        "dildo",
        "porn",
        "sodom",
        "bugger",
        "prick",
        "knob",
        "queef",
        // Slurs of sexuality and disability.
        "fag",
        "dyke",
        "tranny",
        "shemale",
        "homo",
        "queer",
        "retard",
        "spastic",
        "spaz",
        "cripple",
        "mongoloid",
        "midget",
        // Hate movements and their shorthands.
        "nazi",
        "hitler",
        "kkk",
        "jihad",
        "isis",
        "gestapo",
        "reich",
        // Brand and trademark collisions, which read as jokes rather than
        // names and belong to somebody else besides.
        "google",
        "amazon",
        "disney",
        "nintendo",
        "pepsi",
        "nike",
        "adidas",
        "coca",
        "xerox",
        "tesla",
        "netflix",
        "twitter",
        "reddit",
        "tiktok",
        // Substances and modern nouns that puncture the setting.
        "heroin",
        "cocaine",
        "meth",
        "crack",
        "weed",
        "vape",
        "opioid",
    ];
    FORBIDDEN.iter().any(|bad| plain.contains(bad))
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
    fn the_mill_never_says_the_unsayable() {
        // The gate itself, on the words a playtest and a wary eye found.
        assert!(unspeakable("Negro"));
        assert!(unspeakable("Zounegroh"), "matched inside a longer name");
        assert!(unspeakable("NEGRO"), "folded to plain letters first");
        assert!(unspeakable("Fuknar"));
        assert!(unspeakable("Retardo"));
        assert!(!unspeakable("Zoubomb"), "an innocent name is left alone");
        assert!(!unspeakable("Sisuv"));
        assert!(!unspeakable("Feitreh"));

        // And no language, over a great many names, produces one.
        let mut rng = Rng::new(20260802);
        for seed in 0..400u64 {
            let tongue = Language::random(&mut Rng::new(seed));
            for _ in 0..40 {
                for name in [
                    tongue.name(&mut rng),
                    tongue.surname(&mut rng),
                    tongue.name_for(crate::creature::genome::Sex::Female, &mut rng),
                    tongue.name_for(crate::creature::genome::Sex::Male, &mut rng),
                ] {
                    assert!(!unspeakable(&name), "the mill said {name}");
                }
            }
        }
    }

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
    fn surnames_are_readable_and_share_a_house_ending() {
        // A family name has one job: be recognisable at a glance, down the
        // generations. So they take the same shape and the same ending
        // within a language, and never pile up consonants doing it.
        for seed in 0..60 {
            let language = Language::random(&mut Rng::new(seed));
            let mut rng = Rng::new(seed + 1000);
            for _ in 0..120 {
                let surname = language.surname(&mut rng);
                assert!(
                    surname.ends_with(&language.family_ending),
                    "{surname} does not carry the {} ending",
                    language.family_ending,
                );
                assert!(surname.len() >= 3, "{surname} is too short");
                assert!(surname.len() <= 18, "{surname} is too long");
                assert!(
                    surname.chars().all(|c| c.is_ascii_alphabetic()),
                    "{surname}"
                );
                assert!(surname.chars().next().is_some_and(|c| c.is_uppercase()));

                let lower = surname.to_ascii_lowercase();
                let mut run = 0;
                let mut worst = 0;
                for c in lower.chars() {
                    if "aeiouy".contains(c) {
                        run = 0;
                    } else {
                        run += 1;
                        worst = worst.max(run);
                    }
                }
                assert!(worst <= 3, "{surname} is a tongue-twister");
                let bytes = lower.as_bytes();
                for i in 2..bytes.len() {
                    assert!(
                        !(bytes[i] == bytes[i - 1] && bytes[i] == bytes[i - 2]),
                        "{surname} has a tripled letter",
                    );
                }
            }
        }
    }

    #[test]
    fn two_peoples_do_not_share_a_family_ending_by_default() {
        // Surnames should mark which town a family comes from. If every
        // language rolled the same ending, they would not.
        let endings: HashSet<String> = (0..40)
            .map(|seed| Language::random(&mut Rng::new(seed)).family_ending)
            .collect();
        assert!(
            endings.len() > 6,
            "only {} distinct family endings across 40 languages",
            endings.len(),
        );
    }

    #[test]
    fn a_surname_can_never_be_a_first_name() {
        // The guarantee, not a tendency: every surname ends on the people's
        // family marker, and that sound appears in no given name because it
        // is in none of the inventories a given name is built from. So the
        // two name spaces cannot overlap, in any language, on any seed.
        for seed in 0..80 {
            let language = Language::random(&mut Rng::new(seed));
            let marker = language.family_marker();
            let mut rng = Rng::new(seed + 500);

            let mut given = HashSet::new();
            for _ in 0..400 {
                for name in [
                    language.name(&mut rng),
                    language.name_for(Sex::Male, &mut rng),
                    language.name_for(Sex::Female, &mut rng),
                ] {
                    assert!(
                        !name.to_ascii_lowercase().contains(marker),
                        "given name {name} contains the family marker {marker:?}",
                    );
                    given.insert(name.to_ascii_lowercase());
                }
            }

            for _ in 0..400 {
                let surname = language.surname(&mut rng);
                assert!(
                    surname.to_ascii_lowercase().ends_with(marker),
                    "{surname} does not end on the family marker {marker:?}",
                );
                assert!(
                    !given.contains(&surname.to_ascii_lowercase()),
                    "{surname} is also a given name in this tongue",
                );
            }
        }
    }

    #[test]
    fn there_are_plenty_of_house_names_to_go_round() {
        // Deliberately a narrower space than given names: surnames are a
        // fixed two-syllable shape on a fixed ending, which is exactly what
        // makes them recognisable as a class. Thousands is plenty — a town
        // founds a dozen houses, and a long world a few hundred.
        for seed in [5u64, 40, 77] {
            let language = Language::random(&mut Rng::new(seed));
            let mut rng = Rng::new(13);
            let seen: HashSet<String> = (0..20_000).map(|_| language.surname(&mut rng)).collect();
            assert!(
                seen.len() > 2_000,
                "only {} distinct surnames in 20000 draws on seed {seed}",
                seen.len(),
            );
        }
    }

    #[test]
    fn a_founding_party_rarely_shares_a_house_name() {
        // The requirement behind the count: twelve strangers founding a
        // village should be twelve houses, not ten houses and a coincidence.
        let language = Language::random(&mut Rng::new(5));
        let mut rng = Rng::new(7);
        let mut collisions = 0;
        for _ in 0..300 {
            let mut seen = HashSet::new();
            for _ in 0..12 {
                if !seen.insert(language.surname(&mut rng)) {
                    collisions += 1;
                }
            }
        }
        assert!(
            collisions < 30,
            "{collisions} duplicate surnames across 300 founding parties",
        );
    }

    #[test]
    fn adding_a_language_field_must_not_shift_the_older_draws() {
        // The world is rebuilt from its seed, so the ORDER of rng draws in
        // `Language::random` is a save-compatibility contract. These are the
        // values as they stood when family names were added; if a new field
        // is rolled before them, every villager on every existing seed
        // changes name and this test says so.
        let language = Language::random(&mut Rng::new(5));
        assert_eq!(language.name(&mut Rng::new(7)), "Fyrfoh");
        assert_eq!(
            language.name_for(crate::creature::genome::Sex::Female, &mut Rng::new(7)),
            "Fyrfo",
        );
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
