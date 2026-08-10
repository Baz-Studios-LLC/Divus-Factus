# Writing lines for SERMO

Sermo is the village's voice: every word a villager says, thinks, or prays
is picked from the hand-tagged corpus in `assets/voice/*.json`. This sheet
is the whole contract for writing new lines — it is written to be handed
to an outside drafter (ChatGPT drafts, humans judge; drafts never touch
code, only these JSON files, and every batch must pass the gates below).

## The voice, by example

Lowercase starts. Plain, concrete, everyday talk — never poetic fragments.
People talk about floors, pegs, nets, stew, drafts under doors. Study the
shots; imitate the shots, not a description of them:

- "swept the whole floor today. it's a small thing. it's my small thing"
- "my stomach's been {growling|complaining|at me} since I {woke up|got up}"
- "you can tell a good tree by the sound it makes when you knock it"
- "I want to believe, but wanting is not the same thing"
- "the stores are thin and the children eat first. you see how it is down here"
- "I saw it myself. {whom} went straight up into the air like a sack of feathers"

Keep every expanded utterance under eighteen words - brevity is the
voice's spine, and the limit is now part of the audit.

What always fails review: capital-P Poetry, sermons, modern idiom
("okay", "awesome"), fantasy-novel diction ("verily", "the gods willing"),
lines longer than ~2 short sentences, and any line that could not be said
by a tired person leaning on a fence.

## The record shape

```json
{ "t": "the line itself", "tags": ["muse", "hungry"] }
```

- `{a|b|c}` anywhere in `t` is an alternation — one is chosen per saying.
  Use them; they multiply one line into several.
- `{whom}` is a person's name slot. Only in `tell` lines, and only with
  `of:person`. Other slots (`{place}`, `{spouse}`, `{name}`, `{god}`)
  exist but ask before using them.
- `"once": true` marks a line said at most once per world. Rare.

## The registers (one per line, always)

- `muse` — a private thought. Add `prayer` if it is said kneeling, to the
  god ("you"). Prayers are the only lines addressed to anyone.
- `tell` — retelling a witnessed event. Requires an `event:*` tag and one
  of `saw` / `heard` / `distant` (how directly it reached the teller).
  Add `retold` for a story worn from many tellings.
- `reply` — an answer in conversation to something just heard.
- `chat:open` / `chat:followup` / `chat:reply` / `chat:end` — the beats of
  small talk, usually with a `topic:*`.
- `yell` — a scream or call; voiced, loud, short.

## The truth laws (these are load-bearing)

1. **Subject class.** Any `tell` about a physical act (`event:lifted`,
   `event:thrown`, `event:setdown`, `event:impact`) must say what KIND of
   thing it happened to: `of:person`, `of:beast`, or `of:thing` — and the
   words must match the tag. "somebody flew" is an `of:person` line; a
   berry bush is `of:thing` and no line about it may say "somebody".
   Sound-only lines (a crash heard) may stay untagged — a thud is honest
   about anything.
2. **Faith bands**: `devout` / `wavering` / `doubting` — the line's stance
   must match. A `doubting` teller shrugs; a `devout` one credits the god.
3. **Tags must be spoken.** Every tag must exist in the vocabulary below.
   The test `every_tag_is_one_the_game_speaks` fails the build otherwise.
   New tags require a code change — propose, don't invent.

## The vocabulary

registers: `muse` `tell` `reply` `yell` `prayer`
beats: `chat:open` `chat:followup` `chat:reply` `chat:end` `told`
hand: `saw` `heard` `distant` `retold`
faith: `devout` `wavering` `doubting`
states: `devotion` `grudge` `housed` `hungry` `hurt` `married` `night`
`road` `roof` `roofless` `storm` `wolf` `worn out`
subject: `of:person` `of:beast` `of:thing`
events: `event:delivered` (a birth) `event:flourished` `event:impact`
`event:lifted` `event:mauled` (wolf attack) `event:mended`
`event:perished` (a death) `event:provided` `event:quaked`
`event:setdown` `event:smote` `event:thrown` `event:uprooted`
trades (speaker's own): `trade:builder` `trade:cook` `trade:explorer`
`trade:farmer` `trade:fisher` `trade:forester` `trade:gatherer`
`trade:guard` `trade:healer` `trade:hunter` `trade:miner` `trade:priest`
topics (talked about): same list as `topic:*`, plus `topic:food`
`topic:roof` `topic:weather`

## What to write

`voice-wanted.txt` at the repo root is the live order book: the game
itself records every moment that went without words or wore its pool
thin, as the exact tag set that needed a line. Highest counts first.
More tags on a line = more specific = wins the moment; a line with only
`muse` is the last resort of a silent day, so write specific.

## The gates a batch must pass

1. `cargo test every_tag_is_one_the_game_speaks` — vocabulary.
2. `cargo test a_things_story_is_never_told_about_a_person` — truth.
3. Human read for voice. The shots above are the judge.
