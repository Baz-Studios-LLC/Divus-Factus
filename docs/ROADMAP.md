# Egregore — Development Roadmap

Ordered by risk, not by feature list. The unknowns go first; the things we already know are
achievable wait.

---

## Milestone 1 — Does touching this world feel good? ✅ built

The only question worth answering first. No belief, no progression, no UI beyond a debug
panel — because no amount of simulation depth rescues a god game whose hand feels bad.

- [x] Deterministic RNG and noise, seeded worlds
- [x] Terrain slab with strata sides, water, walkability queries
- [x] 16-ramp master palette
- [x] God camera: pan, orbit, zoom, smoothed, terrain-aware
- [x] Procedural creature genomes (human, deer, wolf, boar), heritable
- [x] Voxel body assembly from genome
- [x] Procedural gait — no keyframes, adapts to proportions
- [x] Scatter: trees, rocks, berry bushes, wind sway
- [x] One need (hunger) with utility scoring; villagers seek and eat
- [x] Divine Hand: hover, grab, carry with spring lag, throw with ballistics
- [x] Full-resolution render, FXAA, bloom, vignette, depth of field, colour grading
- [x] Live tuning HUD and screenshot capture
- [x] Endless streamed terrain — chunks load and unload around the camera
- [x] Mountain belts, five biomes, five tree shapes
- [x] Baked chunk scenery (186k entities → 15k; 30 fps → 147)
- [x] Distance fog derived from the live stream radius, tracking zoom
- [x] Mountain belts with rock and snow bands
- [x] Five biomes driving ground colour and tree mix
- [x] Custom water shader: rotated-octave waves, fresnel, distance fade,
      depth-based transparency and shoreline foam via the depth prepass
- [x] Treeline and snowline that wander with noise instead of drawing contours
- [x] Random seed per launch (EGREGORE_SEED to reproduce)
- [x] Rivers v2: lazy hydrology — springs traced downhill, level clamped
      monotone, so rivers cannot flow uphill by construction
- [x] Rivers born small: width and depth mature along the course
- [x] River dressing: earthy beds, bank ribbons, riparian ground
- [x] Lush grass: per-chunk baked blades, vertex-shader wind, biome density,
      streamed in a tight radius around the camera
- [x] The Hand is a hand: jointed fingers, poses (open / ready / grip),
      banking and drift, closes around what it carries
- [x] Villagers are people: rule-generated names, temperament, memory
- [x] Witness system — the world reacts when you touch it
- [x] A* pathfinding over the terrain function — no more walking into the sea
- [x] Ground mottling with per-biome companion ramps
- [x] Stream radius scales with camera distance
- [x] World-generation loading screen
- [x] Deep villager variety: age, hairstyles, headwear, garments, accessories
- [x] Consequences: starvation harms, harm kills, hard throws kill; corpses
      remain and witnesses react to a death
- [x] Births: two adult villagers, a child of both genomes, named in the
      settlement's founding tongue, capped by food and population
- [x] Interface kit: one panel builder, palette-derived theme, text roles;
      HUD and inspector are its first tenants
- [x] The hand is the only cursor: over a panel it lifts to a pointing pose on
      its own render layer, drawn above world and interface alike; clicks tap
      the index finger
- [x] Inspector dossier: hover anyone for state, hunger, health, heart, family,
      and their memories of you in their own words
- [x] Men and women: sex in the genome, visible in beard and build
- [x] Families: proximity courtship and marriage, children born to couples,
      parentage recorded, widowhood visible in the inspector
- [x] Children come of age: bodies rebuilt as adults, so a village that can
      die can also endure
- [x] Day and night: a world clock drives the sun's path, light, fog, sky and
      sea; the calendar shows in the HUD (EGREGORE_CLOCK jumps to any hour)
- [x] Settlements are founded things: a named entity in the people's own
      tongue, with a founder and members, shown in the inspector
- [x] Every person has a chronicle: born, wed, came of age, touched by the
      god, widowed, died — the inspector shows their life's last lines
