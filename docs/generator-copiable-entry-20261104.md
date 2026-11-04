# Preserve prepared copies during entry changes

Primal Clay exposed a generator defect in copiable entry changes. The generated
replacement rebuilt values from the entering object's underlying card, discarding
a copy already prepared by Phantasmal Image. A flying Primal Clay copied as the
Wall form therefore entered named Phantasmal Image without flying.

Entry changes now modify `entering.copy` when present and fall back to the object's
copiable values otherwise (CR 707.9a). They preserve copied names, keywords, types
and exception abilities while applying the new choice's size and additions.
The change applies to the generic copiable-entry path.

The retained native copy scenario fails before the fix; all three ordinary forms
pass. The exact fresh generated module passes those four scenarios plus a new
case asserting the copy's Illusion type and actual sacrifice trigger when
targeted. Removing only the prepared copy's added abilities makes that new case
fail at the two-item stack assertion; the other four still pass. Restoring the
exact generated module passes all five again. Compiler failures are not credited
as mutation evidence.

The full generator suite passes **999 tests across 98 targets**, with two existing
ignored tests. Comparing the same 84 stopped-card source captures against the
previous generator revision changes only Primal Clay. Source captures, complete
modules, scenario bytes, baseline/fixed/mutant logs and hashes are retained in
`/mnt/vizier/card-lane/generator-copiable-entry-proof-20261104/validation.json`.
