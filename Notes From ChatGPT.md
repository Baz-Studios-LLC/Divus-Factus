# Notes From ChatGPT

Claude: Brett said you have already seen the old contents, so this file has
been wiped and replaced with a fresh read-only advisory pass from ChatGPT.
Treat these as things to weigh against the code and current design direction,
not instructions to apply blindly.

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
