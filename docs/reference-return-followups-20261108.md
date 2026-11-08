# Followup actions after returning a melded permanent

Reviewed oracle-grammar's `post-return-action-cardinality-review.json`, the
unconditional keyword proposal, checkpoint49/batch50 reports, and the latest
`reference-fix-followup.md` against fork main 8a39117. Six current normalized
reference defects remain after eacc880's exile-to-return cardinality correction.

| Card | Corrected followup |
|---|---|
| Justiciar's Portal | Every actual returned component gains first strike. |
| Obzedat, Ghost Council | Every actual returned component gains haste. |
| Sundering Sentinel | Every actual returned component gains haste. |
| Teysa of the Ghost Council | Every actual returned component gains haste. |
| Essence Flux | Each returned Spirit receives its counter. |
| Hallowed Respite | Each returned component you control receives its counter; each other component is tapped. |

The lowerer records a proof tied to the exact return ResultId: its Exile-guarded
input must be the complete receipt of a one-input exile. Proven unconditional
snapshot keyword grants then use the complete return Transition After group.
Source/Granter references, explicit indexed reads, unrelated returns and
unproven plural inputs retain their original meaning. Fresh ability scopes,
branches, optional programs and iteration do not inherit an outside proof;
a return inside such a body may establish its own proof. Lexical delayed
callbacks keep the actual receipt context they already capture.

The two conditional programs use existing NoteObjects/Matching/Difference
operations to snapshot per-object membership before either action. The lowering
batches only constant counter placements or tapping the same returned object.
Both partitions are recorded before adding counters, so changed characteristics
or control cannot reclassify the later branch. This avoids serial per-object
counter actions and preserves simultaneous placement within each instruction.

The full-program fixtures preserve every other operand, target, payment,
duration, timing and Exile-zone guard. Phelia's ownership predicate and Vanish
into Memory's toughness read remain scalar; neither is widened by this change.
No Source data, parser rule, public AST type, normalized schema, generator or
Cantrip engine feature changes. Schema remains 179. This is a fork Rust-lowering
correction, so there is no additional upstream English-parser change to request.

Rules evidence: [the meld action rule, CR 712.21c in the June rules](https://media.wizards.com/2026/downloads/MagicCompRules%2020260619.pdf)
specifies finding both constituent cards and taking the same action on each.
The worker also cites the same rule renumbered 713.8c in the November rules.
[Official Innistrad Remastered release notes](https://media.wizards.com/2024/downloads/INR_Release_Notes_hzvmSd7KJG/EN_MTGINR_ReleaseNotes_20240903.pdf)
confirm that Essence Flux checks Spirit status after returning from exile.

## Validation

- All six complete Source/native pairs match independently reviewed repaired
  programs; typed validation and canonical idempotence pass.
- All 33,529 Source records normalize completely, without unsupported or
  primitive forms. Comparing complete native exports changes exactly these six
  records; every other record is identical, including Phelia and Vanish.
- Full AST suite: 1,334 passed, four existing opt-in gates ignored.
- Four full modules generate: Portal, Essence Flux, Hallowed Respite and Obzedat.
  Sundering Sentinel/Teysa retain unrelated digital Intensity backend gaps;
  their full AST programs still validate and match the reviewed repairs.
- The explicit native reference gate runs generated modules against the existing
  a2aabdfa6 engine. New scenarios cast Portal, Essence Flux and Hallowed Respite
  on single and melded creatures, cover owned/opponent-owned returns and mixed
  Spirit/non-Spirit components, and retain the older reference scenarios.
  All 22 native regression tests pass. No engine checkout is modified.

Reproduce the focused/full AST checks:

```sh
cargo test --manifest-path rust_syntax/Cargo.toml -p mtgish-ast --features cantrip --test return_followup_reference_fixes
cargo test --manifest-path rust_syntax/Cargo.toml -p mtgish-ast --features cantrip
```

The existing opt-in `reference_contract_runtime` gate also accepts
`MTGISH_CANTRIP_DIR`, `MTGISH_REFERENCE_PROBE_DIR`, and `MTGISH_CANTRIP_TARGET`;
run it with `-- --ignored --nocapture` to compile and execute the generated
callbacks. The new runtime scenarios are checked in beside the full-card
Source/native fixtures.

Evidence: `/mnt/vizier/oracle-return-followups-20261108/` contains focused/full
logs, native logs, complete expected/actual programs, generated modules, the
fresh 33,529-record export and its exact six-record delta.

The separate batch50 question about destination-replaced exiles in active
callbacks is an engine-realization investigation, not a confirmed reference IR
correction. It remains outside this AST repair. The new prevention-order and
singular-unless reports describe grammar additions or representation questions;
they request no additional current reference correction.
