# Notes From ChatGPT

Living handoff for Claude. Keep current decisions and active work only; delete
completed or contradicted material rather than preserving project history.

## Collaboration Boundary

ChatGPT implements only Sermo corpus assets in `assets/voice/*.json`. Claude
owns Rust systems, emitters, vocabulary, tests, and all other game code.
Suggestions below are design proposals for Claude to evaluate.

`SERMO-AUTHORING.md` remains the corpus contract:

- use only locked tags supplied by live emitters
- keep every expanded utterance at 18 words or fewer
- use normal sentence capitalisation and punctuation
- use British English spelling
- write plain, concrete, ordinary speech rather than poetic prose
- preserve register, subject-class, faith-band, slot, and speaker-side truth
- avoid exact and near duplicates

## Current Corpus

Batch 05 is authored and validated. The corpus now contains **3,339 records**,
all with unique text.

New files:

```text
assets/voice/quarrels_hunger_depth_05.json   96 records
assets/voice/quarrels_roof_depth_05.json     96 records
assets/voice/quarrels_grudge_depth_05.json   96 records
```

Each live grievance has both `aggrieved` and `advantaged` speakers across
`chat:open`, `chat:reply`, `chat:followup`, and `chat:end`. Each combination
contains ordinary lines plus `devout`, `wavering`, and `doubting` variants.

The housed and fed speakers are defensive, embarrassed, confused, practical,
or willing to help rather than being written as automatic villains. Grudge
lines deliberately avoid inventing the old event because the engine currently
knows that resentment exists but not what caused it.

Validation after authoring:

- custom audit: 288 new records, 3,339 corpus records, 3,339 unique texts,
  zero errors
- full `cargo test`: 433 passed, 0 failed, 3 ignored
- Sermo vocabulary, subject-class, repetition, proper-sentence, British
  English, and live-request gates all pass

Batch 05 should now soak in play. Do not open another broad corpus batch until
the game reports a measured need or the quarrels reveal a visible weakness.

## After the Batch 05 Soak

Use the soak to identify repetition, mismatched replies, abrupt endings,
excessive hostility, unreadable crowd scenes, and consequences the player
cannot perceive. Then proceed in this order:

1. Fix specific corpus problems demonstrated by play or `voice-wanted.txt`.
2. Add engine-selected quarrel outcomes: reconcile, withdraw, escalate, and
   mediate.
3. Give those outcomes locked tags and author matching apologies, refusals,
   compromises, interventions, threats, and closing lines.
4. Make aftermath visible through memories, gossip, prayers, injuries, renewed
   grudges, relationship history, and occasional important Codex notices.
5. Add grounded grievance types such as work disputes, neglected duties,
   family tension, witnessed insults, and broken promises only when the
   simulation can prove or credibly source each accusation.
6. Build public speech: sermons, sparse crowd responses, prophets, and
   player-issued commandments.

The preferred priority is **quarrel outcomes first, visible aftermath second,
sermons and commandments third**. Make the existing social simulation feel
consequential before expanding its subject matter.

## Quarrel Contract Now In Use

Every quarrel line has exactly one of each:

```text
beat:       chat:open | chat:reply | chat:followup | chat:end
register:   quarrel
charge:     over:hunger | over:roof | over:grudge
side:       aggrieved | advantaged
faith:      optional devout | wavering | doubting
```

Do not author `over:idleness` or `over:faith`; those remain reserved until a
live emitter carries the corresponding truth.

The engine now preserves the aggrieved entity through all four beats and cools
relationships asymmetrically at parting. That solves factual orientation and
prevents an argument from accidentally using the friendly conversation result.

The next improvement should not be more hostile synonyms. It should be an
engine-selected **quarrel outcome or stance** that makes words agree with the
mechanical aftermath. A small first slice is enough:

```text
reconcile    admission, apology, or practical offer; some regard repair
withdraw     refusal, exhaustion, or separation; grievance remains
escalate     insult, threat, shove, or fight; stronger social consequences
mediate      a third party interrupts; outcome depends on trust and status
```

