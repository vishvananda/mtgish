# Text-adjusted landwalk grants and removals

Scarwood Hag's two resolving effects used a fixed Forestwalk keyword. After Mind
Bend changed Forest to Island in its text, the first ability still granted
forestwalk and the second failed to remove islandwalk.

Resolving keyword grants and removals now use the existing word-aware landwalk
mapping. They read the source's type word as the effect resolves (CR 612.1–2),
then store the resulting keyword mask in the continuous effect. Snow qualifiers
are preserved. Static removals and chosen-ability removals use the same mapping;
the native removal authority expands ordinary references to their snow variant.
Other keyword masks retain their previous output.

The retained removal scenario fails before the fix, as does a new grant scenario
with the exact four-green activation payment. Two unchanged-word controls pass
before and after. The exact fresh generated module passes all four scenarios.
An initial ambiguous setup payment in the new scenario was corrected before
recording its credited keyword assertion failure.

Validation: **1,005 generator tests** across 99 targets, two existing ignored;
all **84 complete stopped-card modules compile with `-D unused_must_use`** against
the unchanged native checkpoint `01adc773bd2435046d08de1b5b698620eded28a4`.
Proofs, complete modules, scenarios and hashes are in
`/mnt/vizier/card-lane/generator-landwalk-words-proof-20261104/validation.json`.

Among the same 84 source captures, Scarwood Hag, Part Water and Urborg change.
Part Water's already-approved output needs this correction during consolidation.
Urborg still has a separate choice-label defect; this correction alone does not
justify retrying it. No native API or fixture data was changed.
