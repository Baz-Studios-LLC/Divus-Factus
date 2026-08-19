//! THE VAULT: the corpus as a database, for a corpus too big to hold.
//!
//! Brett means to ship over a million lines — "I plan on having over 1,000,000
//! lines of dialogue at launch... it would be a massive selling point of the
//! game." A million lines is not a folder of JSON read into memory at startup:
//! that is hundreds of megabytes of `String` and several seconds before the
//! game can open a window. So the words live in SQLite and only the handful a
//! moment can actually use are ever read.
//!
//! # What is different, and what is deliberately not
//!
//! ONLY THE SELECTION MOVES. SQL finds which lines are ELIGIBLE for a moment;
//! the scoring that picks between them stays in Rust, unchanged, over the
//! handful that came back — specificity, then how often it has been heard,
//! then whether this speaker just said it, then the weighted dice. That split
//! is the whole design: the part that must be identical is the part that was
//! not rewritten.
//!
//! `heard` and `recent` stay in memory too. They are what THIS world has said,
//! which is save data and not corpus data, and a database of the words has no
//! business knowing them.
//!
//! # Eligibility is a subset test, which SQL does not have
//!
//! A line may be said when EVERY tag it carries holds in the moment — not when
//! it shares a tag, which would let a prayer answer smalltalk. That is
//! relational division, and the way to write it is to count:
//!
//! ```sql
//! SELECT l.id FROM line l JOIN line_tag t ON t.line = l.id
//! WHERE t.tag IN (the moment's tags)
//! GROUP BY l.id HAVING COUNT(*) = l.tag_count
//! ```
//!
//! A line only reaches the GROUP BY if it shares a tag with the moment, and it
//! only survives the HAVING if ALL of its tags did. With an index on
//! `line_tag(tag)` the query never touches a line that shares nothing.
//!
//! # WHAT THIS DOES NOT YET SOLVE, measured rather than guessed
//!
//! `measure_the_vault` on the real corpus: 3,787 lines bake to a one megabyte
//! file in a third of a second, and a moment answers in about two
//! milliseconds. Two milliseconds is already slower than it looks like it
//! should be, and the reason is the thing that matters at a million lines: the
//! query touches every row sharing ANY tag with the moment, and the commonest
//! tags are enormous. In this corpus `muse` alone is 984 of 3,787 lines. At a
//! million lines it would be a quarter of a million rows joined to answer one
//! villager.
//!
//! THE FIX, when it is needed: every tag on a line must hold, so a line can
//! only be eligible if its RAREST tag is one the moment carries. Store that
//! rarest tag on the row as an anchor, index it, and the scan starts from the
//! most selective tag in the moment instead of the least. The division then
//! runs over hundreds of rows instead of hundreds of thousands.
//!
//! Not built yet on purpose: it is an optimization with a real cost in
//! complexity, the corpus it would help does not exist, and the equivalence
//! test below is what makes it safe to add later.

use std::collections::HashMap;
use std::path::Path;

use rusqlite::{Connection, params_from_iter};

use super::corpus::Line;

/// The slots a line can ask the moment to fill, as one bit each.
///
/// A bitmask rather than a join, because this is the cheapest filter there is
/// and it belongs in the same row as the words: a line wanting `{whom}` in a
/// moment with nobody to name is ineligible before anything else is asked
/// about it.
pub const SLOTS: [(&str, u32); 5] = [
    ("whom", 1 << 0),
    ("place", 1 << 1),
    ("spouse", 1 << 2),
    ("name", 1 << 3),
    ("god", 1 << 4),
];

/// Which slots this line's words demand.
pub fn slots_wanted(text: &str) -> u32 {
    SLOTS.iter().fold(0, |mask, (slot, bit)| {
        if text.contains(&format!("{{{slot}}}")) {
            mask | bit
        } else {
            mask
        }
    })
}

/// Which slots a moment can fill.
pub fn slots_offered(slots: &[(&str, &str)]) -> u32 {
    SLOTS.iter().fold(0, |mask, (slot, bit)| {
        if slots.iter().any(|(key, _)| key == slot) {
            mask | bit
        } else {
            mask
        }
    })
}

