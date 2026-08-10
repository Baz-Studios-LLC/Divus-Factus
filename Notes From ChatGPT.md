# Notes From ChatGPT

Reviewed against the current code on 2026-08-10. This is a living handoff for
Claude, not a history of ideas. Delete a section when it is implemented or no
longer fits the design. Before adding new advice, compare the existing notes
against the current code and remove completed, duplicated, or contradicted
items. Completed work may be named once as foundation, but must not remain
written as pending work.

## Sermo Collaboration Handoff - Immediate Next Work

Brett wants Sermo to become genuinely deep. Since ChatGPT can author corpus
content quickly, the target should be broad situational coverage: many ordinary
thoughts, prayers, stories, and conversations selected from precise simulation
truths. The goal is not volume for its own sake. The goal is that villagers
rarely repeat themselves and usually say something that reveals why this
particular person is having this particular moment.

### Integrated foundation

Commit `8bf7c03` integrated the first ChatGPT-authored depth batch:

```text
assets/voice/births_depth.json
assets/voice/daily_life_depth.json
assets/voice/prayers_depth.json
assets/voice/conversation_depth.json
```

It contains 158 new records and 181 possible utterances after alternations,
bringing the corpus to 1,398 records. It covers the current worn pools for:

- firsthand, secondhand, and distant birth stories
- housed and married thoughts
- fisher, gatherer, forester, hunter, and priest thoughts
- devotion, hunger, wavering, and devout prayers
- generic and wavering first replies
- all four beats of forester conversation

The batch was audited across every expanded alternation for JSON validity,
locked vocabulary, exact duplicates, slot legality, capitalization, terminal
punctuation, and the 18-word limit. Both required gates pass:

```text
cargo test every_tag_is_one_the_game_speaks
cargo test a_things_story_is_never_told_about_a_person
```

That commit also implemented and tested sentence-case presentation in
`sermo::tidy()`. This foundation is complete; do not treat it as pending work.

### Brett's presentation preference

Brett wants normal sentence capitalization and punctuation. He does not like
ordinary dialogue beginning with a lowercase letter. Keep the voice plain,
brief, concrete, and conversational; this is not a request for formal or poetic
speech. Natural fragments remain appropriate when somebody is replying,
hesitating, or yelling.

`sermo::tidy()` now capitalizes every sentence start and supplies terminal
punctuation. Preserve that behavior and its tests.

### Run a clean Sermo soak next

Do not immediately request another blind batch. First run several in-game days
with the new corpus and watch speech at normal speed as well as accelerated
simulation speed.

For a useful authoring sample:

1. Clear or archive the old `voice-wanted.txt` before the controlled run so old
   counts do not masquerade as new misses.
2. Let a village experience work, meals, marriage, housing changes, prayer,
   weather, at least one birth, and several divine events.
3. Watch complete conversations, not only isolated bubbles.
4. Record any line that is grammatically fine but socially or factually wrong.
5. Preserve the newly generated `voice-wanted.txt` and hand it to ChatGPT.

The next batch should be ordered by fresh counts. Likely areas, if the run
confirms them:

- all remaining trades across `muse` and all four chat beats
- food, roof, and weather conversations across faith bands
- every event across `saw`, `heard`, `distant`, and compatible faith bands
- more devotion, hunger, road, and grudge prayers
- uncommon combinations such as housed + married + trade + faith
- better endings and followups so whole conversations sound coherent

### Highest-value engine change: topic-aware first replies

The first listener response to an event still enters Sermo as `reply + faith +
vocation + body`. `Musing.heard` carries the words, but corpus selection does
not receive the structured topic. A reply to a birth, smiting, gift of food, or
quake can therefore come from the same generic pool.

Please give first replies a structured topic path. A dedicated reply request is
cleaner than parsing the heard prose:

```text
reply role + Chat / DivineEventKind + faith + vocation + current state
```

Keep generic `reply` records as fallback. Keep knowledge transfer, faith
movement, morale movement, and chronicle writing on the opener only; later
beats reveal interpretation and must not apply the event repeatedly.

