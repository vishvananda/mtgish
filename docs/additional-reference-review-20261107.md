# Additional reference review, 2026-11-07

## Reviewed inputs

Reviewed Oracle's committed batches after runtime adoption9c30e74 through d5db464:
attached static conditions, optional other-target antecedents, private graveyard
card filters, receipt-sized callback targets, and distributed counter targets.
Their reports distinguish frontend corrections from reference errors. The newer
batch declares no new confirmed reference error. Oracle has adopted fork4bf84f0
with all34,187 prior saved readings retained after its two documented old-graph
repairs; do not describe runtime adoption as still pending at that revision.
Source/IR model pins in that project remain independently frozen at0274d8f.

The two remaining commits on fork branch fix/mode-result-scope-20261107 are the
older607/41ba choice implementation and its merge. The published4bf repair was
independently reviewed and adopted by Oracle, so merging that older implementation
again would replace the reviewed main implementation. No additional branch was
merged, no upstream update was pulled, and Oracle's working files/pins are untouched.

## Halfdane exact duration

Halfdane's printed duration extends through the end of the player's next upkeep.
Using UntilNext(UpkeepBegins) is too early. Using the next draw step is not exact
when steps are added or skipped. Merely adding UpkeepEnds to UntilNext would also
be wrong when the effect is created during an upkeep: it could expire at the end
of that current upkeep.

A distinct Source Expiration::UntilEndOfNextUpkeep(Player) and normalized
Duration::UntilEndOfNextUpkeep(PlayerRef) now express the exact contract. The next
upkeep is one that begins after creation; an upkeep already in progress is not
counted, skipped upkeeps do not consume it, and extra upkeeps do count. Existing
UntilNextUpkeep remains a beginning-of-upkeep deadline for its eight other corpus
callers. Both variants are appended to preserve earlier binary discriminants.

This model extension increments the normalized schema once,178 to179. Validation,
inventory and generated type traversal include the new duration. Source and IR
consumers need a reviewed model-pin update before adopting this revision; the
older0274 model package cannot represent the new duration. No numeric ranges,
Cantrip engine features, or external project pins changed.

The generator refuses the exact unsupported duration explicitly. No approximation
or fabricated gameplay success is claimed. Source parsing, normalized semantic
validation, codec roundtrips and generator capability are separate checks.

Validation: all33,529 Source records parse and normalize, with exactly Halfdane's
Source record changed. All three Source formats are regenerated. Nine full-card
fixtures cover Halfdane and eight unchanged beginning-of-upkeep callers. Four new
tests preserve the complete program, distinguish beginning/end, validate the
player reference, check Source/IR binary roundtrips and idempotence, and verify
the explicit backend refusal. The full AST suite passes1,326 tests with four
opt-in gates ignored. The unsealed audit validates and canonically normalizes all
32,755 faces idempotently at schema179. The uncached full Go suite passes, and all
182 reviewed grammar findings and their matched-use evidence remain identical.
Historical immediate-receipt fixtures retain their original schema178 data; their
test harness updates only the header while testing the unchanged operands.

Evidence: /mnt/vizier/oracle-additional-20261107/.
Upstream Source request: https://github.com/i5jb/mtgish/issues/229.

## Older pending entries requiring further producer correction

The older catalog still contains source-confirmed, currently present defects on
Skulking Killer (creature filter), Back for More (optional reflexive target),
Time to Feed (immediate fight/target scope), and True Identity (one shared trigger
limit). Those are distinct from the newer batches' zero-new-error result.
Ancestral Mask's other-enchantment antecedent remains an explicit investigation;
the bounded primary-source lookup did not supply a decisive ruling, so no Source
edit or new confirmed-error claim is made for it.
