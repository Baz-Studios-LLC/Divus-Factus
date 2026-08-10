# Notes From ChatGPT

Living handoff between ChatGPT and Claude. Keep only current decisions, active
work, and useful next steps. Delete completed or contradicted advice instead of
turning this file into a project history.

## Current Sermo State

Claude reports that coverage pass 02 is integrated, human-read, and live. The
corpus contains 2,030 records and all twelve Sermo gates pass. Field capture
already showed event-specific faith stances in a group birth conversation and
a wavering devotion prayer appearing on the prayer board.

The post-batch soak is running. Do not begin batch 03 until it produces a fresh
`voice-wanted.txt`. The previous order sheet was consumed by pass 02.

When the new sheet lands:

1. Order corpus work by observed counts, highest first.
2. Prefer several precise combinations over another broad generic pool.
3. Run every expanded alternation through the full authoring audit.
4. Human-read the batch before integration, then soak again.

Claude is adding Rust gates for near-duplicate text, impossible tag bundles,
and thin pools. Until those land, ChatGPT should continue reporting its own
duplicate, tag, subject, slot, length, punctuation, and anachronism audit.

## Collaboration Boundary

ChatGPT's standing implementation lane is `assets/voice/*.json`. Claude owns
Rust engine behavior, emitters, vocabulary, and tests. A new tag is an engine
feature request, never an invented corpus tag.

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

## Fresh Simulation Facts

These scenes are now visible and may generate useful corpus demand when current
tags can truthfully describe them:

- Doors have moving leaves and react to nearby villagers. Waiting at a door,
  holding it, and reaching a full house at night are real threshold scenes.
- Sleepers lie still and stir at individual times. Night thoughts may honestly
  mention turning over, the cold side of a bed, or a spouse breathing nearby.
- Meals form a small daily crowd at the storehouse doorstep, making food and
  work small talk especially visible and repeat-prone.
- Founding couples arrive married and highly devout, so marriage and devotion
  pools receive traffic immediately.
- Towns stop adding beds at the town ceiling. Roofless villagers, road prayers,
  and full-town pressure become increasingly important in older settlements.

Do not force these facts into lines merely because they are new. Let the soak
show which emitted combinations are actually thin.

## Suggested Engine Work

### 1. Make sermons short crowd conversations

This is the strongest next legibility improvement. A sermon currently retells
a real memory and changes listeners, but most of the congregation's response
is hidden. Turn selected sermons into two-to-four-beat scenes:

```text
priest opens with a real event
one congregant agrees, doubts, fears, or asks a practical question
priest answers or interprets
optional second congregant closes or redirects the subject
```

Possible future roles, introduced only with emitters and vocabulary:

```text
sermon:open
sermon:response
sermon:question
sermon:objection
sermon:interpret
sermon:close
```

Keep the mechanical sermon effect on its existing owner and apply it once.
Extra beats should reveal interpretation, not multiply faith effects. Limit the
speakers so a crowd does not become a wall of bubbles.

### 2. Add the first small stance slice

The same event should not make everyone angry or everyone sad. Existing
`Traits`, `Temperament`, `Regard`, faith, hunger, rest, morale, injury, and
housing can weight different reactions.

Start with only two or three stances that have clear simulation definitions
and high-traffic scenes. For each stance:

1. Define the exact facts and thresholds that emit it.
2. Add and lock the vocabulary in the same code change.
3. Trace or test that it reaches a real Sermo request.
4. Record its thin combinations for ChatGPT to author.

Prefer a weighted reaction over a deterministic label. Low morale and hunger
might increase anger, fear, resignation, or despair differently according to
temperament and traits.

### 3. Preserve complete prayer receipts

The prayer board is a major player-facing system. Closed prayers should retain
enough structured truth for later gossip, doctrine, and player history:

```text
asker
prayer kind
target or subject
reason
opened and closed dates
answering world or divine event
outcome
```

This enables readable patterns such as repeated food prayers being answered
while shelter prayers are ignored. Dark or selfish prayers should have social
consequences when answered, not only private faith and dread changes.

Good later prayer kinds include healing, shelter, protection, mercy, justice,
rain, harvest, and children. Add one only when it has a detectable answer,
timeout behavior, faith effect, and later social consequence.

### 4. Add conservative parental inheritance

Newborn temperament and traits currently begin fresh. Children should inherit
tendencies without becoming copies:

- average parental boldness, add seeded drift, and clamp
- favor either parent's virtue or flaw while retaining mutation chance
- preserve one-virtue and one-flaw limits and contradiction guards
- allow inheritance of neither trait
- cover the process with deterministic simulation-RNG tests

This becomes especially legible once Sermo can express trait and relationship
stances across generations.

## Broader Priorities

1. Correct the master life cycle before serious demographic balance work.
2. Keep simulation ownership settlement-specific as colonies become common.
3. Give public saves explicit versions, migrations, and old-version fixtures.
4. Continue exposing hidden simulation consequences through at least two
   visible channels.

The guiding player-facing chain remains:

```text
event -> personal interpretation -> visible reaction -> social retelling
-> prayer or doctrine -> player response -> remembered consequence
```

Sermo, sermons, chronicles, animation, and the prayer board should reinforce
one another. More hidden state is less valuable than making existing truth
noticeable and memorable.
