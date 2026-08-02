# Notes From ChatGPT


## Current Suggestions

### The lifecycle constants are the next load-bearing tuning pass

The roadmap now says the natural life should be about five years, with a
childhood of year 1, prime through roughly year 3.5, elderhood around year 4,
and old-age death in a 4.5-6 year window.

The current code still appears to be in an intermediate state:

- `SECONDS_TO_COME_OF_AGE` is 16 days, not one year.
- `SECONDS_OF_PRIME` is 84 days, roughly three seasons.
- `grow_old` explicitly says no one dies of age yet.
- Birth recovery is 24 days and fertility falls by prior births, but the
  broader demographic calendar has not fully moved to the five-year model.

This is probably the highest-value next pass because every soak result depends
on it. If rates are tuned before age/birth/courtship are calendar-correct, the
village will keep lying about whether food, shelter, work, and winter are
balanced.

### Define faith mass vs believers before doctrine deepens

The code has a useful `Faith::BELIEVER` threshold and uses it in some player-
facing places, but `tally_belief` sums every living villager's `trust`, and
ascension currently counts `Faith` holders rather than only believers.

That may be the right design, but it wants naming discipline:

- **Presence / belief mass:** the total psychic weight sustaining the god,
  including weak trust and doubt.
- **Believers / congregation:** people over the threshold, used for social
  legibility, shrines, sermons, schism, and maybe ascension gates.

If those remain the same number in UI copy, later doctrine code may accidentally
make skeptics power the god as strongly, or make weak trust count as formal
membership.

### Multi-settlement architecture is halfway real, halfway singleton

The good news: `Settlement` and `SettlementGround` are components, `MemberOf`
is widespread, and many work/home systems already resolve a person's own town.
That is the right direction for colonies and schism.

The risky middle: several systems still lean on singleton resources such as
`SettlementSite`, `KnownWorld`, and `SettlementCulture`, which are explicitly
"focused town" or founding conveniences. Before a second town becomes real,
audit any system that reads those resources and ask whether it should instead
resolve through `MemberOf`.

Places especially worth checking later:

- birth capacity and larder checks
- hunger/store decisions
- save/load, which currently gathers one focused settlement
- store trend sampling
- known-world/explorer pockets
- language/culture for naming children in daughter towns

### The simulation-first directive should be guarded as a rule

The roadmap says no new god powers until the unattended-village test passes.
The current hotbar already has Flourish, Smite, Bounty, plus one earned tier-two
slot. That is enough surface area to test theology.

Suggestion: make "no new miracles until the soak passes" a standing rule in the
roadmap or comments near `Miracle`. The temptation to add powers will be high
because the hotbar is fun, but the game's stated hard-road is that the world
must become trustworthy before the god becomes larger.

### Flourish and Bounty both emit Provided

`Miracle::Bounty` naturally writes `DivineEventKind::Provided`. `Miracle::Flourish`
also currently writes `Provided`, even though `DivineEventKind::Flourished`
exists and farmers use it for heavy harvests.

This may be intentional: the god provided food, not merely a good harvest. But
if doctrine is going to distinguish "food placed before us" from "the land was
made abundant," Flourish may want its own event kind. Otherwise villagers'
memory and legend may flatten two theologically different acts into one story.

### Quake discards its subject

`cast_earned` for Quake finds a first affected entity, then writes the event with
`subject: None`. If Quake is meant to be remembered as an impersonal disaster,
that is fine. If a villager should remember "Feitreh was thrown down when the
earth buckled," then this is a small missed opportunity.

Same design question as storm lightning: personal names make doctrine sharper.

### Save format needs a real version story before public saves matter

`SaveGame.version` still writes `1`, while many serde-default fields have been
added. The current compatibility style is pragmatic and probably fine during
rapid iteration, but once saves are player-facing, version branches and a
round-trip fixture suite will matter.

This is especially important because saves now include enough living state that
a half-compatible load can look "mostly fine" while quietly damaging families,
homes, stores, faith history, worked ground, or wildlife patches.

### The highest-payoff legibility task is still the attribution moment

The underlying system is now good: witnesses get per-memory `divine` verdicts,
faith moves only when they attribute the event, and gossip carries retellable
memories. But the screen still seems less expressive than the model.

The manifestation audit's top item still looks right: when an event lands, the
crowd should visibly split. People who believe the god did it kneel, raise
hands, approach, or stare upward; people who do not should flinch, resume work,
or react only to the worldly danger.

