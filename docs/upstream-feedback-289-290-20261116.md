# Upstream feedback on MTGish #289 and #290 — 2026-11-16

## Decisions

**Aladdin's Lamp (#290): accept the maintainer's Source contract.** The complete
draw-replacement inspection partition reserves a card; lowering must not emit
an additional top reorder just because its first constructor says Put. Restore
the original English/Source form, remove our appended Leave constructors and
lower this complete partition before discarding the replacement family. There
is no card-name override. A different inspection sequence and Telling Time's
printed top placement keep their real top reorders. The final Draw remains a
Draw. The AST/reference correction itself stands; its parser-defect attribution
was wrong. No owned schema or engine change is required.

- [Maintainer's explanation](https://github.com/i5jb/mtgish/issues/290#issuecomment-6930590089)
- [Our withdrawal](https://github.com/i5jb/mtgish/issues/290#issuecomment-6932699501)
- [Grammar attribution correction; keep U0039](https://github.com/vishvananda/oracle-grammar/issues/26#issuecomment-6932699869)

**Snowfall (#289): research found contrary precedent; do not reverse code or
U0038 until the interpretation is reconciled.** The maintainer interprets the
snow clause as replacing the complete optional offer: zero/two additional blue.
Our current code permits zero/one/two. We initially accepted the response too
quickly; subsequent verification does not establish that the reversal is right.

The decisive question is what the second may qualifies: applying the
self-replacement or doing the replacement action. CR 608.2c requires reading
later clauses with earlier ones; CR 614.15 describes self-replacement and its
priority. Neither, alone, proves how that may is scoped in this wording.
The maintainer's phrase “text replacement” is not being treated as an instruction
to add a layer-3 text-changing effect to the AST.

### Evidence, with limits

1. James Bennett's [Cranial Insertion, Take your base](https://www.cranial-insertion.com/article/1753)
   answers Emeria Shepherd's closely matching double-may wording: a Plains
   allows return to hand, return to battlefield, or no return. This is an
   independently authored Level 3 judge explanation, not an official
   Snowfall-specific ruling. Its direct statement is stronger than inferring
   the ruling from our own generated program.
2. Wizards' [Wilds of Eldraine release notes](https://magic.wizards.com/en/news/feature/wilds-of-eldraine-release-notes),
   Back for Seconds, explicitly allow declining the conditional may/instead
   battlefield replacement and retaining the return-to-hand result even when
   eligible. This is an official optional-self-replacement example. Its initial
   action is not another may, so Emeria remains the closer double-may analogy.
3. [XMage's Snowfall](https://github.com/magefree/mage/blob/f7944e98e8c5fd882820c31ef776d6924ca26ec4/Mage.Sets/src/mage/cards/s/Snowfall.java#L105-L125)
   explicitly offers zero/one/two. [Forge's Snowfall](https://github.com/Card-Forge/forge/blob/master/forge-gui/res/cardsfolder/s/snowfall.txt)
   offers zero/two. They are conflicting implementations, not rules authority.
4. A [Stack Exchange discussion](https://boardgames.stackexchange.com/questions/27524/returning-card-to-hand-off-emeria-shephard-from-plains-trigger)
   links an Emeria answer from Matt Tabak. The original Tumblr URL currently
   returns 404. We have not independently authenticated the primary answer;
   its quotation is a research lead, not proof used for a code change.
5. The cached Snowfall ruling from 2008-08-01 concerns the extra mana's color
   and does not settle whether one blue remains available for snow.

[Official Comprehensive Rules, effective 2026-11-06](https://media.wizards.com/2026/downloads/MagicCompRules%2020261106.pdf)
were checked. The frozen local Oracle input independently confirms both cards'
wording; no new snapshot or input population was adopted. The original #289
confirmed-bug claim is now qualified, our misleading historical report comments
are corrected, and [the upstream follow-up](https://github.com/i5jb/mtgish/issues/289#issuecomment-6932668510)
asks for the distinction from Emeria. [Grammar #25](https://github.com/vishvananda/oracle-grammar/issues/25)
is reopened as a clarification, explicitly requesting no automatic U0038
reversal. Its native grammar may remain unchanged while this is unresolved.

In either interpretation, preserve optionality, the Island controller as
recipient and decider, the nonsnow branch, and cumulative-upkeep-only spending.
Do not make a general replacement-rule change based on one disputed case.

## Validation

The parser regenerated the same frozen **33,529** records. Only Lamp's typed
Source record changes; its three serialized exports agree. The complete
regenerated Source multiset matches the checked-in values. All **182** reviewed
capture findings and matched-use evidence remain unchanged, with zero new
findings. Only reviewed grammar fingerprints advance.

The full **33,529-record AST export is byte-for-byte identical** to the export
before this follow-up, including Lamp and Snowfall. The existing independent
amendment references are unchanged; their check still passes with the existing
12 explicit outstanding records. Focused tests include Lamp's complete
select/remaining/real-draw behavior and explicit-put controls.

The complete Rust workspace passes **1,596 tests**, with 10 ignored
across 140 targets. The full Go suite passes; literal audit has zero
unreviewed findings. File hashes and report receipts are recorded
in the adjacent proof JSON. Scratch evidence:
`/mnt/vizier/mtgish-rejections-289-290-20261116/`.

No Oracle Grammar working files, Blocks deployment, Cantrip generator, engine,
reservations or workers changed. The 57-card backlog remains paused.
