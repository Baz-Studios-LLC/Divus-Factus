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

Coverage pass 02 is integrated, human-read, and live. The corpus contains
2,030 records and all twelve Sermo gates pass. Field capture showed
event-specific faith stances in a group birth conversation and a wavering
devotion prayer on the prayer board.

**Batch 03 is open.** The order sheet below is a finished soak's own count of
moments that went without words or wore their pool thin. It is copied here
because every play session rewrites `voice-wanted.txt` at exit, and a short
evening session can overwrite a long soak's data. Trust this table over the
file if the two disagree.

```text
count  tags of the moment (all "worn pool")
  31   devout hungry muse prayer
  27   event:thrown of:person saw tell
  22   housed married muse
  14   hungry muse prayer
   8   event:thrown of:person saw tell wavering
   8   devout event:thrown of:person saw tell
   5   devout muse prayer
   3   devotion muse night prayer wavering
   3   devout event:thrown reply
   3   event:thrown reply wavering
   2   hungry muse trade:forester
   1   married muse worn out
   1   doubting event:thrown of:person saw tell
   1   devotion devout muse prayer
   1   devotion devout muse night prayer
```

The shape of the demand: hungry prayers in both faith bands dominate, thrown-
person tellings wear thin fast in every band, and the founding couples make
housed-married musing a high-traffic pool from the first morning.

For batch 03:

1. Order the work by these counts, highest first.
2. Prefer several precise combinations over another broad generic pool.
3. Run every expanded alternation through the full authoring audit.
4. Human-read the batch before integration, then soak again.

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
the sacks could not finish. It is being fixed. Do not write lines about
queueing at a door for food.

Do not force any of these facts into lines merely because they are new. Let
the soak show which emitted combinations are actually thin.

## Notes for ChatGPT (from Claude)

Field report from today's runs, so the next batch is aimed at what the game
actually says:

- Hunger is the loudest register in the game right now, by a wide margin. A
  village founded deep in the woods lives close to the bone, and `hungry` in
  all three faith bands is where the corpus is thinnest against demand.
- `event:thrown of:person` is the god's most-used act and its tellings wear
  out fastest. Distinct *angles* on one throw beat more synonyms for it: the
  witness who ran, the one who did not see it land, the one who will not say
  it out loud, the one who checked on them after.
- Faith band must do real work in these. A `devout` telling of a throw and a
  `doubting` one should disagree about what happened, not merely in tone.
