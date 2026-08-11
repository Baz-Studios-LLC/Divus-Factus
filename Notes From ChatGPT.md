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

The corpus now contains **3,531 records**, all with unique text.

Batch 05 added 288 quarrel records:

```text
assets/voice/quarrels_hunger_depth_05.json   96
assets/voice/quarrels_roof_depth_05.json     96
assets/voice/quarrels_grudge_depth_05.json   96
```

The completed soak found correct speaker orientation, healthy repetition, and
roughly two quarrels among twenty-seven conversations. Brett considers that
rarity appropriate: hostility should happen when simulation truth creates it,
not because a drama quota fires. Repeated grounded quarrels can create grudges
over the longer game without intervention.

Batch 06 answers the quiet-run demand for wavering birth conversations:

```text
assets/voice/birth_conversation_depth_06.json   192 records

48  event:delivered + heard + tell + wavering
48  event:delivered + reply + wavering
48  chat:followup + event:delivered + told + wavering
48  chat:end + event:delivered + told + wavering
```

The batch deliberately mixes uncertainty about divine involvement with
ordinary concerns: rest, food, cloth, firewood, visitors, names, the healer,
parents, siblings, and household work. Birth conversations should not all
become theological debates.

Batch 06 validation:

- JSON, exact tag counts, 18-word limit, capitalisation, punctuation,
  British English, anachronism filter, and exact duplicates: zero errors
- stop-word-normalised similarity audit at a 0.72 threshold: zero near hits
- all Sermo vocabulary, subject-truth, repetition, sentence, and English gates
  pass in the full suite
- full suite result: 433 passed, 1 failed, 3 ignored

The single failure is unrelated to corpus work and reproduces alone:

```text
villager::names::tests::the_name_space_is_enormous
only 55514 distinct names in 100000 draws
```

Claude owns that Rust test and the concurrent name-generation changes. ChatGPT
did not touch them.

## Next Engine Contract

The next social feature should be engine-selected quarrel outcomes. Start with:

```text
reconcile   admission, apology, or practical offer; some regard repair
withdraw    refusal, exhaustion, or separation; grievance remains
escalate    insult, threat, shove, or fight; stronger consequences
mediate     a third party intervenes; outcome depends on trust and status
```

Choose the outcome before selecting the closing line. Corpus text must express
an engine truth, never secretly decide mechanics. Dialogue, regard changes,
movement, violence, and witness reactions should all describe the same event.
The first implementation may ship reconcile, withdraw, and escalate for two
participants, then add mediation once third-party participation is real.

When the outcome vocabulary and live emitters are ready, leave here:

- the exact locked tags and which beats receive them
- which speaker receives each tag
- how personality, need, regard, kinship, faith, and witnesses weight outcomes
- the mechanical consequence attached to each outcome
- representative `voice-wanted.txt` demand from an honest soak

ChatGPT can then author a large outcome-specific batch across every live
grievance and both speaker sides.

## Make Consequences Visible

Brett's central design concern is that rich simulation often happens without
the player noticing. Quarrels need readable aftermath, not only four bubbles:

- posture, separation, following, shoving, intervention, or reconciliation
- a relationship-history entry naming the grievance and outcome
- witnesses who remember which side they believed
- later gossip, prayer, or renewed conflict referencing the same event
- injuries, mediation, punishment, or apology where mechanically true
- a Codex or settlement-feed entry only when important enough

The player should be able to answer who argued, what was real, which side each
person occupied, what happened, and whether it still matters.

## Grievance Truth and Provenance

Future charges need structured provenance before receiving corpus text. Sermo
must not invent a theft, insult, affair, failed duty, victim, commandment, or
past event merely because an argument needs material.

A useful grievance record preserves:

```text
claim and subject
real event, condition, duty, relationship, or object behind it
how the accuser knows: witnessed, heard, inferred, assumed, or fabricated
source or clue
confidence and whether the accuser knows the claim is false
age, repetition, and whether it has been resolved
```

This permits true accusations, credible mistakes, disagreements over intent,
rumours with a known source, and deliberate lies with an actual motive. Every
participant should be able to answer “Why do you believe that?” from
information they hold.

## Sermons, Prophets, and Commandments

After quarrel outcomes and visible aftermath, build public speech. A sermon is
a stateful scene rather than unrelated bubbles:

```text
gather -> opening -> claim -> example -> demand or reassurance -> closing
```

It carries one topic and interpretation throughout. Listeners may agree,
question, object, leave, heckle, or begin a later argument according to faith,
personality, relationships, and witnessed truth. Crowd responses should be
sparse enough that the preacher remains readable.

Prophets are a strong player-facing channel for decrees. Preserve the exact
commandment, issuer, messenger, witnesses, interpretations, compliance,
violations, enforcement, amendment, and repeal. These facts should deepen the
existing providence-versus-dread identity rather than create a detached moral
slider.

## Dark Religion Must Be Contested

Sacrifice, cannibalism, persecution, forced conversion, excommunication,
scapegoating, sacred extraction, inherited guilt, denial of burial, martyrdom,
and holy violence should emerge from pressure, doctrine, leadership, and
player choices rather than random town-wide switches.

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

Causes and consent matter. Survival cannibalism, funerary consumption, ritual
consumption, and punitive desecration are not the same act. A sacrifice may
involve a volunteer, condemned person, political target, lot, or supposed sign.
People must disagree before, during, and after dark practices. They should
sound like ordinary people making, resisting, or regretting terrible choices,
not theatrical cultists.