Once event/topic tags legally reach first replies, ChatGPT can author real
responses such as sympathy after a death, practical concern after a quake,
relief after a birth, unease after a smiting, and gratitude or suspicion after
provision. Until then, writing hundreds more generic `reply` lines creates
variety but not depth.

### The later depth multiplier: stance from existing simulation truth

After topic-aware replies are stable, expose a small, carefully chosen set of
stance tags derived from existing data: `Traits`, `Temperament`, `Regard`,
faith, hunger, rest, morale, injury, and housing. The same pressure should
weight different reactions rather than force one universal emotion.

Any new stance tag is a code feature first:

1. Define exactly which simulation facts emit it.
2. Add it to the locked vocabulary in the same code change.
3. Confirm it reaches a real Sermo request in a test or bench trace.
4. Then ask ChatGPT to author its corpus coverage.

Do not let ChatGPT invent tags directly in JSON. Its standing lane is
`assets/voice/*.json`; engine context and vocabulary remain Claude's lane.

### Definition of success

Sermo is deep when a player can watch one exchange and infer several truths:

```text
what happened
how directly the speaker knows it
what the speaker believes
what practical pressure they are under
how they feel about the person involved
whether the listener accepts, doubts, redirects, or ends the subject
```

The long-term loop should remain:

```text
simulation emits truthful context
-> Sermo selects authored words
-> player and human review expose thin or false coverage
-> voice-wanted.txt records missing combinations
-> ChatGPT authors a focused batch
-> gates, human read, soak, repeat
```

## Current Priorities

1. Correct the life cycle before doing another serious balance pass.
2. Make Sermo's first listener reply topic-aware, then add stance tags.
3. Let sermons become short crowd scenes instead of one line plus hidden math.
4. Deepen the prayer board's requests and receipts.
5. Give children some inheritance from their parents.
6. Continue removing focused-town assumptions before colonies become common.
7. Version and fixture-test public save data before release.

## Sermons Should Become Short Crowd Scenes

The current sermon system has the right simulation foundation: a working priest
at a shrine retells a real memory, incense extends reach and sway, nearby people
receive the story, faith changes, and chronicles record it. On screen, however,
it is still one preacher line followed by mostly invisible congregation math.

Give the sermon two or three additional scheduled beats:

```text
priest opens with a real event
one congregant reacts from faith, traits, and current need
priest interprets or answers
optional second congregant objects, agrees, or redirects to a practical need
```

Useful roles:

```text
sermon:open
sermon:response
sermon:question
sermon:objection
sermon:interpret
sermon:close
```

Keep it short and selective. One devout voice and one doubting or pressured
voice can make the whole crowd legible. Do not have every congregant emit a
bubble.

As with conversation, apply memory and faith effects once. Extra beats reveal
interpretation; they must not multiply the sermon mechanically.

Examples of the desired register:

```text
Priest: "hear how food came when the stores were nearly empty."
Villager: "we needed it. that much is true."
Doubter: "that doesn't mean the god put it there."
Priest: "no. but it means we should remember who was fed."
```

```text
Priest: "hear how lightning took Mara in front of everyone."
Villager: "I don't like that story."
Priest: "neither do I. power is not the same as kindness."
```

## Prayer Board: Preserve More of the Request

The prayer board is no longer a suggestion; it is working. Open prayers are
named, visible in the codex, clickable, pinned in the world, and closed into a
ledger. Food, dark, road, and devotion prayers already create a good moral and
mechanical range. Answering through an action in the world is much stronger
than an `Accept` button, so protect that design.

The next structural improvement is richer receipts. `ClosedPrayer` currently
keeps the asker's name, optional words, and broad outcome. It loses the prayer
kind, target, reason, dates, and the event that answered it. Preserve enough
structured information to let the board and later gossip say what actually
happened:

```text
who asked
what kind of prayer it was
who or what it concerned
why they asked
when it opened and closed
which world/divine event satisfied it
how it ended
```