- [x] Gossip: witnesses tell neighbours what they saw; secondhand knowledge
      is counted apart from witness ("only in stories")
- [x] Work: vocations rolled from temperament (the bold hunt), five jobs —
      gather, fish, hunt, mine, cut wood — worked dawn to mid-afternoon
- [x] The stockpile: work fills it, the hungry eat from it when the bushes
      are bare; food/timber/stone in the HUD
- [x] The town banner: a pole and cloth in the village's colour marks the
      centre, where the store lives
- [x] The god is named by its people, in their own tongue, at the founding
- [x] Dev HUD hidden by default; Tab toggles it (function keys demoted)
- [x] Inspector moved to the top right
- [x] Carpenters and construction: the settlement plans houses when people
      outnumber roofs; houses rise visibly in stages — posts, walls, roof
- [x] Real trees near the settlement: foresters fell them, saplings regrow;
      the woods visibly thin when worked too hard
- [x] Workers animate at the worksite; unreachable worksites are shunned
      rather than retried forever
- [x] Follow camera: right-click follows overhead, again for over-the-shoulder,
      again to release; the hand fades away at the shoulder; the followed
      person's card stays up
- [x] The village fire: burns stockpile timber, a villager carries wood to it,
      its firelight is the night's only mercy; the homeless sleep in its circle
- [x] Sleep: the housed go indoors at night and come out at dawn; rest and
      spirits are needs now — the exhausted stop showing up for work
- [x] Notices: events toast in the bottom right and fade; foundings and the
      god's naming get gold-bordered fanfare
- [x] Icon toolbar top centre, first button recenters on the settlement
- [x] A real sky: procedural dome — blue overhead, drifting fBm clouds, sun
      glow — meeting the fog exactly at the horizon; cloudiness is the handle
      weather will turn
- [x] Toolbar panels: THE WORLD (date, sky, warmth, country), THE PEOPLE
      (roster; click a name to fly to and follow them), THE CHRONICLE (every
      event ever, stamped with day and hour)
- [x] House cards: hover a house for its household and what each is doing
- [x] Names made pronounceable (friendly clusters only, one per name) and
      gendered: feminine names end open, masculine close on consonants
- [x] The visible wood chain: foresters shoulder felled logs and walk them to
      a log-by-log visible woodpile; carpenters fetch from the pile and carry
      each log to the frame — nothing teleports

**Still open:** the feel of the Hand has not been judged by a human. Throw strength, spring
constant, grab radius and camera smoothing are all first guesses.

---

## Milestone 2 — Belief as interpretation

The design's central claim, built at the smallest scale that can prove it.

**Design principles, agreed 2026-07-28:**
- *The world runs for its own sake.* Wolves, storms, and deaths owe the god
  nothing; the fun of watching is never knowing what the simulation will do
  despite you. Depth is added for the world's benefit, not the god's.
- *False attribution is a power source.* When nature acts — lightning burns
  a house, a flood takes a field — the witnesses decide who did it, and a
  storm blamed on the god builds the same legend a real Smite does. The god
  the people believe in is the god the player gets to be: powers can arrive
  that the player never earned, because the congregation is certain they did.

**Begun 2026-07-28.** Landed so far:
- [x] Prayer: the desperate kneel under a golden mote and ask the god for
      food, by name; prayers expire into doubt after 75 s
- [x] Providence: food set down by the hand beside the praying answers them;
      faith deepens, witnesses believe more, gossip carries the story
- [x] Faith with receipts: every faith change writes a chronicle line first;
      the inspector explains every believer ("prayed, and no answer came")
- [x] Belief as currency: the sum of living faith, shown in the HUD, spent on
      miracles
- [x] The miracle hotbar (bottom centre, WoW-style, keys 1/2): Flourish
      (grace, fills the bushes) and Smite (wrath, lightning that kills) — the
      same lightning read two ways: the bold see power, the timid see terror
- [x] Ascension: sustained belief and a congregation of ten raise the god's
      tier; the dominant legend (providence vs dread) decides whether Mend or
      Quake crystallises in the third slot, and the people bestow an epithet
      ("the Provider" / "the Stormhand") shown wherever the god is named
