# Waterspout Warden and Storm Entity: explicit self referents

Both outstanding items are resolved by changing their actual Source producers.
They were retained in the earlier gameplay-only review because their event
subject equals their source. That establishes equivalence for these frames; it
does not justify emitting different forms for matching words. This is a
consistency correction under [MTGish issue 275](https://github.com/i5jb/mtgish/issues/275),
not a newly demonstrated gameplay defect.

Waterspout Warden's self-attack event names exactly `ThisPermanent`. “Another
creature entered ... this turn” therefore excludes that source. An EventHistory
query does not rebind the enclosing ability to a different attacker or to one
of the historical entering creatures. All eight equivalent self-attack templates
now bind their intervening condition's `ThatPermanent` to `ThisPermanent`.
Their bodies retain their existing independent bindings. This changes 17 cached
records, including conditions on granted abilities and a sticker sheet.

Storm Entity's own entry-counter clause excludes its source from the spell
history count, as Thrasta, Thunder Salvo and Call Forth the Tempest already do.
The Source operand is `ThisSpell`, which lowers to IR `Source`; the Source
spell-filter type does not accept `ThisPermanent`. This spelling does not impose
a stack-zone or was-cast requirement on the entering creature.

The shared entry-quantity grammar now takes an explicit `ThatSpell` argument.
Its three own-entry consumers pass `ThisSpell`. Its six other consumers keep
`ThatEnteringPermanent`, including a rule that changes other creatures' entry.
Other prospective-entry observations, such as mana spent to cast the entrant,
remain unchanged. Only Storm Entity's cached record changes from this family.

Fresh parser runs before and after produce the same 33,570 records and refuse
the same 1,477 inputs. Exactly 18 records change. All three cached Source
representations contain those same 18 changes; the other 33,511 records are
unchanged. The 24-record regression includes every changed record and six
controls: Bristlebane Outrider, Wary Farmer, Thrasta, Thunder Salvo, Call Forth
the Tempest and Lagrella. Full-record test helpers now handle sticker sheets,
whose Source representation has no redundant card-name field.

Raw IR regressions check both positive cases and two counterexamples: a trigger
on an arbitrary attacking creature must keep EventSubject in its historical
exclusion, and a replacement modifying another entering creature must keep that
entrant's EventSubject. A composed English parser test verifies the latter
distinction too. Each synthetic control is a test input, not an Oracle or gold
edit. Complete programs validate and round-trip.

No generic lowering-context rewrite or new comparison alias is included. The
previously rejected rewrite remains excluded; nested event frames, prospective
controllers and return callbacks retain their existing contracts. No AST layout,
schema, quantity width or Cantrip feature is changed by this reference batch.
The original 58-record proposal fixture and its expected output remain historical
and unchanged. Oracle Grammar's worktree, independent gold and reference pin are
untouched.

For the Oracle Grammar worker: use the new producer forms when advancing the
reference pin. These two faces now join the four previously corrected faces in
the reopened consistency review: **6 completed, 30 pending**. Their original
gameplay-equivalence decisions remain historical in the ledger; the
`consistency_review` fields record the current resolution.

The companion [receipt](reference-self-frames-20261112.proof.json) records final
validation and input hashes. Evidence is under
`/mnt/vizier/reference-self-frames-20261112`.

Final validation: full uncached Go suite PASS (83.027 seconds); complete Rust
workspace with Cantrip PASS (2,022 tests, 24 ignored, 198 targets). The current
producer snapshot changes exactly two expected operands to Source. The
independent historical 58-record proposal and expected file remain unchanged.
