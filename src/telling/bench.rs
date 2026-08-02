//! The voice bench: the corpus, on a command line, with no world around
//! it.
//!
//! Judging the writing inside the game is slow and awkward. You have to
//! find two villagers, get the camera near enough that a line is worth
//! composing at all, and then wait on the dice — three soaks went by this
//! afternoon just proving that conversations were happening. None of that
//! has anything to do with whether the words are any good.
//!
//! The corpus needs none of it: [`Corpus::pick`] is data in, string out,
//! with no world, no ECS and no camera. So this runs it directly.
//!
//! `divus-factus --voice`
//!
//! What the game still answers and this cannot: whether the RHYTHM feels
//! like talking, and whether the right subject came up at the right
//! moment. Read the lines here; watch the timing there.

use crate::rng::Rng;
use crate::telling::corpus::Corpus;
use std::io::{BufRead, Write};

/// Runs the bench until the operator is done, then leaves.
pub fn run() {
    let mut voice = Corpus::load();
    let mut dice = Rng::new(0x0_1CE);
    println!(
        "the voice bench - {} lines, {} utterances",
        voice.len(),
        voice.utterances()
    );
    println!("type tags to hear a line, `help` for the rest, `quit` to go");

    let stdin = std::io::stdin();
    let mut speaker = 1u64;
    loop {
        print!("\n> ");
        let _ = std::io::stdout().flush();
        let mut asked = String::new();
        if stdin.lock().read_line(&mut asked).unwrap_or(0) == 0 {
            return;
        }
        let asked = asked.trim();
        let (head, rest) = asked.split_once(' ').unwrap_or((asked, ""));
        match head {
            "" => {}
            "quit" | "q" | "exit" => return,
            "help" | "?" => help(),
            "audit" => audit(&voice),
            "chat" => {
                let topic = if rest.is_empty() { "topic:food" } else { rest };
                exchange(&mut voice, &mut dice, topic, &mut speaker);
            }
            // Anything else is a tag list. `x8` anywhere in it asks for
            // eight of them, which is how you SEE a pool wearing thin
            // rather than being told it has four lines in it.
            _ => {
                let mut times = 1;
                let tags: Vec<&str> = asked
                    .split_whitespace()
                    .filter(|word| {
                        if let Some(n) = word.strip_prefix('x')
                            && let Ok(n) = n.parse::<usize>()
                        {
                            times = n.clamp(1, 200);
                            return false;
                        }
                        true
                    })
                    .collect();
                for _ in 0..times {
                    speaker = speaker.wrapping_add(1);
                    match voice.pick(speaker, &tags, &SLOTS, &mut dice) {
                        Some(said) => println!("  {said}"),
                        None => {
                            println!("  (nothing in the corpus fits those tags)");
                            break;
                        }
                    }
                }
            }
        }
    }
}

/// Stand-in facts, so lines with slots in them can be heard too.
const SLOTS: [(&str, &str); 5] = [
    ("whom", "Feitreh"),
    ("name", "Temewa"),
    ("god", "Speku"),
    ("place", "the long water"),
    ("spouse", "Shezirav"),
];

fn help() {
    println!(
        "  <tags...>        a line matching every tag
  <tags...> x12    twelve of them, to see a pool repeat
  chat <topic>     a whole four-beat exchange
  audit            what the corpus covers and what it does not
  quit

  tags: muse tell reply yell chat:open chat:reply chat:followup chat:end
        event:smote event:mauled ... topic:food topic:roof topic:weather
        topic:<trade> devout wavering doubting saw heard distant
        hungry \"worn out\" hurt roofless housed married prayer"
    );
}

/// Two invented villagers, four beats, printed as a script — which is
/// how you find out whether an exchange reads as people talking or as
/// two lines that happen to share a tag.
fn exchange(voice: &mut Corpus, dice: &mut Rng, topic: &str, speaker: &mut u64) {
    let a = {
        *speaker = speaker.wrapping_add(1);
        *speaker
    };
    let b = {
        *speaker = speaker.wrapping_add(1);
        *speaker
    };
    // A conversation about something one of them SAW opens with the
    // telling and is answered with a reply; only ordinary talk opens
    // with `chat:open`. Getting this wrong made the bench print "not
    // stopping, just saying hello" as the opening line of an account of
    // a mauling, which is exactly the kind of lie a bench must not tell.
    let told = topic.starts_with("event:");
    let beats: [(&str, u64, &str, &[&str]); 4] = if told {
        [
            ("A", a, "tell", &["tell", "saw"]),
            ("B", b, "reply", &["reply"]),
            ("A", a, "chat:followup", &["chat:followup", "told"]),
            ("B", b, "chat:end", &["chat:end"]),
        ]
    } else {
        [
            ("A", a, "chat:open", &["chat:open"]),
            ("B", b, "chat:reply", &["chat:reply"]),
            ("A", a, "chat:followup", &["chat:followup"]),
            ("B", b, "chat:end", &["chat:end"]),
        ]
    };
    println!("  [{topic}]");
    for (who, seed, role, base) in beats {
        let mut on_topic: Vec<&str> = base.to_vec();
        on_topic.push(topic);
        let said = voice
            .pick(seed, &on_topic, &SLOTS, dice)
            // A beat with nothing on topic falls back to the role alone,
            // exactly as the game does. A telling has no such fallback -
            // it either has words for that act or the written phrasing
            // answers instead, which is not the corpus's business.
            .or_else(|| voice.pick(seed, base, &SLOTS, dice));
        match said {
            Some(said) => println!("  {who}: {said}"),
            None => println!("  {who}: ...  ({role} has nothing for this)"),
        }
    }
}

/// What the corpus covers, and where it is thin. The checks that were
/// being run by hand in throwaway scripts, kept somewhere they can be
/// re-run.
fn audit(voice: &Corpus) {
    let lines = voice.lines();
    let mut by_tag: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for line in lines {
        for tag in &line.tags {
            *by_tag.entry(tag.as_str()).or_default() += 1;
        }
    }
    println!(
        "  {} authored lines -> {} utterances, {} distinct tags",
        lines.len(),
        voice.utterances(),
        by_tag.len()
    );

    // A pool is only reachable if some line needs NOTHING beyond the
    // role and the subject: anything narrower is a bonus, not a floor.
    // `tell` and `yell` are absent on purpose: a telling always has an
    // act behind it and a shout always has a trouble, so neither has an
    // unconditional form to want. Every other role can be reached with
    // nothing but the role, and must have something to say when it is.
    let roles = [
        "muse",
        "reply",
        "chat:open",
        "chat:reply",
        "chat:followup",
        "chat:end",
    ];
    println!("\n  thin pools (a role with few lines that always fit):");
    let mut all_well = true;
    for role in roles {
        let plain = lines
            .iter()
            .filter(|line| line.tags.len() == 1 && line.tags[0] == role)
            .count();
        if plain < 4 {
            println!("    {role:16} {plain} unconditional");
            all_well = false;
        }
    }
    if all_well {
        println!("    none");
    }

    println!("\n  every tag in use:");
    for (tag, count) in by_tag {
        println!("    {count:4}  {tag}");
    }
}
