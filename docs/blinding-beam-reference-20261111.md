# Blinding Beam: creatures and the named player's untap step

The oracle-grammar shared-clause packet confirms the still-open parser defect
reported in [upstream issue183](https://github.com/i5jb/mtgish/issues/183).
The second mode says “Creatures don't untap during target player's next untap
step.” It does not restrict creatures to those controlled by that player.

The old Source selects only that player's creatures and applies each object's
controller's next untap step. The fix selects every battlefield creature, sets
`RuleChange::UntapStep.players` to the target player, and scopes the effect to
`Duration::DuringNextUntapStep(target)`. Both paid/unpaid entwine branches retain
their announcements, counts and first mode (tap exactly two target creatures).
`LiveObjects` is preserved. A creature controlled by somebody else that would
untap in that player's untap step, for example through Seedborn Muse, is also
covered. Wizards describes that cross-player untap interaction in
[It Takes Two](https://magic.wizards.com/en/news/feature/it-takes-two-2013-08-29).
This is a source/rules witness, not a newly executed gameplay scenario.

The normalized AST already supports the exact operands. The Source vocabulary
needs one explicit player-set rule, `DoesntUntapDuringPlayersUntapSteps(Players)`,
so the parser does not substitute an affected object's controller. This is
appended to the Source enum, preserving existing variant order, and lowers to
the existing rule. Grammar declaration, Rust Source type and both TypeScript
Source exports agree. No normalized schema, canonicalization, numeric-width,
engine or Wasm interface changes are required.

The regression compares the entire corrected card's abilities against the
independent report's complete expected programs, then checks the affected set,
step player, duration and live evaluation separately in both entwine branches.
The complete-card Go fixture reads the actual checked-in Oracle entry.
Unrelated controller-relative untap templates are retained as controls.
The latest review catalogs had already adopted the preceding reference fixes,
including Trygon Prime; this is the new confirmed shared-clause defect.

Evidence: `/mnt/vizier/reference-beam-20261111`. The original question/report
packet, reference pins, comparison aliases and gold are untouched. A sibling
resolution note supplies the fix/proof links to the oracle-grammar worker.

Validation: full Go suite passes, including the updated regression for issue183.
Six focused Rust tests pass (Blinding Beam, Trygon Prime and four previous batch
controls). Fresh Source and normalized exports contain 33,529 records: only
Blinding Beam changes and 33,528 remain unchanged; schema188 stays identical.
The normalized complete programs equal the independent proposal after ordinary
canonicalization of its four unused result declarations. Both target-player
untap rule operations match every other operand exactly. Tagged JSON and pretty
RON are refreshed. The earlier local regression/audit note incorrectly asserted
a controller restriction; the regression now rejects it and the audit note
records the correction while retaining its prior output/reason for provenance.
All other review entries and unrelated controller-relative templates remain.
