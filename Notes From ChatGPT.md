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
- use American English spelling
- write plain, concrete, ordinary speech rather than poetic prose
- preserve register, subject-class, faith-band, slot, and speaker-side truth
- avoid exact and near duplicates

## Current Corpus

The corpus now contains **3,787 records**, all with unique text.

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

Batch 07 answers the remaining measured demand for interpreting divine
provision and broadens the entire live event surface:

```text
assets/voice/provision_tellings_depth_07.json       144 records
assets/voice/provision_conversation_depth_07.json   112 records

12 each  saw + tell, with neutral/devout/wavering/doubting variants
12 each  heard + tell, with neutral/devout/wavering/doubting variants
12 each  distant + tell, with neutral/devout/wavering/doubting variants
24 each  reply, with devout/wavering/doubting variants
40       chat:followup
```

The event truth is narrow and preserved: food appeared for hungry people
without a human delivery visible to the witness. Direct witnesses describe
what they actually saw; heard and distant accounts acknowledge their weaker
provenance. Devout speakers credit the god, wavering speakers remain honestly
uncertain, and doubting speakers seek an ordinary explanation without denying
that hungry people ate.

Replies and follow-ups widen the social meaning beyond wonder: fair division,
who still lacks food, storage, spoilage, credit, witness agreement, possible
control of the gift, and what happens once the provisions are gone.

Current validation:

- JSON, exact tag counts, 18-word limit, capitalisation, punctuation,
  American English, anachronism filter, and exact duplicates: zero errors
- stop-word-normalised similarity audit at a 0.72 threshold: zero near hits
- all Sermo vocabulary, subject-truth, repetition, sentence, and English gates
  pass: 13 passed, 0 failed
- full suite: 439 passed, 0 failed, 3 ignored

## Audit of Batches 06 and 07 (from Claude)

Brett asked whether everything in the corpus is good. It was audited
independently rather than against the validation claims above. Result:
3,787 records, zero exact duplicates, every tag locked and live.

**Batch 06 passes on every count, including the ones no script checks.**
The birth conversations sound like people. "A neighbor told me the child
is doing well. I hope the mother is too." Ordinary concern carrying the
faith question rather than announcing it. This is the house voice.

**Batch 07 drifted, and it is measurable.** Both provision files sit far
outside the corpus in register:

```text
file                                    n    word len   abstract-noun lines
provision_tellings_depth_07.json      144      4.78            39.6%
provision_conversation_depth_07.json  112      4.81            29.5%
birth_conversation_depth_06.json      192      4.48            11.5%
corpus norm                                 3.95-4.50        2-12%
```

Three times the abstract-noun rate of everything else, and the longest
words in the corpus. In the text it reads as villagers arguing like a
debating society:

```text
If the god provided, the evidence will survive a proper count.
An answer that feeds people deserves attention, even from me.
The god provided the meal. Our duty is to divide it fairly.
I saw food appear, though I still cannot explain who answered whom.
```

Compare a batch 06 line doing the same job in plain speech: "Maybe the
god was listening. I know the neighbors were."

The doubting register is the likely cause. Scepticism was written as
courtroom reasoning - evidence, counts, testimony, claims - when a
sceptical villager is more often blunt or tired than forensic. "Food does
not fall out of the sky" is doubt. "The evidence will survive a proper
count" is a lawyer. Please rewrite batch 07 at batch 06's register, and
keep the abstract-noun rate near 12%: concrete nouns, short words,
and the doubt carried by tone rather than vocabulary.

## Corpus Edits Made Directly (from Claude)

Thirty lines across eighteen files were once changed the OTHER way -
`afterward` -> `afterward` - on a British rule that has since been
dropped. The whole game moved to American English on 2026-08-17: corpus,
labels, code and this contract. They are back to `afterward` and
`toward`. The English gate in
`sermo/corpus.rs` is a hand-written list of pairs and simply had no entry
for either; both are in it now, so this cannot recur.

Two more worth a decision, NOT changed, because they are vocabulary
rather than error and the world's own words are Brett's call:

- `creek` x13 - fine now, and the reason it was ever raised is gone: in
  British English a creek is a tidal inlet, but the village speaks
  American and a creek is a creek. `brook` and `stream` still sound
  older, if that is wanted for its own sake.
- `gotten` x1 (`replies.json`): "the world's gotten strange". Also fine
  now; it is the American form.

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
rumors with a known source, and deliberate lies with an actual motive. Every
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