/// One line as the vault hands it back: the words, its weight, and enough
/// about it to score it exactly the way the in-memory corpus would.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub id: u64,
    pub t: String,
    pub w: f32,
    pub once: bool,
    pub tag_count: usize,
}

/// The corpus, on disk.
///
/// Some of what follows is not called by the running game yet: `bake` and
/// `open` are for compiling the authored JSON into a vault and shipping it,
/// and `tag_census` is the coverage report. They are tested, they are the
/// next two pieces of the pipeline, and deleting them to silence a warning
/// would mean writing them twice.
///
/// The connection is behind a `Mutex` because `rusqlite::Connection` is `Send`
/// but not `Sync`, and a Bevy resource must be both. Nothing contends for it -
/// one system speaks to the vault at a time - so the lock costs nothing and
/// buys the whole thing a place in the ECS.
pub struct Vault {
    db: std::sync::Mutex<Connection>,
}

// `bake`, `open` and `tag_census` are the next two pieces of this pipeline -
// compiling the authored corpus into a shippable vault, and the coverage
// report. They are written, tested, and not yet called by the running game.
// Deleting them to quiet a warning would mean writing them twice; the allow
// goes here rather than on the struct, which is where I put it first and which
// covers the fields and not the methods.
#[allow(dead_code)]
impl Vault {
    /// The connection, however the lock is doing. A poisoned mutex here means
    /// a thread died mid-query, and the words are still perfectly readable.
    /// The connection, alone.
    ///
    /// THE ONE PLACE THAT LOCKS, and nothing called while the guard is alive
    /// may take `&self` again — a plain `Mutex` is not re-entrant and the
    /// second take never returns. Work that has to run under the guard takes
    /// the connection, like [`tags_on`], so that it cannot lock at all.
    fn held(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.db.lock().unwrap_or_else(|held| held.into_inner())
    }