This is not a new simulation. It is the existing verdict finally made visible,
and it would make the game's thesis readable in one glance.

### Winter should be tuned as a story engine, not just a difficulty spike

The roadmap frames winter as the antagonist. The code already has seasons,
growth slowdown, chill, weather toil, fire fuel pressure, fields, food kinds,
stores, roofs, and wolves. That is enough machinery to make winter readable.

The key is causes:

- Did they starve because the harvest was too small?
- Because fish/meat spoiled or were drawn down before winter ended?
- Because firewood was spent on buildings?
- Because roofless sleepers lost morale and stopped working?
- Because wolves came close when wild prey thinned?

The existing `starvation_watch` and chronicle style are good models. Winter
needs the same receipts before it needs harsher numbers.

### Roads and provisioning are the bridge to the wider world

The range problem in the roadmap is exactly what the work code is starting to
solve with rations and known far pockets. The next outward step probably wants
roads/trails to have mechanical weight before horses or caravans.

Low-risk sequence:

1. Trails reduce travel cost or route patience loss.
2. Long jobs require visible rations and write "road" causes when they fail.
3. Expedition parties share provisions.
4. Carts/horses multiply capacity once the foot-road loop is trustworthy.

That keeps distance from becoming teleporting with extra art.

### Watch global scans before cities, but do not optimize too early

The code is wonderfully explicit: lots of systems scan broad queries and choose
nearby things directly. That is healthy at village scale and keeps behavior
legible.

Before populations reach low hundreds or multiple towns, plan a measured cache
pass rather than a general rewrite:

- per-town census/resource/work caches
- simple spatial bins for talkers, patients, prey, trees, and job sites
- save/load tests around any cache that shadows authoritative ECS state

The danger is not today's performance. The danger is needing those caches under
pressure and adding them after systems have grown around singleton assumptions.

### The architecture doc is now historical

`docs/ARCHITECTURE.md` is still useful for intent, but it says only Bevy is a
dependency and lists several modules as planned that now exist. The roadmap and
code comments are currently more authoritative.

At some point, update it as a design-history document or bring it current. Until
then, anyone new to the project should be warned that it is not the live map.

### The Atelier contract is clean; protect that boundary

The Atelier/game separation is conceptually strong: no shared code, palette and
blueprints as files, and game-understood parts rather than arbitrary geometry.

When importing Atelier output into the game, keep that boundary strict:

- widgets describe semantics; they should not become generic object scripts
- colours stay palette names and shades, not raw RGB
- format versions advance whenever meaning changes
- authored pieces should still be built by villagers stage by stage, not appear
  as prefab exceptions

That lets the tool grow without weakening the procedural village premise.

### Personality should be added as foundation data before it drives behavior

A good next architecture step is to give villagers stable personality stats,
but not immediately wire those stats into work, survival, or belief behavior.
This preserves the current simulation while creating a reliable base for richer
speech, prayers, gossip, doctrine, and family identity later.

Suggested first-pass component:

```rust
pub struct Personality {
    pub boldness: f32,
    pub temper: f32,
    pub compassion: f32,
    pub resilience: f32,
}
```

Keep each value in `0.0..=1.0`. Founders can receive seeded random values.
Children should inherit from both parents with a little drift, for example:

```rust
child_trait = average(parent_a, parent_b) + small_random_drift
```

Then clamp the result. This makes family tendencies visible over generations
without making children clones of their parents.

Important implementation advice:

- add the component to new villagers and newborn children
- persist it through save/load
- give old saves a stable fallback
- consider showing it in the inspector/debug UI
- do not change behavior in the first pass
- use the game's seeded RNG, not thread-local randomness

This matters because the same simulation pressure should not affect everyone in
the same way. Low morale plus high hunger should not always mean `angry`. For
one villager it might become anger; for another, fear, sadness, prayer,
bitterness, or grim determination.

The eventual speech flow should be:

```text
simulation truth -> pressure -> personality-shaped interpretation -> speech tag
```

Examples:

```text
hungry + low morale + high temper      -> angry
hungry + low morale + low resilience   -> despairing or afraid
hungry + low morale + high compassion  -> worried about others
hungry + low morale + high faith       -> prayerful
smite witnessed + doubting             -> fearful or bitter
food provided + devout                 -> grateful
```

This would let future voice lines use tags like `angry`, `afraid`, `sad`,
`hopeful`, `bitter`, `grateful`, or `vengeful` without pretending those emotions
are universal reactions. Personality should weight emotional outcomes, not make
them deterministic.

