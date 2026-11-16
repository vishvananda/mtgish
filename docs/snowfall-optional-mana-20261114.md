# Snowfall's optional snow replacement — disputed interpretation

The original 2026-11-14 fix added a one-blue alternative to the snow branch.
That rules interpretation is now disputed, not independently verified as an
upstream parser defect. [MTGish #289's response](https://github.com/i5jb/mtgish/issues/289#issuecomment-6932193900)
says the second clause replaces the entire optional offer: snow offers zero or
two extra blue, while nonsnow offers zero or one.

Research found a close precedent pointing the other way. James Bennett's
published answer for Emeria Shepherd's matching double-may construction allows
return to hand, return to battlefield, or no return when a Plains enters:
[Cranial Insertion, Take your base](https://www.cranial-insertion.com/article/1753).
That is a first-hand explanation by a Level 3 judge, not a Snowfall-specific
Wizards ruling. A historical Matt Tabak quotation is linked from discussion,
but its original Tumblr page returns 404 and was not independently recovered.

XMage explicitly offers Snowfall zero/one/two; Forge offers zero/two. These are
conflicting implementations, not rules authority. CR 608.2c requires reading
later clauses together; CR 614.15 describes self-replacement effects. Those
rules alone do not settle the disputed scope of this second may. A clarification
request on #289 asks what distinguishes Snowfall from Emeria Shepherd.

The existing code, tests and U0038 reference remain unchanged while that question
is unresolved. This is not a claim that passing tests establish the ruling.
Neither interpretation requires an AST/schema change: both preserve the
Island's controller as decider/recipient, the nonsnow branch and the restriction
to cumulative-upkeep costs. The earlier outer optionality/controller fixes are
separate from this disputed extra alternative.

[Oracle Grammar #25](https://github.com/vishvananda/oracle-grammar/issues/25)
is notified of the disagreement. Do not automatically reverse U0038 or generalize
a zero/two rule to other double-may cards before resolving the precedent.
Detailed links and decisions:
[upstream-feedback-289-290-20261116.md](upstream-feedback-289-290-20261116.md).
