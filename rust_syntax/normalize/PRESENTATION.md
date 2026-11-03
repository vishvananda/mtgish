# Presentation in schema 167

The normalized IR shape is frozen by `../ir/schema/167.txt`. The contract records
types, variant order, field order and field types, as read by `ir_schema`.
An intentional shape change requires a schema bump, a new reviewed contract,
and loading the grammar patterns against the new dependency. Source vocabulary
enums outside the normalized IR are not part of this structural contract.

`Ability::WithPresentation` adds an optional ability/flavor label and a reminder
flag to the ability it contains. It creates no new reference, result, target,
event-subject or holder scope. `Keyword::WithReminder` attaches a flag to one
keyword instance, including in grants and characteristic definitions. A keyword's
reminder belongs to that keyword node; other printed mechanics and explanatory
abilities use the ability flag. Reminder text comes from grammar templates,
never an opaque copy of the input.

Absence of a wrapper means no label/reminder. Clearing the last flag or label
reduces to the plain canonical representation, so edited values can print without
old presentation. Nonempty presentation is preserved by `canonical::abilities`
and `canonical::reduced`. Nested wrappers of the same type are invalid, and labels
have bounds and cannot contain the label separator or control characters.

`canonical::semantic_abilities` explicitly removes presentation before computing
the ordinary normal form. It is for meaning comparisons with the earlier gold,
not exact round-trip verification. `presentation::erase_abilities` walks only
typed positions, preserves opaque lookalikes, bounds work before cloning input,
and leaves the input unchanged on refusal.

These additions do not recover presentation previously discarded by the Go
parser. Existing source exports still omit it. The phrase grammar must capture
the labels and flags from Oracle, and reporting must keep exact presentation
and normalized meaning as separate gates. Cantrip expansion remains deferred.

To generate a proposed contract explicitly, run the ignored
`ir_schema::contract::dump_schema_contract` library test with `--ignored
--nocapture`, capture only the text between its markers, review it, and run the
ordinary frozen-shape test. The generator never overwrites a checked fixture.

The legacy `rust_syntax` source dependency is frozen independently at published
revision `449b4bcf461fd6476cb2c93b78c2b2ff2c78ce28`. Its `src/` and manifest
are byte-identical to the parent parser at this split. Normalized-IR edits can
reuse that package build instead of rebuilding the large unchanged parser.
Changing source enums requires explicitly updating this dependency revision as
well as reviewing generated adapters. The workspace source package remains
available to its own tools; the IR library uses the explicit git dependency.
