# Other creature targets and optional target populations

This follows the additional Jaheira/Kinetic Ooze note for MTGish #275 and the
subsequent operation-by-operation optional-target audit. The earlier reopened
36-face worklist remains complete; these are additional findings.

## Antecedents of “other”

For Jaheira, Heroic Harper, Kinetic Ooze, Rhino, Terrible Trampler and Amy Rose,
the later creature target excludes the creature that has the ability. It does
not exclude an earlier artifact, enchantment, land or Equipment target merely
because that target appears first. The common producer form is
`Not(SameObject(Source))` in that later creature filter.

Kinetic Ooze already has that form. Three English templates are corrected for
Jaheira, Rhino and Amy. Exactly those three cached Source records change; the
other 33,526 are byte-identical. Fresh parsing preserves the same successful
and refused input sets, 33,570 and 1,477. Source JSON, line RON and pretty RON
agree. No new target-distinctness constraint is added between the two slots.
An artifact creature can satisfy both target descriptions; each “other” here
excludes the source.

Evidence and its limits:

- [CR 115.3](https://media.wizards.com/2026/downloads/MagicCompRules%2020260619.pdf)
  permits reusing an object across separate instances of target, subject to
  their actual criteria. It does not alone identify an English antecedent.
- [Jaheira's published card](https://magic.wizards.com/en/news/card-image-gallery/alchemy-horizons-baldurs-gate-card-image-gallery)
  and [Kinetic Ooze's release notes](https://media.wizards.com/2026/downloads/SOS_Release_Notes_FwhcBWdFIE/EN_MTGSOS_ReleaseNotes_20260410.pdf)
  supply the wording. Neither provides an additional ruling that resolves this
  particular antecedent question. The interpretation of their English wording
  is a producer decision, not a quoted card-specific ruling.
- Rhino's Japanese printed card explicitly uses `これでないクリーチャー`,
  creatures other than this object. The actual printed scan and API response
  are retained in the evidence directory; the scan is primary card evidence,
  not an inference from a retailer's paraphrase. Its
  [printing record](https://scryfall.com/card/msc/730/ja) identifies the edition.
- The earlier artifact/enchantment and later creature descriptions can overlap.
  This decision does not rely on treating card types as disjoint, nor introduce
  a global rule that differing noun types always select the source.

Explicit target-to-target contrasts remain: Band Together excludes its first
chosen creatures, and Bathe in Light names its original target. Common Bond
retains two independent mandatory target slots without an invented exclusion;
Guardian Gladewalker may target itself because its wording has no “other”.

## Zero-or-one populations

The operation split was an incomplete lowering path, not an intended difference
between damage, counters, attaching and other effects. The actual producer now
normalizes a statically `UpTo(1)` target population to `Targets(slot)` wherever
it is used as an object, player or damage-recipient population. This includes
all movement destinations, counter recipients, damage recipients, attachments,
counterspells and ordinary effect subjects. The previous recipe-specific helper
is removed.

Scalar identity fields remain scalar: attachment hosts, property reads,
conditions and explicit members of larger target groups do not become sets.
Mandatory single-target populations retain the existing scalar convention.

Required payments need an additional distinction. A singular payment must not
become payable just because the target population is empty. Movement,
sacrifice and tap-state cost selections normalize their population and retain
an explicit `Exactly(1)` payment count when the old singleton supplied it.
Unquantified cost forms retain their existing scalar representation; they are
excluded from the generic effect-population rewrite. Existing explicit payment
counts are preserved. This is semantic cardinality, not an operation-name
exception for ordinary effects.

The same typed traversal keeps lexical target scopes: modes and created
triggers can shadow captured slots; independent granted abilities cannot borrow
the creator's target cardinality. Created event subjects are rebound before
population normalization, making one walk idempotent. Delayed closures retain
their captures. A zero-or-one target query retains a proven singleton without
allowing an arbitrary element of a plural unordered group. Blue Mage's Cane
and Niko Aris are concrete scope/receipt controls.

## Validation and adoption

The focused packet contains ten complete Source records and compares strict and
unified lowering, validation and round trips. Nine unit tests cover operations,
all movement destinations, scalar fields, scope isolation, idempotency and
required payments. Historical proposals and saved migration inputs remain
unchanged. Test-only data records thirteen fixture/card pairs and fifteen exact
before/after paths, checking all other fields unchanged. It does not participate
in production, canonical equality, Oracle Grammar's scorer or migration.

The portable schema stays 190. There are no AST payload fields or enum variants,
quantity widening or engine features in this correction. The producer uses its
existing typed walk with reusable scope buffers; it does not serialize nodes to
JSON or clone a card to normalize it. Additional traversal hooks have default
no-ops for other visitors. Browser workspaces are not migrated.

The complete Rust workspace passes **2,061 tests** (24 ignored, 200 targets),
and the uncached Go suite passes. All **33,529** cached records lower, validate
and round-trip without unreduced forms or primitive dependencies. The generated
four-card target callbacks pass against Cantrip, and actual Amy attack scenarios
cover both zero Equipment targets and one artifact creature chosen for both
slots. The existing 62 generated foundation runtime scenarios pass; all 212
ready modules plus Reflecting Mirror's partial callback control type-check.
The frozen foundation scope and every card's gap list are unchanged (212 ready,
74 gapped out of 286). Jaheira's specialize event and Rhino's divided-target
backend requirements remain explicit; these checks do not claim either card is
fully executable in the backend.

The implementation revision, hashes and upstream publication link belong in the
accompanying proof receipt. Local reproducible evidence is under
`/mnt/vizier/reference-other-targets-20261112`. This task does not modify Oracle Grammar's worktree, gold,
reference pin or comparison logic. Its worker can adopt the
three Source corrections and refresh new-producer expectations independently;
no comparison alias should conceal a target-to-source difference.

Published implementation: `5408607bc4342aa3cdde1e764e2f405e71dccd39`.
Upstream Source corrections: [MTGish #275 follow-up](https://github.com/i5jb/mtgish/issues/275#issuecomment-6832226608).
The [proof receipt](reference-other-targets-20261112.proof.json) records the
final checks and preserved historical inputs.
