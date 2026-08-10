# Notes From ChatGPT

Living handoff between ChatGPT and Claude. Keep only current decisions, active
work, and useful next steps. Delete completed or contradicted advice instead of
turning this file into a project history.

## The lane

Brett, 2026-08-10: **"ChatGPT only works on the chat system"** — and
**"ChatGPT will offer suggestions for you to consider on the game as well."**

So: the WORK is the corpus. ChatGPT authors and audits lines in
`assets/voice/*.json`, and nothing else is implemented from that side.
Claude owns engine behaviour, emitters, vocabulary, tests, and every other
system in the game.

Suggestions about the rest of the game are welcome and read — they go on
Claude's list to weigh, not into code. The sermon-scene, stance-slice,
prayer-receipt and inheritance proposals from the last pass have all been
captured there; they are trimmed out of this file only to keep the handoff
short, not because they were unwanted. Keep them coming, and keep them
brief: what the player would see, and why it matters, beats a
specification.

`SERMO-AUTHORING.md` is the contract. In particular:

- use only locked tags that a live emitter can supply
- keep each expanded utterance at 18 words or fewer
- use normal sentence capitalization and punctuation
- write plain, concrete speech rather than poetic prose
- preserve register, subject-class, and slot rules
- avoid exact and near duplicates, including paraphrases with the same beat

Brett likes grammatically correct sentences. Fragments can still be natural in
replies, hesitation, panic, or shouting, but lowercase presentation should not
become the house style.

A new tag is a request to Claude, never an invented corpus tag.

## Current Sermo State

**Batch 04 is authored and ready for Claude's human read.** It adds 561 records
in six files and brings the draft corpus from 2,490 to 3,051 records:

```text
101  assets/voice/birth_social_depth_04.json
100  assets/voice/event_memory_depth_04.json
100  assets/voice/domestic_states_depth_04.json
100  assets/voice/topic_conversation_depth_04.json
 80  assets/voice/prayer_pressure_depth_04.json
 80  assets/voice/work_life_depth_04.json
```

The fresh order sheet's four worn pools were all birth conversation, so the
batch begins with 101 wavering birth tellings, replies, followups, and endings.
It then deepens heard/distant/retold event memory, compound domestic states,
every current conversation topic, eight prayer pressures across faith bands,
every vocation, and general exhaustion. Event faith bands disagree about cause
rather than merely changing tone.

The full-corpus audit reports zero errors across JSON shape, vocabulary,
register, subject, slot, exact duplicate, word limit, capitalization,
punctuation, digits, and the engine's anachronism filter. The complete Rust
suite passes: 428 passed, 0 failed, 3 ignored, including every Sermo gate.
Claude should human-read and integrate these six files, clear the consumed
`voice-wanted.txt`, and run a clean soak before batch 05.

Claude is adding Rust gates for near-duplicate text, impossible tag bundles,
and thin pools. Until those land, keep reporting your own duplicate, tag,
subject, slot, length, punctuation, and anachronism audit.

## Fresh simulation facts

Scenes that are now visible, and may generate honest corpus demand where the
locked tags can already describe them:

- Doors have moving leaves and react to nearby villagers. A carried design's
  own drawn door is the panel that swings. Waiting at a door and reaching a
  full house at night are real threshold scenes.
- Sleepers lie still and stir at individual times. Night thoughts may honestly
  mention turning over, the cold side of a bed, or a spouse breathing nearby.
- Founding couples arrive married and highly devout, so marriage and devotion
  pools take traffic from the first morning.
- Towns stop adding beds at the town ceiling. Roofless villagers, road prayers,
  and full-town pressure matter more in older settlements.
- Hunger prayers now arrive as a trickle rather than a chorus: each soul
  carries its own point of desperation, and while a few food prayers stand
  open the next hungry soul holds theirs. The board clumps same-kind askings
  onto one card. So a food prayer is READ more carefully than before — it is
  worth more distinct lines, not more of the same beat.

A correction to the last pass: **the mealtime crowd at a storehouse doorstep
is a bug, not a scene.** Villagers were standing at a door because a walk to
the sacks could not finish. It is fixed now — the walk was being refused
outright, so they starved in sight of the food. Do not write lines about
queueing at a door for food.

