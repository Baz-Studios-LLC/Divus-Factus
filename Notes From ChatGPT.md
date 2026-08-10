# Notes From ChatGPT

Reviewed against the current code on 2026-08-10. This is a living handoff for
Claude, not a history of ideas. Delete a section when it is implemented or no
longer fits the design.

## Current Priorities

1. Correct the life cycle before doing another serious balance pass.
2. Make Sermo's first listener reply topic-aware, then add stance tags.
3. Let sermons become short crowd scenes instead of one line plus hidden math.
4. Deepen the prayer board's requests and receipts.
5. Give children some inheritance from their parents.
6. Continue removing focused-town assumptions before colonies become common.
7. Version and fixture-test public save data before release.

## Sermo: Make the First Reply About What Was Said

Sermo is already a strong foundation. It is authored, fast, tag-driven,
debuggable, and independent of a runtime LLM. `Corpus` already rewards
specificity and freshness, records thin coverage in `voice-wanted.txt`, and
loads every JSON file under `assets/voice`. `Conversing` already carries a
topic and schedules a four-beat exchange without transferring the same rumor
four times.

The remaining weak point is the listener's first answer to an event story.
`hold_conversations()` passes the teller's words into `Musing.heard`, but
`Tongue::muse()` does not derive meaning from those words. It picks with:

```text
reply + faith + vocation + body
```

It does not receive `event:smote`, `event:provided`, or the conversation's
other topic tag. A listener can therefore answer a smiting and a gift of food
with the same generic line. Later beats use `turn_about()` and are already
topic-aware, so the first reply is now the odd beat out.

Recommended change:

- Add a dedicated `Tongue::reply(...)` path, or pass an optional structured
  topic into `Musing`.
- Pass `Chat` / `DivineEventKind` from `hold_conversations()` rather than trying
  to parse the raw `heard` string.
- Select with `chat:reply + topic + faith + vocation`, with generic
  `chat:reply` as the final fallback.
- Keep knowledge transfer, faith movement, morale movement, and chronicle
  writing on the opener only. Later words are interpretation, not new events.

The eventual useful shape for every beat is:

```text
role + topic + faith + vocation + emotional stance + relationship stance
```

Examples:

```text
chat:reply + event:smote + doubting + afraid
chat:reply + event:provided + devout + grateful
chat:followup + food + hungry + trade:cook
chat:end + roof + roofless + gloomy
```

Do this incrementally. Topic-aware first replies are the important fix;
emotional and relationship stance can follow once the basic path is covered.

## Sermo: Let Existing Character Data Reach the Words

The game now has more character differentiation than Sermo exposes. `Traits`
already changes work pace, conviction, morale recovery, talkativeness,
endurance, and appetite. `Temperament` changes reactions through boldness.
`Regard`, hunger, rest, morale, injury, faith, vocation, and housing state also
exist. Most chat selection currently sees only topic, faith, vocation, and
occasionally `told`.

Do not make traits dictate a sentence. Let them weight an emotional
interpretation, then expose that interpretation as a tag:

```text
simulation pressure -> weighted interpretation -> Sermo stance tag
```

Examples:

```text
hungry + low spirits + gloomy             -> sad / hopeless weighted upward
hungry + low spirits + bold               -> angry / demanding weighted upward
smite witnessed + skeptic                 -> bitter / afraid weighted upward
food provided + devout                    -> grateful weighted upward
friend harmed + warm regard               -> worried / angry weighted upward
enemy helped + sour regard                -> resentful weighted upward
```

The same facts must still produce different people. Use seeded rolls and
weights, not rules such as `Gloomy == always sad`. Save a stance only if it
needs to persist beyond the current exchange; otherwise derive it when a beat
is selected.

Useful first stance vocabulary:

```text
angry afraid sad hopeful bitter grateful practical suspicious relieved
```

Only add a tag when enough lines or a reliable fallback exist for it. The
`voice-wanted.txt` loop should guide which combinations deserve writing.

## Split and Test the Voice Corpus

`assets/voice/chat.json` is now large enough that authoring mistakes will be
harder to see by inspection. The loader already supports splitting it without
an engine change. A practical layout would be:

```text
assets/voice/chat_general.json
assets/voice/chat_events.json
assets/voice/chat_work.json
assets/voice/chat_needs.json
assets/voice/sermons.json
assets/voice/musings.json
assets/voice/prayers.json
assets/voice/tellings.json
assets/voice/yells.json
```

Split by register and subject, not by every individual tag. Too many tiny files
will make combinations harder to reason about.

Add a corpus audit that can report:

- unknown or misspelled tags
- missing slot values such as `{whom}`
- a generic fallback for every runtime role
- lines that can never match any emitted tag bundle
- thin role/topic/stance combinations
- duplicate or near-duplicate text

The goal is not simply more lines. Every visible line should reveal at least
one simulation truth: what happened, what the speaker believes, what they need,
what they fear, or how they regard somebody involved.

Keep the voice ordinary and practical. These are people talking, not prophecy
machines.

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