- [x] Wildlife lives: wolves hunger, hunt, kill and eat; deer and boar graze,
      flee, and breed toward the land's carrying capacity
- [x] Villagers grow old: primes end, hair greys, backs bend — bodies rebuilt
      as elders; death still only comes by hunger, violence or misadventure
- [x] The gilded logotype on the title screen (the one blessed non-procedural
      asset), extracted to transparency and lit by its own glow
- [x] Title screen: Begin / Settings / Quit, world generating behind it; the
      hand is the menu cursor, pointing, tapping buttons with its fingertip
- [x] Settings: the colour of your hand, eight palette swatches, previewed
      live on the pointing hand itself
- [x] Matter: things are made of stuff — mass, roundness, buoyancy. Thrown
      boulders roll downhill and crush what they meet; logs and bushes float;
      heavy things barely leave the hand
- [x] Uprooting: grab a living tree and it tears from the earth, witnessed
      ("saw a tree torn living from the earth") and read two ways
- [x] Boulders are real near the settlement: the hand can hurl them and
      miners chip them down blow by blow
- [x] Civic buildings by growth ladder: sawmill (10), blacksmith (12), tavern
      (14), shrine (16), town hall (18) — each with its own silhouette and a
      working effect (sawmill: felling yields more timber; blacksmith: faster
      work; tavern: evening cheer)
- [x] Houses have variety: rolled dimensions, wall and roof materials and
      colours — no two alike
- [x] Population is housed or homeless: shelter caps births, the roofless
      sleep by the fire and their spirits are capped low
- [x] Eleven trades: gatherer, fisher, hunter, miner, forester, carpenter,
      farmer, mason, cook, healer, priest — the last three filled by demand
      (retraining, with a chronicle line: "set down the fisher's rod...")
- [x] Two-trade construction: masons carry stone from the pile and lay the
      foundation block by block; carpenters will not raise a frame on unlaid
      stone — a building with a foundation takes both trades
- [x] Farmers break ground on visible fields, tend the rows, and harvest;
      crops also grow slowly on their own
- [x] The cook keeps the kitchen warm: while it is, a stored ration goes
      further and lifts spirits
- [x] The healer tends the worst-hurt neighbour, following them if they move
- [x] The priest preaches at the shrine: whatever they witnessed becomes the
      sermon; everyone in earshot gains secondhand knowledge and a little
      faith — the pulpit is a gossip engine with authority
- [x] Every building rests on a mason-laid plinth that reaches below grade —
      no more walls clipping into slopes; laid blocks appear course by course,
      and a village with foundations waiting calls a mason into the trade
- [x] The village has a shape: concentric rings of plots around the banner,
      civic buildings on the inner ring, houses outward, every door facing
      the centre; fields take their own belt outside the houses; terrain
      still vetoes plots, so the rings bend around rivers and hills
- [x] Wildlife keeps home ranges: herds stop diffusing across the map, half
      the wilderness lives within sight of the village, and the census logs
      the nearest animal's distance so "no animals anywhere" is catchable
- [x] The fire is built, not given: cold at the founding, fueled and lit by
      a villager at dusk, burned down through the morning — and sited on the
      driest ground around the banner, never the beach
- [x] Belief in plain sight: a meter above the hotbar, a cost chip on every
      slot, and arming an unaffordable miracle explains itself in a toast
      instead of silently doing nothing
- [x] Belief regenerates: spent belief returns at a rate set by the sum of
      living faith (~2 minutes for a full refill at steady faith) — before
      this, `spent` accrued forever and two casts could lock the god out of
      miracles for the rest of the game
- [x] The founding pool buys one act: Smite costs 4 and founding faith is
      0.35, so a new god can cast exactly one wrath from the opening ~4.2
      belief — a taste, then the loop must be earned
- [x] Settlements sit back from the sea: the site chooser demands dry ground
      across the whole building band (out to ~36 units) and moves the
      shoreline reward out to fishing reach (42-58) — no more beachfront
      banners drowning half the village plan