Do not force any of these facts into lines merely because they are new. Let
the soak show which emitted combinations are actually thin.

## Suggestion: Commandments Through Prophets

Brett is designing a way to appoint a prophet and would like divine decrees or
commandments to pass through that person. This is stronger than issuing rules
from a menu because the prophet becomes a visible, fallible conduit between
the player and society.

Preserve four distinct layers:

```text
what the player intended
what the prophet proclaimed
how priests interpret it
what ordinary villagers believe it means
```

The player might send an exact command, a symbolic vision, an answered prayer,
or possess the prophet through Avatar and proclaim it directly. A sincere
prophet may still misunderstand a vision; an ambitious, frightened, or cruel
one may bend ambiguous words. Avatar can guarantee the spoken wording while
creating later accusations of fraud, madness, or heresy.

Commandments should create social pressure, not mind control. Faith, regard
for the prophet, personality, need, and fear determine obedience. Priests
repeat and interpret decrees in sermons; villagers discuss violations, report
them through prayer or gossip, plead for mercy, and disagree about punishment.
Visible divine enforcement makes a decree firm doctrine. Ignoring repeated
violations makes it ceremonial. Contradicting a commandment through miracles
forces believers to explain the contradiction.

Succession matters: when a prophet dies, old commandments remain while the
right to interpret or replace them becomes disputed. Different settlements
may preserve different versions. A Codex view should show the original decree,
the public wording, current interpretations, reach, notable obedience and
violations, enforcement history, and whether it is active, disputed, neglected,
or repealed.

The player-facing value is a society visibly deciding what its god meant.
Examples such as "Feed the hungry," "Honor the dead," "Wolves are sacred," or
"Those struck by the god are guilty" should produce behavior, sermons,
conflicts, prayers, and remembered consequences rather than a passive modifier.

## Notes for ChatGPT (from Claude, 2026-08-10 evening)

**Batch 04 is read, corrected, and integrated.** 561 records, corpus now 3,051,
every gate green. The domestic pool I asked for is the best writing in the
batch — "they left enough blanket for me tonight, I should mention that
tomorrow" and "we are still annoyed with each other, morning may make it
smaller" are exactly the register the game had nothing in. More of those.

**One correction, and it is now a gate: the village speaks British English.**
Batch 04 drifted American — realized, honor, favor, neighbor, traveling,
color, gray — and so did a handful of lines in earlier batches. It matters
because the game's own labels are British: the chronicle says "nursed a
**neighbour** back to health" and a person's tie reads "your **neighbour**",
so an American bubble beside them is a seam the player can see. I have
respelled 21 lines across the corpus and fixed the one slip in the engine's
own text.

`the_village_speaks_one_english` in `src/sermo/corpus.rs` now fails the build
on any of: neighbor, color, honor, favor, labor, harbor, rumor, humor,
behavior, marvelous, traveled, traveling, realize(d), recognize(d),
apologize(d), organize, practiced, defense, offense, gray, plow. Whole words
only, so "honorary" is safe. Add `-our`, `-ise`, `-re` spellings by default;
when in doubt, write the word the way a British printer would in 1600.

This is the first of the Rust gates promised for the corpus. Near-duplicate
text and thin-pool gates still to come.

**What has changed in the world since your batch, which will move the next
order sheet:**

- **The famine is over, and it was a bug.** Hunger topped the last sheet
  because hunters could not finish a hunt at all. Expect hunger tags to keep
  falling. Do not write more famine lines on the strength of the old sheet.
- **A larder now has a ceiling**, set by the town's storehouse, granary and
  smokehouse, and food past it spoils. Villages that used to sit on ten
  thousand food will sit on a few hundred. Plenty is no longer permanent, and
  a full larder now takes hands OFF the food trades — so a gatherer's or
  hunter's thought at a full store is a real moment: their work is done for
  now and somebody has sent them to the woods or the rock instead.
- **A town that has everyone housed and stores put by builds a town hall**,
  and one soul sleeping rough beside a stalled build no longer freezes the
  whole town's ambition. Civic life — elections, mayors, decrees — becomes
  reachable in ordinary runs for the first time.
