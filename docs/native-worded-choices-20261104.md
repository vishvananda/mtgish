# Captured words for printed choices

Worker checkpoint `208b07007a9d874b312c27dfe5bd25b4c3de12a1` combines the
previous validated `31abfac8` foundation with native commit
`256e3edbc14801f01cdc50f9fb227125e774a875`.

Printed card/option labels can now capture their applicable `Words`. Ordinary
unchanged labels keep their previous identity; new variants are appended.
The engine, host, and scenario runner share one text renderer. Host headings
and rows use the same captured text, and the selected printed index and linked
card remain unchanged through replay.

As-enters choices read the prospective copy's words, with the same native rule
for carrying text changes from a permanent spell. They do not accidentally read
the underlying old copy or a resolving ability's unrelated words. The shared
`choose::option_as_enters` handles this automatically.

Primal Clay's generator output already prints **Wall** in the third label.
Its retained reviewed card had substituted a generic additional-type caption
to work around the old presentation limitation; restore the printed label on
the new checkpoint. Keep printed `Wall` in its copiable subtype writes and
preserve `copy.words`: native layers apply the change there. Pre-transforming
that subtype can apply chained text changes twice.

Urborg has a separate generator correction: a typed landwalk choice now names
the exact native keyword, rather than its whole family. Thus its printed labels
are `first strike` and `swampwalk`; the shared renderer can display `islandwalk`
after a Swamp-to-Island change. Snow and other supported native qualifiers are
retained by the same keyword-name path.

Validation of the combined checkpoint:

- Verified checked native workspace: **17,303 passed**, 139 targets, 15 existing ignored.
- Whole .NET solution: zero build warnings/errors; **1,216 passed**, one existing skip.
- Matched CPU-0 release benchmark, verification unset, 128 games per binary:
  **36.70 → 34.70 microseconds/decision**, both zero unfinished. No speedup claim.
- Standalone feature: 13 native choice controls; replacing captured rendering
  with printed text caused six actual runtime failures and seven controls passed.
- Generator: **1,014 passed**, 102 targets, two existing ignored. The new Urborg
  label test failed before the correction; the printed Primal Clay form passed
  as its control.
- All **84** whole retained implementation-stop records regenerate without
  holes and compile against the combined native API with `-D unused_must_use`.

The API/mechanics fingerprint is
`7f0caabdf9d585e06a8cc51d87f0f1ab72f1a6dbe3b9b649c76f3819bf21fd10`.
Proofs are in `~/card-lane/pipeline-unlimited-20261103/integration/worded-combined-proof/`,
the sibling `worded-choice-labels-proof/`, and
`/mnt/vizier/card-lane/generator-worded-choices-preflight-20261104/`.
The promotion receipt pins the tested commit. Native publication remains
disabled while the campaign is consolidated; semantic schema stays 176.