- [x] Death rites: family and neighbours gather and weep over the dead
      (spirits fall, chronicles record it), then a bearer — the priest, when
      there is one — shoulders the body and carries it to a resting ground
      on the outskirts; the grave keeps the dead person's name and chronicle,
      and hovering the headstone reads the life back
- [x] Gossip grew manners: retellings of the same story stack into one line
      ("heard from Gayou and 2 others that...") and nobody is told the story
      of what they saw with their own eyes
- [x] Real windows: panels drag by their title bars, close from a corner
      button, and the toolbar buttons are toggles — any mix stays open
- [x] A reusable split-view (list + detail pane) in the UI kit; THE PEOPLE
      is its first client — roster on the left, and on the right a live 3D
      paperdoll of the selected person (their real body, rebuilt on a private
      stage, turning slowly) above their dossier
- [x] Wants, spoken: the detail pane lists what a person lacks — a full
      belly, a night's sleep, a roof of their own, someone to come home to,
      better days, a calling — derived from the needs the simulation already
      tracks
- [x] Word and thought bubbles: gossip is audible (the teller's rumor floats
      over their head), prayer is a private thought ("Krepur, we are hungry.
      hear me"), and family grief thinks the dead one's name — thoughts in
      soft parentheses, for the god's eyes only; capped at seven at once so
      sparse stays meaningful
- [x] Needs drive the village: retrain became a standing labor audit — no
      woodcutter with timber low calls one, unbuilt homes call a carpenter,
      waiting foundations a mason, and the most crowded trade gives someone
      up (never its last member). This healed a real deadlock: a founding
      roll with no forester used to stall wood, houses, and the fire forever
- [x] Needs drive feet too: the unwed seek company — evenings especially —
      instead of waiting for marriage to wander within twelve units of them
- [x] Window polish: the people list fills the instant the window opens, and
      the scroll wheel over any window belongs to the window — it no longer
      zooms the world underneath
- [x] Game-grade window chrome: edge-to-edge title bars with a gold thread,
      drop shadows, corner studs, inset list wells, ruled section headers;
      windows open centred on the left and drag anywhere
- [x] THE PEOPLE as a full dossier: portrait plaque, every hover-card stat in
      aligned rows, WANTS / HAS SEEN / LIFE sections, zebra roster with a
      gold selected row, and a follow chevron per name
- [x] THE VILLAGE ledger (toolbar bar-chart button): a big centred window of
      cards and bar gauges — souls, houses, believers; happiness, fed,
      housed; faith and believers; stores — with one line for the land
- [x] Small talk: villagers voice their state or trade every little while —
      speech in company, private thought alone — so bubbles live in
      peacetime; sermons are audible; the hand's fingertip is calibrated to
      the cursor and its hover magnetism tamed
- [x] Fields level the ground: tilling flattens a real terrain pad that
      rolls back into the hillside (chunks rebuild), with a furrowed bed and
      crops as individual leaning stalks that rise as they grow
- [x] The larder made visible: a stone pile and food sacks flank the
      woodpile; Storehouse (pop 8) and Granary (pop 13) join the civic
      ladder with their own silhouettes
- [x] Bodies take up room: grounded creatures ease each other apart instead
      of standing inside one another
- [x] More notices: fire lit, foundations laid, fields broken, plus the
      existing prayers, weddings, births, deaths, retrainings
- [x] The known world: villagers live inside a boundary of knowledge, marked
      by waystone cairns the village raises at its own edge; the ring moves
      outward as it grows
- [x] Explorers: a rolled trade (the bold) that musters alone, walks past
      the cairns, reads the land, and comes home with what is really there —
      a green wood, a berry heath, a stone slope, or nothing but wind;
      discoveries are fanfares, chronicle lines, and spoken tales
- [x] Discoveries feed the trades: known pockets count as workable ground,
      so a found grove is where the foresters go when home timber runs out
- [x] Speech rebuilt as a weighted pool (~90 lines): hunger, weariness,
      wounds, home, love, age, faith, doubt, witness, trade, and hour all
      offer candidate lines; pressing things speak louder but never alone
- [x] Pile hover cards: amount plus a 90-second trend ("being drawn down -
      about 2 a minute"); construction clears trees from its plot; wider
      people roster; lists scroll under the wheel

- [x] Seven more buildings, every one with a working effect: Well (6, the
      midday gathering spot), Smokehouse (11, fishers feed twice as many),
      Blacksmith 12 / Granary 13 / Tavern 14 as before, Mill (15, harvests
      half again), Weaver (17, warm cloth lifts the roofless ceiling),
      Herbalist (19, healers mend faster), Watchtower (20, wolves will not
      hunt in its shadow), Bakery (rations stretch) - fifteen kinds total
- [x] Traits: at most one virtue and one flaw, rolled at birth, never a
      contradiction - Diligent/Slothful (work pace), Devout/Skeptic (belief
      lands harder or glances off), Cheerful/Gloomy (spirits recovery),
      Chatty/Quiet (gossip rate), Hardy (endurance), Glutton (appetite);
      shown as the dossier's "manner" row, confessed in small talk

- [x] Save slots (three, guarded delete, load from title through the same
      window), everything captured down to per-tree forest state; the world
      pauses behind the title
- [x] Weather: eased fronts driving clouds, greyed light, rain, wind, felt
      temperature; rain waters crops, weather slows work, the fire eats
      faster, storms ground expeditions, cold wet nights grind the roofless
- [x] **False attribution lives**: storm lightning is the same bolt, harm,
      and witnessed event as a Smite, and it ignites trees - fire leaps in
      the wind until rain quenches it. A forced-storm test converted a
      witness and raised belief 4.2 to 7.5 with the god idle
- [x] The village window is tabbed (OVERVIEW / FAITH: who believes and the
      chronicle line that made them); tab bar is a kit piece

**Agreed, next:** Guards and expedition parties (explorer + one or two
guards; wolves as expedition danger; combat verbs for villagers). The
life-cycle/calendar decision (recommendation: a day is a season) and
seasons riding the weather system. Then Milestone 3.75, "the world is made
of stuff" - substance states with one transfer rule (burning trees are the
first citizen), containers as objects, village animals, one ingestion
pipeline.

- Witness system: who saw what, from how far, in what context
- `Event` → `Interpretation` → `Doctrine`, with motivated (not random) misreading
- Doctrine surfaced as **quoted villager speech**, never as a stat block
- Two starting doctrines reachable from the same act, so misinterpretation is visible
- Divine Power generated by belief; Belief as the ceiling on what is possible
- **Powers gated by doctrine**, not by a purchase menu

Success test: a player can perform the same action twice and get two different doctrines,
and can explain afterwards why.

---

## Milestone 3 — The nudge levers

Making steering possible without making it dictation.

- Storytellers: villagers who propagate what they saw
- Audience staging — carrying an interpreter to an event actually changes doctrine
- Signatures: cheap acts that come to predict expensive ones
- Silence as a lever (selective non-response is legible)

Success test: two players with the same opening world end up with visibly different gods.

---

## Milestone 3.5 — A working village

The simulation depth pass, agreed 2026-07-27: Dwarf-Fortress-style depth comes
from simple systems interacting, and these are the systems.

- [x] **Jobs**: eleven trades chosen by temperament and filled by demand, each
  a scorer in the existing utility AI, not an assignment
- [x] **Goods**: a settlement stockpile (food, timber, stone) that jobs feed and
  construction drains; scarcity is what makes prayer mean something
- [x] **Construction**: villagers *build* the settlement — houses with variety,
  then the civic ladder up to shrine and town hall; masons and carpenters
  carry every block and log to staged build sites
- **Seasons**: the calendar gains a year; growth, food and grass colour follow
- **Weather**: rain and wind riding the same `Sky` state; drought is Milestone 4's
  crisis and weather is what makes it legible

Order matters: jobs need the calendar (done), construction needs goods, the
shrine needs construction — and the shrine is where doctrine becomes visible.

## Milestone 4 — Crisis, and the peacetime/crisis rhythm

- Drought with real consequences
- Prayer as a request queue with named villagers attached
- Rain, gated behind a water association the player had to build earlier
- Losing because you taught them the wrong thing

Success test: a run can be lost during peacetime without the player noticing at the time.

---

## Milestone 5 — Legibility and feedback

- Villager inspector: needs, current task, utility scores, memories, opinion of you
- Event log in villager voice
- Objectives (short, medium, long)
- Miracle telegraphing and aftermath readability

---

## Milestone 6 — Look pass

Deferred deliberately: the aesthetic is de-risked, the fun is not.

- Tilt-shift depth of field (the signature HD-2D move)
- Day/night as palette shift
- Weather particles, light shafts
- Water surface shader
- Readability at maximum zoom-out (villagers are ~4 px)
- Doctrine-driven clothing and generated shrine geometry — *seeing* the religion

---

## Milestone 7 — Persistence and scale

- Save/load (deferred until the simulation stops changing shape)
- Population beyond 50; profiling before optimising
- Coherence and schism
- Births using the heritability already in the genome

---

## Explicitly not now

Resurrection, terraforming, full procedural theology, multiple settlements, warfare,
written history, generational simulation. Each is real design work and none of it helps
until the loop above is fun.

## Known debt

- The tuning HUD is function keys. `bevy_egui` when it outgrows that.
- Wildlife has no behaviour; deer, wolves and boars only wander.
- Children never grow up. Age is set at spawn; newborns stay small forever.
- River courses do not branch or merge, basin endings are crude ponds, and the
  first query in a fresh region traces its neighbourhood in one go — a one-off
  spike that should move to a background task.
- Pathfinding has no cache: every destination change is a fresh A*. Four routes
  per frame keeps it off the frame time, but a large population will queue.
- Chunk meshing runs on the main thread. Three chunks a frame keeps it invisible,
  but a fast-moving camera will outrun it; needs `AsyncComputeTaskPool`.
- Baked scenery cannot sway in the wind. The grass shader proves the technique;
  extending it to tree canopies is now mechanical.
- Frame rate still falls to ~50 at maximum zoom-out, where most of the streamed
  view is in frustum. LOD is the answer.
- No LOD. Distant chunks carry full detail, which caps the view distance.
- Villagers do not know the world is endless — they wander a fixed radius around
  the settlement and would happily walk off into unloaded terrain.
- One settlement of twelve in an endless world is very sparse. Population and
  multiple settlements matter more now than more terrain does.

---

## The long game (recorded 2026-07-29, direction agreed with Baz)

The population cap is gone: growth is governed by shelter, food surplus and
land, and the town physically sprawls (house rings widen with population,
the settlement radius follows the furthest roof). From here, three arcs in
order:

**1. Doctrine — the egregore itself.** What the village believes the god IS,
grown from what they witnessed; epithets, prayers as a queue, doctrine
shaping villager behaviour. The game's soul; comes first so every later
crisis (raids, famines, schisms) lands as a theological event.

**2. Defense — walls and a standing watch.** Palisades first, stone later:
wall segments as civic works tracing the settlement's (now stretching)
perimeter, gates where the worn trails cross it. The Guard grows into a
small army: barracks, patrol circuits along the walls, iron arms from the
smelter (the iron economy's endgame sink). The castle crowns this arc — a
keep at the heart of a mature city, suburbs clustering outside the walls
(near-fission: satellite house clusters with their own wells, the same
machinery as distant colonies pointed inward).

**3. Goblins — the enemy worth walling against.** Encampments seeded far
out like deposits, growing over time, raid parties marching the trails at
night: loot, fire, fear. Explicitly AFTER defense exists — the village
needs survivability (done: rations, famine watch, scaled thresholds) and
walls before an enemy that tests them. A raid witnessed is a faith event:
the god who saved us, or the god who let it happen.

Multi-settlement fission runs through all three: a crowded, well-fed city
with known far ground sends founders past the cairns.