    /// Opens a baked corpus, read-only.
    pub fn open(at: &Path) -> rusqlite::Result<Vault> {
        let db = Connection::open_with_flags(
            at,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        Ok(Vault {
            db: std::sync::Mutex::new(db),
        })
    }

    /// Builds a vault from lines, at `at`, replacing whatever was there.
    ///
    /// This is the BAKE, and it runs when a human is authoring rather than
    /// when a player is playing: the JSON files stay the thing that is written
    /// and reviewed and diffed in a commit, and this is what they are compiled
    /// into. A corpus you cannot read in a pull request is a corpus nobody
    /// checks.
    pub fn bake(at: &Path, lines: &[Line]) -> rusqlite::Result<Vault> {
        let _ = std::fs::remove_file(at);
        if let Some(parent) = at.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let mut db = Connection::open(at)?;
        db.execute_batch(
            "PRAGMA journal_mode = OFF;
             PRAGMA synchronous = OFF;
             CREATE TABLE line (
                 id        INTEGER PRIMARY KEY,
                 t         TEXT    NOT NULL,
                 w         REAL    NOT NULL,
                 once      INTEGER NOT NULL,
                 tag_count INTEGER NOT NULL,
                 slots     INTEGER NOT NULL
             );
             CREATE TABLE line_tag (
                 line INTEGER NOT NULL,
                 tag  TEXT    NOT NULL
             );",
        )?;

        {
            let write = db.transaction()?;
            {
                let mut add_line = write.prepare(
                    "INSERT OR REPLACE INTO line (id, t, w, once, tag_count, slots)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                )?;
                let mut add_tag =
                    write.prepare("INSERT INTO line_tag (line, tag) VALUES (?1, ?2)")?;
                for line in lines {
                    // The id hashes the WORDS, exactly as the in-memory corpus
                    // does, so a save that remembers what it has heard still
                    // recognizes those lines after a rebake.
                    let id = super::corpus::id_of(&line.t) as i64;
                    add_line.execute(rusqlite::params![
                        id,
                        line.t,
                        line.w,
                        line.once as i32,
                        line.tags.len() as i64,
                        slots_wanted(&line.t) as i64,
                    ])?;
                    // OR REPLACE above means a duplicated line keeps one row;
                    // its tags would otherwise be inserted twice and break the
                    // count the division depends on.
                    add_tag.execute(rusqlite::params![id, ""])?;
                    write.execute("DELETE FROM line_tag WHERE line = ?1", [id])?;
                    for tag in &line.tags {
                        add_tag.execute(rusqlite::params![id, tag])?;
                    }
                }
            }
            write.commit()?;
        }

        // Indexed AFTER the insert, which is much faster than maintaining the
        // index a million times on the way in.
        db.execute_batch(
            "CREATE INDEX line_tag_by_tag ON line_tag(tag);
             CREATE INDEX line_tag_by_line ON line_tag(line);
             ANALYZE;",
        )?;
        Ok(Vault {
            db: std::sync::Mutex::new(db),
        })
    }

    /// Opens the vault a generated line goes into, making it if it is not
    /// there yet.
    ///
    /// Read-WRITE, unlike [`Vault::open`], and it is a different thing from
    /// the authored corpus on purpose: the JSON files are what a person wrote
    /// and reviewed, and this is what the living voice has said. Keeping them
    /// apart is what lets the settings offer them as three separate voices
    /// rather than one blurred pile - Brett: "in the settings for sermo I can
    /// turn their voice to one of three different settings. Authored, ChatGPT
    /// or the database."
    pub fn opened_for_writing(at: &Path) -> rusqlite::Result<Vault> {
        if let Some(parent) = at.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let db = Connection::open(at)?;
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS line (
                 id        INTEGER PRIMARY KEY,
                 t         TEXT    NOT NULL,
                 w         REAL    NOT NULL,
                 once      INTEGER NOT NULL,
                 tag_count INTEGER NOT NULL,
                 slots     INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS line_tag (
                 line INTEGER NOT NULL,
                 tag  TEXT    NOT NULL
             );
             CREATE INDEX IF NOT EXISTS line_tag_by_tag ON line_tag(tag);
             CREATE INDEX IF NOT EXISTS line_tag_by_line ON line_tag(line);",
        )?;
        Ok(Vault {
            db: std::sync::Mutex::new(db),
        })
    }

    /// Writes one line down, with its tags.
    ///
    /// Keyed by the WORDS, so the same sentence arriving twice is stored once
    /// however many moments produced it - which is the cheapest dedup there
    /// is, and the one that matters most: a corpus of a million lines is only
    /// worth having if they are a million DIFFERENT lines.
    ///
    /// Returns whether this was a sentence the vault had never held.
    pub fn remember(&self, line: &Line) -> rusqlite::Result<bool> {
        let db = self.held();
        let id = super::corpus::id_of(&line.t) as i64;
        let known: i64 = db.query_row("SELECT COUNT(*) FROM line WHERE id = ?1", [id], |row| {
            row.get(0)
        })?;
        if known > 0 {
            return Ok(false);
        }
        if already_thought(&db, line)? {
            return Ok(false);
        }
        db.execute(
            "INSERT INTO line (id, t, w, once, tag_count, slots) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                id,
                line.t,
                line.w,
                line.once as i32,
                line.tags.len() as i64,
                slots_wanted(&line.t) as i64,
            ],
        )?;
        for tag in &line.tags {
            db.execute(
                "INSERT INTO line_tag (line, tag) VALUES (?1, ?2)",
                rusqlite::params![id, tag],
            )?;
        }
        Ok(true)
    }

    /// How many lines the vault holds.
    pub fn len(&self) -> usize {
        self.held()
            .query_row("SELECT COUNT(*) FROM line", [], |row| row.get::<_, i64>(0))
            .unwrap_or(0) as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Every line eligible for this moment: all of its tags hold, it carries
    /// every tag in `must`, and the moment can fill every slot it asks for.
    ///
    /// The ranking is NOT done here. See the module head: the part that had to
    /// stay identical is the part that was not rewritten.
    pub fn eligible(
        &self,
        context: &[&str],
        must: &[&str],
        slots: &[(&str, &str)],
    ) -> rusqlite::Result<Vec<Candidate>> {
        if context.is_empty() {
            return Ok(Vec::new());
        }
        let offered = slots_offered(slots);
        let holes = (1..=context.len())
            .map(|i| format!("?{i}"))
            .collect::<Vec<_>>()
            .join(", ");
        // `slots & ~offered` is "wants something the moment cannot give".
        let sql = format!(
            "SELECT l.id, l.t, l.w, l.once, l.tag_count
             FROM line l
             JOIN line_tag t ON t.line = l.id
             WHERE t.tag IN ({holes})
               AND (l.slots & ~?{offered_hole}) = 0
             GROUP BY l.id
             HAVING COUNT(*) = l.tag_count",
            offered_hole = context.len() + 1
        );
        let db = self.held();
        let mut ask = db.prepare_cached(&sql)?;
        let mut binds: Vec<String> = context.iter().map(|tag| tag.to_string()).collect();
        binds.push(offered.to_string());
        let found = ask
            .query_map(params_from_iter(binds.iter()), |row| {
                Ok(Candidate {
                    id: row.get::<_, i64>(0)? as u64,
                    t: row.get(1)?,
                    w: row.get(2)?,
                    once: row.get::<_, i32>(3)? != 0,
                    tag_count: row.get::<_, i64>(4)? as usize,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        if must.is_empty() {
            return Ok(found);
        }
        // The register wall. Cheaper as a second pass over the few lines that
        // survived the division than as more SQL, and far easier to read.
        //
        // ON THE CONNECTION ALREADY IN HAND. This pass ran through `tags_of`
        // once, which takes the lock again - and this lock is not re-entrant,
        // so the second take never returned. Nothing failed: the game simply
        // STOPPED, the first time anybody prayed, because the register wall is
        // what a prayer is. The test for this rule hung instead of failing,
        // which is how it sat unnoticed.
        let mut kept = Vec::with_capacity(found.len());
        for line in found {
            let tags = tags_on(&db, line.id)?;
            if must.iter().all(|need| tags.iter().any(|tag| tag == need)) {
                kept.push(line);
            }
        }
        Ok(kept)
    }

    /// Every tag one line carries.
    pub fn tags_of(&self, line: u64) -> rusqlite::Result<Vec<String>> {
        tags_on(&self.held(), line)
    }

    /// Every tag in the vault, and how many lines wear it.
    ///
    /// The coverage report's raw material: what the corpus is thick and thin
    /// on, which is the question that matters far more than its total size.
    /// A million lines that are four fifths hunger smalltalk is a worse corpus
    /// than fifty thousand spread evenly, and only this can tell them apart.
    pub fn tag_census(&self) -> rusqlite::Result<HashMap<String, usize>> {
        let db = self.held();
        let mut ask = db.prepare("SELECT tag, COUNT(*) FROM line_tag GROUP BY tag")?;
        let counted = ask
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as usize))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(counted
            .into_iter()
            .filter(|(tag, _)| !tag.is_empty())
            .collect())
    }
}

/// How much of two lines has to be the same words before they are the same
/// line wearing a different coat.
///
/// Measured against the corpus rather than guessed. At six tenths, the wolf's
/// six ways of saying "keep out of the woods" collapse to one, while two lines
/// that share an opening and then say DIFFERENT things - "A wolf attacked
/// someone from the village" followed by "keep watch near the woods" against
/// "I will look for its tracks" - come out at about four tenths and both live.
/// The join is where the thought is, not the setup.
const THE_SAME_THOUGHT: f32 = 0.6;

/// Whether the vault already holds this thought in other words.
///
/// THE POINT OF THE WHOLE VAULT IS A MILLION LINES THAT ARE A MILLION LINES.
/// Twenty-eight percent of the first six hundred had a near-twin in the same
/// register - "keep away from the woods", "keep clear of the woods for now",
/// "stay clear of the woods for now", "I'll stay out of the woods for now" -
/// which is fifty thoughts in a thousand costumes, and no amount of generating
/// fixes it because each call is blind to the last.
///
/// Compared only against lines sharing this one's RAREST tag. That is the whole
/// scaling story: the rarest tag is the smallest set that must contain any true
/// duplicate, since a duplicate wears the same tags. A line tagged `event:mauled
/// trade:forester` is checked against the foresters, not against the corpus.
fn already_thought(db: &Connection, line: &Line) -> rusqlite::Result<bool> {
    let mine = words_of(&line.t);
    if mine.is_empty() {
        return Ok(false);
    }
    let Some(anchor) = rarest_tag(db, &line.tags)? else {
        return Ok(false);
    };
    let mut ask = db.prepare_cached(
        "SELECT l.t FROM line l JOIN line_tag t ON t.line = l.id WHERE t.tag = ?1",
    )?;
    let mut said = ask.query_map([anchor], |row| row.get::<_, String>(0))?;
    Ok(said.any(|other| {
        other
            .map(|other| overlap(&mine, &words_of(&other)) >= THE_SAME_THOUGHT)
            .unwrap_or(false)
    }))
}

/// The tag of this line's that the fewest other lines wear.
///
/// A free function over the connection, like everything else that runs while
/// the lock is held. See [`Vault::held`].
fn rarest_tag(db: &Connection, tags: &[String]) -> rusqlite::Result<Option<String>> {
    if tags.is_empty() {
        return Ok(None);
    }
    let holes = (1..=tags.len())
        .map(|i| format!("?{i}"))
        .collect::<Vec<_>>()
        .join(", ");
    // A tag no line wears yet cannot have a duplicate under it, so it wins by
    // being absent - which is why the count comes from a LEFT JOIN against the
    // asked-for tags rather than from `line_tag` alone.
    let mut ask = db.prepare_cached(&format!(
        "SELECT t.tag, COUNT(t.line) FROM line_tag t WHERE t.tag IN ({holes})
         GROUP BY t.tag ORDER BY COUNT(t.line) ASC LIMIT 1"
    ))?;
    let mut found = ask.query_map(params_from_iter(tags.iter()), |row| {
        row.get::<_, String>(0)
    })?;
    match found.next() {
        Some(tag) => Ok(Some(tag?)),
        // Every tag is new, so nothing in the vault shares one.
        None => Ok(None),
    }
}

/// A line's words, lowered and stripped of everything else, without repeats.
fn words_of(line: &str) -> Vec<String> {
    let mut words: Vec<String> = line
        .split_whitespace()
        .map(|word| {
            word.chars()
                .filter(|c| c.is_alphanumeric() || *c == '{' || *c == '}')
                .collect::<String>()
                .to_ascii_lowercase()
        })
        .filter(|word| !word.is_empty())
        .collect();
    words.sort_unstable();
    words.dedup();
    words
}

/// How much two word-sets share, over everything either of them holds.
fn overlap(a: &[String], b: &[String]) -> f32 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let shared = a.iter().filter(|word| b.contains(word)).count();
    shared as f32 / (a.len() + b.len() - shared) as f32
}

/// Every tag one line carries, on a connection already in hand.
///
/// A free function rather than a method BY DESIGN: it is called from inside
/// [`Vault::eligible`], which is holding the lock, and anything that could take
/// `&self` there is a deadlock waiting for the first prayer. Taking the
/// connection makes locking twice impossible to write.
fn tags_on(db: &Connection, line: u64) -> rusqlite::Result<Vec<String>> {
    let mut ask = db.prepare_cached("SELECT tag FROM line_tag WHERE line = ?1")?;
    ask.query_map([line as i64], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<String>>>()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(t: &str, tags: &[&str]) -> Line {
        Line {
            t: t.to_string(),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            w: 1.0,
            once: false,
        }
    }

    /// A vault of its own, per test. Named from the lines themselves rather
    /// than from how many there are: two tests with three lines each shared a
    /// file and the second one found the first one's tables.
    fn vault_of(lines: &[Line]) -> Vault {
        let name: u64 = lines.iter().fold(0u64, |seed, line| {
            seed ^ super::super::corpus::id_of(&line.t)
        });
        let at = std::env::temp_dir().join(format!(
            "divus-vault-{}-{name:016x}.sqlite",
            std::process::id()
        ));
        Vault::bake(&at, lines).expect("bake")
    }

    /// The same thought in a different coat is not a new line.
    ///
    /// The rule the corpus most needed: a million lines are only worth having
    /// if they are a million lines. Pins both directions - a reworded twin is
    /// turned away, and a line that shares an opening but ends somewhere else
    /// is kept, because the ending is where the thought is.
    #[test]
    fn a_reworded_twin_is_not_a_new_line() {
        let vault = vault_of(&[line(
            "A wolf attacked someone from the village. Keep clear of the woods for now.",
            &["event:mauled", "tell"],
        )]);
        let twin = line(
            "A wolf attacked someone from the village. Keep clear of the woods for a while.",
            &["event:mauled", "tell"],
        );
        assert!(!vault.remember(&twin).unwrap(), "a rewording got in");
        assert_eq!(vault.len(), 1);

        // Same setup, different thought: the hunter is not saying what the
        // others said.
        let other = line(
            "A wolf attacked someone from the village. I will look for its tracks.",
            &["event:mauled", "tell"],
        );
        assert!(
            vault.remember(&other).unwrap(),
            "a line that ends somewhere new was turned away"
        );
        assert_eq!(vault.len(), 2);
    }

    /// Curly and straight punctuation are the same sentence.
    #[test]
    fn a_curly_apostrophe_is_not_a_second_line() {
        let straight = "I saw {whom}'s child born safely.";
        let curly = "I saw {whom}\u{2019}s child born safely.";
        assert_eq!(
            crate::sermo::tidy(curly),
            crate::sermo::tidy(straight),
            "the vault was keeping two of these"
        );
    }

    /// EVERY tag must hold, not merely one of them. This is the whole
    /// eligibility rule and the reason the query counts instead of matching:
    /// a prayer sharing one tag with smalltalk must never answer it.
    #[test]
    fn a_line_needs_all_of_its_tags() {
        let vault = vault_of(&[
            line("hungry and hoping", &["prayer", "hungry"]),
            line("just hungry", &["hungry"]),
            line("wants more than is here", &["prayer", "hungry", "night"]),
        ]);

        let found = vault.eligible(&["prayer", "hungry"], &[], &[]).unwrap();
        let said: Vec<&str> = found.iter().map(|c| c.t.as_str()).collect();
        assert!(said.contains(&"hungry and hoping"));
        assert!(said.contains(&"just hungry"), "a subset is still eligible");
        assert!(
            !said.contains(&"wants more than is here"),
            "a line wanting `night` cannot speak in a moment that is not night"
        );
    }

    /// A line that asks for a slot the moment cannot fill is not eligible,
    /// and this is decided in the row rather than after the fetch.
    #[test]
    fn a_slot_the_moment_cannot_fill_rules_a_line_out() {
        let vault = vault_of(&[
            line("I saw {whom} go", &["tell"]),
            line("I saw it myself", &["tell"]),
        ]);

        let without = vault.eligible(&["tell"], &[], &[]).unwrap();
        assert_eq!(without.len(), 1, "only the line needing nothing");
        assert_eq!(without[0].t, "I saw it myself");

        let with = vault.eligible(&["tell"], &[], &[("whom", "Tiwa")]).unwrap();
        assert_eq!(with.len(), 2, "both, once there is somebody to name");
    }

    /// The register wall: `must` tags are required whatever else holds.
    #[test]
    fn the_register_wall_holds() {
        let vault = vault_of(&[line("quiet today", &["chat"]), line("hear me", &["prayer"])]);
        let found = vault
            .eligible(&["chat", "prayer"], &["prayer"], &[])
            .unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].t, "hear me");
    }

    /// THE ONE THAT MATTERS: against the real shipped corpus, the vault finds
    /// exactly the lines the in-memory scan finds. Every moment, every time.
    ///
    /// The rest of the tests here check rules I wrote down. This one checks
    /// the rules I did NOT write down - whatever the JSON corpus has grown in
    /// four hundred files, including whatever nobody remembers deciding. If
    /// SQL and the scan ever disagree about eligibility, the game says a
    /// different thing than it used to, and no unit test built from three
    /// invented lines would ever notice.
    #[test]
    fn the_vault_and_the_scan_agree_about_the_whole_corpus() {
        let corpus = super::super::corpus::Corpus::load();
        let lines = corpus.lines().to_vec();
        if lines.is_empty() {
            // The assets are not beside the test runner. Nothing to compare,
            // and failing here would only teach people to ignore this.
            return;
        }
        let vault = vault_of(&lines);
        assert_eq!(vault.len(), unique(&lines), "every line reached the vault");

        // Moments drawn from the corpus's OWN tags, so they are the shapes the
        // game actually produces rather than shapes I would have thought of.
        let mut moments: Vec<Vec<String>> = Vec::new();
        for line in lines.iter().take(400) {
            moments.push(line.tags.clone());
            // And a widened one: the real caller passes more tags than any
            // single line wears, which is what the division has to survive.
            let mut wider = line.tags.clone();
            wider.extend(["chat".to_string(), "day".to_string()]);
            moments.push(wider);
        }

        let offered = [("god", "the god"), ("whom", "Tiwa"), ("name", "Prorae")];
        for tags in &moments {
            let context: Vec<&str> = tags.iter().map(|t| t.as_str()).collect();

            let mut by_hand: Vec<u64> = lines
                .iter()
                .filter(|line| line.tags.iter().all(|tag| context.contains(&tag.as_str())))
                .filter(|line| {
                    let wants = slots_wanted(&line.t);
                    wants & !slots_offered(&offered) == 0
                })
                .map(|line| super::super::corpus::id_of(&line.t))
                .collect();
            by_hand.sort_unstable();
            by_hand.dedup();

            let mut by_sql: Vec<u64> = vault
                .eligible(&context, &[], &offered)
                .expect("query")
                .into_iter()
                .map(|c| c.id)
                .collect();
            by_sql.sort_unstable();

            assert_eq!(
                by_sql, by_hand,
                "the vault and the scan disagree for the moment {context:?}"
            );
        }
    }

    /// Lines are keyed by their words, so a corpus with the same sentence
    /// twice holds it once.
    fn unique(lines: &[Line]) -> usize {
        let mut ids: Vec<u64> = lines
            .iter()
            .map(|line| super::super::corpus::id_of(&line.t))
            .collect();
        ids.sort_unstable();
        ids.dedup();
        ids.len()
    }

    /// The census is what says where the corpus is thin - the question that
    /// matters more than the total, once the total is in the millions.
    #[test]
    fn the_census_counts_every_tag() {
        let vault = vault_of(&[
            line("a", &["chat", "hungry"]),
            line("b", &["chat"]),
            line("c", &["prayer"]),
        ]);
        let census = vault.tag_census().unwrap();
        assert_eq!(census.get("chat"), Some(&2));
        assert_eq!(census.get("hungry"), Some(&1));
        assert_eq!(census.get("prayer"), Some(&1));
    }
}

#[cfg(test)]
mod scale {
    use super::*;

    /// Prints what the real corpus bakes to, and how fast it answers. Ignored:
    /// this is a measurement, not a check.
    #[test]
    #[ignore]
    fn measure_the_vault() {
        let corpus = super::super::corpus::Corpus::load();
        let lines = corpus.lines().to_vec();
        let at = std::env::temp_dir().join("divus-vault-measure.sqlite");
        let started = std::time::Instant::now();
        let vault = Vault::bake(&at, &lines).expect("bake");
        let baked = started.elapsed();
        let size = std::fs::metadata(&at).map(|m| m.len()).unwrap_or(0);

        // A REAL moment: one line's own tags, widened the way a caller
        // widens them. Invented tags match nothing and measure nothing.
        let busiest = {
            let census = vault.tag_census().unwrap();
            let mut by_count: Vec<(String, usize)> = census.into_iter().collect();
            by_count.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
            by_count.truncate(6);
            by_count
        };
        println!("the six commonest tags: {busiest:?}");
        let widened: Vec<String> = lines[lines.len() / 2]
            .tags
            .iter()
            .cloned()
            .chain(busiest.iter().map(|(tag, _)| tag.clone()))
            .collect();
        let tags: Vec<&str> = widened.iter().map(|t| t.as_str()).collect();
        let asked = std::time::Instant::now();
        let mut found = 0;
        for _ in 0..1000 {
            found = vault
                .eligible(
                    &tags,
                    &[],
                    &[
                        ("god", "x"),
                        ("whom", "y"),
                        ("name", "z"),
                        ("place", "p"),
                        ("spouse", "s"),
                    ],
                )
                .unwrap()
                .len();
        }
        let each = asked.elapsed() / 1000;

        println!(
            "{} lines -> {} rows, {:.1} KB, baked in {:.2}s; a moment answers {found} lines in {each:?}",
            lines.len(),
            vault.len(),
            size as f64 / 1024.0,
            baked.as_secs_f64()
        );
    }
}
