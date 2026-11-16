# Aladdin's Lamp: Source contract and retained-card lowering

## Correction after upstream feedback — 2026-11-16

Our original parser-defect claim was wrong. The maintainer explains that the
inspection list allocates cards among destinations; a member cannot always be
interpreted as an independent move/reorder. In the draw-replacement partition
`[PutAGenericCardOnTopOfLibrary,
PutTheRemainingCardsOnTheBottomOfLibraryInARandomOrder]`, the first member
reserves one card and the second bottoms all others. It already represents
Lamp's printed instruction. See [the response to MTGish #290](https://github.com/i5jb/mtgish/issues/290#issuecomment-6930590089).

The fork restores that upstream English/Source expansion and removes the two
appended `LeaveAGenericCardOnTopOfLibrary` Source variants and their TypeScript
forms. Earlier discriminants are unchanged. Lamp's RON, tagged JSON, pretty RON
and active fixtures use the upstream form again. The independent reference
values remain unchanged.

Lowering retains the draw-replacement family before ordinary-action retagging.
For this complete two-member partition, it chooses one card as possible,
excludes its Selected receipt from the remainder and randomly bottoms that
remainder. It emits no additional top reorder or fabricated move receipt.
The following Draw remains a real Draw, preserving other draw replacements.
This is structural matching on the Source family and complete action sequence,
not a card-name or Oracle-text override. Other inspection sequences keep their
existing explicit ordering behavior. Telling Time is a printed positive control;
a different draw-replacement sequence is a structural negative control.

The corrected AST and all other corpus AST values are expected to remain
unchanged; validation receipts are in the follow-up decision record below.
No published schema, crate, native grammar or engine API change is needed.

## Earlier work and current ownership

The 2026-11-14 change introduced the extra Source variants because our lowering
had emitted an extra top reorder. Its game-result distinction and the corrected
AST remain useful, but the constructor name was not proof of a parser defect.
The original upstream request and our report comments are withdrawn.

[Oracle Grammar #26](https://github.com/vishvananda/oracle-grammar/issues/26)
corrected the AST reference through U0039 in grammar-v0.2.2. That reference is
still appropriate; it needs no reversal. Its explanation must attribute the
old extra reorder to lowering, not to an incorrect upstream Source program.

Current decisions, Snowfall's separate unresolved ruling and validation:
[upstream-feedback-289-290-20261116.md](upstream-feedback-289-290-20261116.md).