For the prayer board idea, this is especially valuable. A hungry compassionate
person might pray for children to be fed first. A hungry resentful or hot-headed
person might pray for someone else to suffer. The same town condition produces
very different petitions, which gives the player morally interesting choices.

### Conversations and sermons should become small staged exchanges

The game already has a real conversation foundation in `src/villager/gossip.rs`.
It is not just random barks: a villager with a recent `Witnessed` memory finds
an idle neighbour, both enter `Activity::Chatting`, they walk toward each other,
the teller speaks, the listener may reply, the listener receives the memory, a
chronicle entry is written, and faith can shift.

That is strong groundwork. The important next step is not to replace it, but to
deepen it from a two-beat gossip exchange into a small staged conversation.

Current shape, simplified:

```text
A has recent memory
A finds idle B
A and B meet
A tells memory
B may reply once
B receives memory / faith shifts / chronicle records it
conversation ends
```

Desired next shape:

```text
A opens with a topic
B reacts from their stance
A follows up or pushes back
B agrees, doubts, objects, asks, or ends
conversation releases both villagers
```

Keep this deliberately small. Two to four spoken turns is enough. The goal is
not long dialogue; the goal is that the player can see villagers process an
event socially.

#### Conversation topic should be explicit shared state

Characters stay on topic by carrying a topic object through every turn. Do not
recreate the subject from scratch on each line.

Suggested direction:

```rust
pub struct Conversing {
    pub partner: Entity,
    pub until: f64,
    pub topic: ConversationTopic,
    pub role: ConversationRole,
    pub turn_index: u8,
    pub next_turn_at: f64,
    pub remaining_turns: u8,
    pub last_line: Option<ConversationLineKey>,
}
```

Possible topic:

```rust
pub struct ConversationTopic {
    pub kind: ConversationTopicKind,
    pub memory: Option<crate::witness::Memory>,
    pub tags: Vec<&'static str>,
    pub started_by: Entity,
}
```

Possible topic kinds:

```rust
pub enum ConversationTopicKind {
    Memory(crate::witness::DivineEventKind),
    Need,
    Prayer,
    Work,
    Weather,
    Home,
    Sermon(crate::witness::DivineEventKind),
}
```

For a first pass, `Memory(DivineEventKind)` is enough. The conversation can
continue using the existing gossip memories and only add structure around turn
order.

#### Use turn roles, not free generation

Runtime AI is not needed. The current authored corpus approach is better for
this game because it is controllable, debuggable, and tied to actual simulation
truths.

Add line roles such as:

```text
chat:open
chat:reply
chat:followup
chat:agree
chat:disagree
chat:question
chat:end
```

Then every line is selected by:

```text
conversation role + topic tags + speaker stance + speaker condition
```

Example tags:

```text
chat:reply event:smote doubting
chat:question event:provided hungry
chat:followup event:mauled afraid
chat:end event:smote wavering
```

The current `tell`, `reply`, and `event:*` tags can remain during transition.
Do not break existing lines. Add new roles alongside them and let the corpus
support both old and new shapes for a while.

#### Stance should come from simulation truth plus personality

A listener's response should not be random only. It should be weighted by:

- faith: devout, wavering, doubting
- whether they also witnessed the event
- morale / hunger / injury / housing pressure
- future `Personality` stats like temper, compassion, resilience, boldness
- relationship to the subject if known
- how often they have heard/told this kind of story

Example interpretation:

```text
event:provided + hungry + devout      -> grateful agreement
event:provided + doubting             -> skeptical explanation
event:smote + wavering                -> fear or moral unease
event:smote + high temper             -> anger or blame
event:mauled + guard                  -> practical safety response
event:quaked + low resilience         -> fear
event:lifted + devout                 -> awe
event:lifted + doubting               -> discomfort, suspicion, denial
```

The same topic can produce different lines because villagers interpret it
differently. This is the heart of making them feel like people.

#### Conversation line selection can stay simple

A first implementation does not need a new dialogue engine. It can extend the
existing `Tongue` / `Corpus` path.

Possible API direction:

```rust
pub struct ConversationTurn {
    pub who: Entity,
    pub role: ConversationTurnRole,
    pub topic_tags: Vec<&'static str>,
    pub stance_tags: Vec<&'static str>,
    pub condition_tags: Vec<&'static str>,
    pub slots: Vec<(&'static str, String)>,
}
```

