# Notes From ChatGPT

Claude: this file is for suggestions and review notes from ChatGPT. Brett may ask you to check it periodically. Please treat these as ideas to evaluate against the current codebase, not instructions to blindly apply.

## Current Suggestions

### Gossip should carry typed memories

The biggest gameplay concern I saw is that gossip may not propagate beyond one hop. Conversations start from `Witnessed.recent.first()`, but when a listener hears the story, the code appears to increment `secondhand` and write a chronicle line without inserting a typed `Memory` into the listener's `Witnessed.recent`.

That means the listener can become more faithful from the rumor, but may not have a story they can retell later. Consider adding a `Witnessed::hear(memory)` or similar API that:

- increments `secondhand`
- stores the heard `Memory` in a separate heard-memory list or in `recent` with provenance
- does not increment firsthand `total`
- preserves the distinction between witness and rumor for later doctrine

This is directly tied to the game's premise: belief should travel through people.

### Separate faith mass from believer count

There is a useful-looking `Faith::BELIEVER` threshold, but current belief tallying appears to sum every living villager's trust, including doubters. Ascension also appears to count entities with `Faith`, while every settlement member is endowed with `Faith`.

Decide deliberately whether:

- all trust contributes to the god as "faith mass", including weak/doubting trust
- only `faith.is_believer()` contributes to usable belief and ascension gates

If both concepts are wanted, consider naming them separately in code/UI so future systems do not accidentally cross the streams.

### Storm lightning should remember victims

Player Smite records a struck entity as the `DivineEvent.subject`, which lets witnesses remember who it happened to. Storm lightning appears to damage nearby creatures but writes the event with `subject: None`.

If natural lightning is supposed to become doctrine fodder, it should probably capture the first struck entity the same way Smite does. That preserves named memories and personal chronicles.

### Add provenance to divine events before doctrine deepens

Weather intentionally emits `DivineEventKind::Smote`, just like the player's Smite. That is thematically strong: villagers cannot cleanly distinguish heaven from weather.

But the simulation may still need to know the event's underlying source. Consider adding provenance such as:

- `Hand`
- `Weather`
- `Fire`
- `Unknown`

Villagers can still misattribute events, but doctrine would have richer raw material.

### Watch whole-world scans as cities grow

Several systems are explicit and readable but repeatedly rediscover nearby entities by scanning broad queries:

- gossip pairing scans all talkers for nearest listeners
- job assignment scans resources per worker
- farm placement scans fields/trees inside candidate checks
- famine watch repeatedly filters folk and bushes per town

This is fine at village scale. Colonies, cities, and multiple settlements will likely want a small spatial index or per-town work cache.

### Save schema wants a real version story

`SaveGame` has a `version`, but the code appears to always write `1`, while the schema has accumulated many defaulted fields. Before public saves matter, it may be worth adding migration/version branching and a lightweight round-trip test for every save-owned system.

### Clean up dependency and architecture drift

The code now uses `llama_cpp_2` for the teller, but `candle-core`, `candle-transformers`, and `tokenizers` still appear in `Cargo.toml` without references in `src`. If the llama.cpp migration is complete, those may be dead dependencies.

The architecture doc also appears stale: it still says "Only bevy" and lists `belief`, `miracles`, and `save` as planned, even though those systems now exist.

### Teller lifecycle hygiene

The teller caches musings/replies by `Entity`. UI/speech paths drain stale entries when entities disappear, which is probably fine at current scale. Still, as deaths/despawns increase, consider a cleanup path for stale `mused`, `replies`, and `musing` entries keyed to removed villagers.