That enables meaningful history such as "you answered three prayers for food
but ignored two for shelter" and lets doctrine form around patterns instead of
only totals.

Good future prayer kinds, added slowly:

- healing for a named injured or sick person
- shelter for a roofless household
- protection from a wolf or other known danger
- mercy or forgiveness after a conflict
- justice against someone, distinct from a request for execution
- rain or a good harvest during a real shortage
- a child, when family and fertility systems can support the consequences

Every prayer needs a detectable answer condition, timeout outcome, faith
effect, and at least one later social consequence. An answered dark prayer in
particular should become gossip and doctrine, not only a private faith increase
and dread score.

## Children Should Inherit Some of Their Parents

The old personality-foundation suggestion is complete in a different and
better-fitting form: the game has `Traits` plus the continuous `Temperament`
boldness value, both saved and already used by mechanics. The remaining family
piece is inheritance. Newborns currently roll fresh traits and temperament.

A conservative inheritance pass:

- Average the parents' boldness and add small seeded drift, then clamp it.
- For virtues and flaws, favor traits carried by either parent but keep a
  modest chance of a new trait.
- Preserve the current maximum of one virtue and one flaw.
- Preserve the contradictory-pair guard.
- Allow children to inherit neither trait; heredity should create tendencies,
  not dynasties of clones.
- Add deterministic tests using the simulation RNG.

The player should eventually be able to notice family tendencies over several
generations: a line of bold explorers, a devout household, or a gloomy family
that has also lived through repeated loss. Inheritance becomes especially
valuable once Sermo uses trait and relationship stance.

## Fix the Life Cycle Before Balance Tuning

The roadmap still defines a natural life of about five in-game years, but the
code remains on the intermediate calendar:

- childhood is 16 days instead of year 1
- prime lasts 84 days instead of roughly year 3.5
- `grow_old()` still states that nobody dies of age

This remains the highest-impact simulation correction. Birth spacing,
courtship, food balance, housing demand, skill mastery, inheritance, winter
survival, and colony pressure all lie when measured against the wrong
generation length.

Implement the roadmap's master calendar first, then rerun demographic and
economic soaks. Include old-age death as its own readable cause with a
chronicle entry, family response, rite, and grave history.

## Keep Settlement Ownership Explicit

Multi-settlement support has advanced: settlement state is increasingly held on
components and resolved through `MemberOf`, and colony/road prayers are real.
The project still contains focused-town resources such as `SettlementSite`,
`KnownWorld`, and `SettlementCulture`, while save gathering still starts from
one `SettlementSite`.

Before daughter towns become common, audit each gameplay system that reads a
focused resource. UI and camera focus may legitimately use it; simulation
decisions should usually resolve the actor's settlement through `MemberOf`.
Pay special attention to:

- naming and culture in daughter towns
- food and housing capacity
- work targets and stores
- exploration knowledge
- miracles whose effect is intended to be local to one town
- save/load of multiple settlements and their relationships

Do this as targeted ownership fixes, not a broad ECS rewrite.

## Give Public Saves a Version Story

`SaveGame.version` still writes `1` while the save schema has accumulated many
defaulted fields. Defaults are useful during development, but a partially
compatible load can look healthy while quietly losing family, faith, prayer,
settlement, or social state.

Before saves become a player promise:

- define explicit migration branches by version
- keep at least one fixture from each released version
- round-trip a living multi-generation village
- verify prayer history, traits, parentage, homes, stores, memories, and
  settlement membership after load
- reject unsupported future versions clearly

## Legibility Rule

The central design standard should remain: simulation truth must reach the
player. When adding or deepening a system, identify at least two visible
channels for its consequences, such as:

```text
animation or posture
speech / conversation / sermon
prayer board
chronicle or stirrings
building use and town layout
notice or codex history
```

The best chain in this game is:

```text
event -> personal interpretation -> visible reaction -> social retelling ->
prayer or doctrine -> player response -> remembered consequence
```

That chain is more valuable than another isolated stat. Sermo, sermons, and the
prayer board are not decoration around the simulation; together they are its
player-facing interface.