Then the corpus receives one combined tag list:

```text
chat:reply event:smote heard doubting afraid
```

Keep generic fallbacks for every role:

```json
{ "t": "I don't know what to say to that", "tags": ["chat:reply"] }
{ "t": "maybe. I need to think about it", "tags": ["chat:end"] }
```

Generic fallback lines prevent silence while the authored corpus is still thin.

#### Conversation turns should be scheduled beats

Avoid making both villagers talk in the same frame. Speech should have rhythm.

Example timing:

```text
meet distance reached
0.0s: opener
3.0s: reply
5.5s: followup
8.0s: closing line
10.0s: release villagers
```

This can reuse the current `until` idea, but `spoke_at` and `replied` should
evolve into `turn_index` and `next_turn_at`.

Important: conversations should not trap villagers. If hunger, sleep, danger, or
work pressure is high, shorten the exchange or decline to start it.

#### Knowledge transfer should happen once

The listener should receive the memory once, not on every conversational beat.
The cleanest rule is:

```text
knowledge transfers when the opener lands
faith/chronicle update once
later turns only reveal interpretation
```

This protects the simulation from duplicate rumor propagation while still
letting the conversation look richer to the player.

#### Suggested first implementation sequence

1. Rename nothing yet. Keep current `Conversing` working.
2. Add `turn_index`, `next_turn_at`, and `remaining_turns` to `Conversing`.
3. Keep memory transfer exactly where it is now.
4. Add one extra optional followup turn from the teller after the listener reply.
5. Add a small set of `chat:followup` and `chat:end` corpus lines.
6. Only after that works, add richer stance tags.
7. Later, split voice files into `thoughts`, `prayers`, and `chat` families.

This minimizes risk because the existing conversation loop remains intact.

#### Sermons should use the same beat idea, but with a crowd

The current sermon system in `src/villager/work/buildings.rs` is also a strong
foundation: a priest at a shrine retells a memory, nearby villagers receive it,
faith increases, and chronicles record the hearing. Right now it is one preacher
line plus invisible congregation effects.

Make sermons visibly social by adding beats:

```text
preacher opens topic
devout listener answers or murmurs agreement
doubter mutters or questions
preacher interprets the event
hungry / hurt / roofless listener pulls it back to practical need
preacher closes
```

Suggested sermon state:

```rust
pub struct SermonScene {
    pub preacher: Entity,
    pub audience: Vec<Entity>,
    pub topic: ConversationTopic,
    pub beat: u8,
    pub next_beat_at: f64,
    pub until: f64,
}
```

Start even smaller if needed: do not create a full component yet. The sermon
system can schedule one or two delayed crowd responses using a resource or event
queue. But the eventual model should be a short scene.

Useful sermon corpus roles:

```text
sermon:open
sermon:interpret
sermon:close
sermon:response
sermon:amen
sermon:mutter
sermon:question
sermon:objection
```

Useful sermon tags:

```text
event:provided
event:smote
event:mauled
devout
wavering
doubting
hungry
roofless
hurt
afraid
grateful
angry
```

Example non-poetic sermon exchange:

```text
Priest: "hear how food came when the stores were almost empty."
Villager: "we needed that. badly."
Doubter: "or someone found a basket and dressed it up."
Priest: "maybe. but people ate, and that matters."
```

Another:

```text
Priest: "hear how lightning took Mara in front of everyone."
Villager: "I don't like that one."
Priest: "neither do I. power is not the same as kindness."
```

This matters because crowd response makes doctrine visible. The player should
not only see that faith increased; they should see who accepts the sermon, who
is unsettled by it, and who turns the teaching toward immediate village needs.

#### Plain-language rule for authored lines

Keep the voice ordinary. Villagers should sound like tired, practical people,
not like prophecy machines.

Good:

```text
"we needed that food. I don't care what anyone calls it."
"I saw it happen, and I still don't know what it means."
"don't say that like it was easy. someone died."
```

Avoid:

```text
"the heavens unfurled their terrible mercy"
"our souls trembled beneath the divine radiance"
```

The strongest writing for this game will usually be short, specific, and tied
to what the simulation actually did.

#### Why this helps the game

Back-and-forth conversation is one of the best ways to solve the problem of the
simulation being invisible. A good exchange can show:

- what happened
- who saw it
- who believes it
- who doubts it
- who is afraid
- what practical pressure the town is under
- how a doctrine is beginning to form

That means the conversation system is not decorative. It is an interface into
the simulation.
