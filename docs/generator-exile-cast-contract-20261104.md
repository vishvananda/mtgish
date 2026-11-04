# Exile-casting API contract correction

The selection-consolidation native probe found a pre-existing generator bug:
both `ExileCast` constructors omitted the engine's required `instead` field.
The permission for casting the source itself, and the permission for casting
other cards from exile, now explicitly supply `instead: None`.

Neither constructor accepts a replacement mana cost in its supported forms.
`None` retains their existing cost behavior; `Some(ManaCost::ZERO)` would instead
allow casting without paying the mana cost. Additional costs and the separate
life-for-mana-value option remain as emitted before.

The full generator suite passes **957 tests across 93 targets**, with two
existing ignored tests. A new regression covers Ian Malcolm, Chaotician,
Misthollow Griffin and Eternal Scourge, exercising both constructors. Their
exact full-source generated modules compile against the validated native API.

All **33,529** ordered output identities and capability gaps remain unchanged.
Seventeen whole modules change only by adding `instead: None`; a complete
before/after comparison verifies that no other generated tokens change.
This is separate from the AST selection refactor and requires no schema change.

Evidence is retained under
`~/card-lane/pipeline-unlimited-20261103/integration/exile-cast-costs-proof`
and the adjacent focused/full generator logs. The paired consolidation probe
retains the original native compilation failure as pre-existing evidence.