Choose the outcome before selecting the final line. Corpus text should express
an engine truth, never secretly decide mechanics. Personality, need, regard,
kinship, faith, witnesses, and prior quarrels can weight the choice later.

## Make Consequences Visible

Brett's central design concern is that rich simulation often happens without
the player noticing. Quarrels need readable aftermath, not just four bubbles:

- posture, separation, following, shoving, intervention, or reconciliation
- a short relationship-history entry naming the grievance and outcome
- witnesses who remember which side they believed
- later gossip, prayer, or renewed conflict referencing the same event
- injuries, mediation, punishment, or apology where mechanically true
- a Codex or settlement feed entry only when the event is important enough

The player should be able to answer: who argued, what was real, which side each
person occupied, what happened, and whether it still matters.

## Grievance Truth and Provenance

Future charges need structured provenance before they receive corpus text.
Sermo must not invent a theft, insult, affair, failed duty, victim,
commandment, or past event merely because an argument needs material.

A useful grievance record would preserve:

```text
claim and subject
real event, condition, duty, relationship, or object behind it
how the accuser knows: witnessed, heard, inferred, assumed, or fabricated
source or clue
confidence and whether the accuser knows the claim is false
age, repetition, and whether it has been resolved
```

This supports true accusations, credible mistakes, disagreements over intent,
rumours with a named source, and deliberate lies with an actual motive. Every
participant should be able to answer “Why do you believe that?” from
information they hold.

## Sermons, Prophets, and Commandments

Brett wants one villager to preach while a gathered crowd occasionally
responds. Treat a sermon as a stateful public scene rather than independent
speech bubbles:

```text
gather -> opening -> claim -> example -> demand or reassurance -> closing
```

The sermon carries one topic and interpretation throughout. Listeners can
agree, question, object, leave, heckle, or begin a later argument according to
faith, personality, relationship to the preacher, and whether the claim
matches witnessed events. Crowd responses should be sparse enough that the
preacher remains readable.

Prophets are a good player-facing channel for decrees. The player issues a
commandment through a chosen prophet; the prophet announces it; priests,
listeners, families, and settlements interpret it; compliance and violation
become observable social facts. Preserve the exact decree, issuer, messenger,
witnesses, interpretations, enforcement history, amendments, and repeal.

The Deity page already derives broad public identity from providence and dread.
Extend that biography instead of adding a detached good/evil slider. Track
descriptive tendencies inferred from behaviour:

```text
merciful or punitive
protective or demanding
consistent or capricious
present or distant
universal or partisan
```

Commandments issued, contradicted, enforced, ignored, or repealed should affect
that identity, as should which prayers are answered and whether the god stops
atrocities committed in its name.

## Dark Religion Must Be Contested

The game should allow the player to become a benevolent, terrible,
contradictory, distant, or reforming god. Human sacrifice, cannibalism,
persecution, forced conversion, excommunication, scapegoating, sacred
extraction, inherited guilt, denial of burial, martyrdom, and holy violence
should therefore emerge from pressure, doctrine, leadership, and player
choices rather than a random town-wide switch.

A reusable progression is:

```text
event, miracle, decree, or remembered coincidence receives an interpretation
prophet or priest states it publicly
villagers argue and factions form
authority defines who qualifies, benefits, and pays
a target or practice is proposed
families, doubters, rivals, and believers respond
the player confirms, forbids, interrupts, rescues, punishes, or stays silent
the outcome becomes doctrine, taboo, shame, grievance, or precedent
```

Keep causes distinct. Survival cannibalism, funerary consumption, ritual
consumption, and punitive desecration are not the same act socially or morally.
Track consent, necessity, secrecy, kinship, doctrine, and who profits. Likewise,
a sacrifice may involve a volunteer, a condemned person, a political target,
a lot, or somebody declared chosen after a sign.

People must disagree before, during, and after dark practices. Lines should
sound like ordinary villagers making, resisting, or regretting terrible
decisions, not theatrical cultists. The player should always be able to see
who benefits, who suffers, who objects, who stays silent, and which story
survives.
