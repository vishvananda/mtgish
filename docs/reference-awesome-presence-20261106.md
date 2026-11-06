# Awesome Presence declaration cost

The [maintainer's response to issue 33](https://github.com/i5jb/mtgish/issues/33#issuecomment-6658070401)
clarifies that `CantBeBlockedUnlessCost` applies its inner cost separately to each
creature declared blocking the subject. Its argument is plain `PayMana({3})`.
The fork's extra `PayManaForEach` multiplied that cost by the blocker count again.
Creatures created blocking, such as Mirror Match tokens, do not pay declaration
costs. This withdraws the earlier claim that the source form could not express
the printed per-blocker payment.

The English template and source fixtures now retain the plain cost. The existing
Go regression rejects the multiplier, and the Rust regression requires exactly
one `Mana([Generic(3)])` part with the declaring player as payer. Both checks
failed before the correction and pass after it. No source or IR schema change
is needed.

Fresh before/after parser runs each produce 33,529 records and differ only on
Awesome Presence. Regenerating the checked-in corpus also incorporates twenty
previously committed Aura-template corrections; those are refreshes of existing
producer fixes. All 185 audit findings retain identical complete usage evidence
(37 contextual, 148 automatic). The complete Go and Cantrip-enabled AST suites
pass. Typed binary/JSON and pretty-RON exports succeed; record identities and
layouts are checked against a fresh manifest of the actual pre-change corpus.
The historical baseline manifest already differs from that pre-change corpus and
is not used as evidence of identity preservation here.

The new worker queue is cataloged separately in
[reference-followup-20261106.json](reference-followup-20261106.json). Its worker
classifications still require current-producer review; representation questions
are not treated as confirmed gameplay defects.
