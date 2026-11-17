# Snowfall's optional snow replacement — retain zero/one/two

## Adopted decision — 2026-11-17

The user consulted an external judge who agrees with zero/one/two additional
blue for a snow Island and instructed us to retain that interpretation. The
second may makes the self-replacement optional: declining it preserves the
original one-blue offer. A nonsnow Island continues to offer zero or one.

Keep the existing Source `Or([U, And([U,U])])`, lowered Optional containing
ChooseOne(Blue(1), Blue(2)), and the U0038 reference. Existing regressions assert
both snow amounts, the outer optionality, the nonsnow amount, the Island's
controller as decider/recipient and cumulative-upkeep-only spending. No code,
reference, schema or dependency change is required.

The external judge consultation is reported by the user; it is not being
represented as a newly published Wizards ruling or an upstream acceptance.
The maintainer's contrary interpretation remains recorded in
[MTGish #289](https://github.com/i5jb/mtgish/issues/289#issuecomment-6932193900).
The fork's decision is settled, and the Grammar clarification can close with
U0038 retained. The separate Lamp Source/lowering correction is already pushed.

## Research leading to this decision

Supporting precedent comes from Emeria Shepherd. James Bennett's
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

The existing code, tests and U0038 reference remain unchanged under the adopted
decision above. The earlier outer optionality/controller fixes are separate
from the choice between one and two additional blue.

[Oracle Grammar #25](https://github.com/vishvananda/oracle-grammar/issues/25)
records the decision to retain U0038. The earlier investigation and Lamp fix are
in [the follow-up](upstream-feedback-289-290-20261116.md).
