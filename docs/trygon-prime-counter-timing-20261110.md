# Trygon Prime: coordinated counter placement — 2026-11-10

Trygon Prime is an adjacent seventh instance of the parser defect documented in
[counter-placement-timing-20261110.md](counter-placement-timing-20261110.md).
Its one shared placement verb was incorrectly split into two successive
`PutCounters` actions. The corrected exact English template produces one
`PutCounters([source contribution, optional target contribution])`, followed by
the existing continuous effect making the chosen creature unable to be blocked.

“Other” keeps the target distinct from the attacking source. It does not change
the simultaneity of the shared placement instruction (CR608.2f). The target stays
optional, attacking and a creature. The later unblockability effect still reads
the original chosen target, independently of whether any counters were actually
placed on it, including under Solemnity. It does not read a placement receipt.
The instruction boundary before that follow-up is preserved.

Only Trygon Prime changes in the fresh 33,529-card source and native exports;
33,528 other records and schema188 remain unchanged. Tagged JSON and pretty RON
are refreshed with the same source change. No lowerer, canonicalization, engine,
numeric-width or schema edits are required. Existing engine batch support is
unchanged; no new card-level engine scenario is claimed.

The new Go fixture parses the full Oracle card into the complete corrected
source. The native regression compares every ability operand against a pinned
expected program, checks the single two-contribution batch, optional target and
independent original-target follow-up. The previous six-card controls also pass.
Full Go tests and full browser corpus roundtrip/binding/Wasm validation cover
the refreshed catalog. Audit review entries are unchanged; only the two current
grammar fingerprints are refreshed after confirming this exact template does
not overlap any saved findings.

Upstream: follow-up on [issue259](https://github.com/i5jb/mtgish/issues/259).
Evidence: `/mnt/vizier/trygon-counter-20261110`; the adjacent machine-readable
proof records the exact corpus and audit boundaries. The original oracle-grammar
question packet and gold are untouched.
