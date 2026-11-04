# Typecycling movement validation follow-up

Independent review of the typed transfer checkpoint (`be6f556`, report
`2b52212`) found that `valid_cycling_body` still used `..` for the new movement
terms. The helper checked the hand destination and plain entry but could accept
nondefault ordering, placement or set qualifications inside the fixed
typecycling program. That weakens the keyword identity contract.

The follow-up binds the actual `MovementTerms` and requires `is_default()`.
It changes only the typecycling body validator. Ordinary movement and its
capability diagnostics retain their existing behavior. The backend already
guards richer terms before combined library recipes and reports unsupported
operations rather than erasing their terms; this investigation found a
validation hole, not a newly demonstrated wrong native module.

The new public whole-card regression starts from the real Pale Recluse fixture.
Its default program validates and produces a cycling activation. Independent
mutations add random order, a whole-set qualifier and a stated placement chooser;
all must reject specifically at cycling validation, including through the
generator's public API. Controls change the valid rich movement bodies to ordinary
hand-zone activations: those still validate and retain their backend gaps.

Before the guard, the new test fails because validation returns `Ok(())` for the
random-order mutation. After the guard, the library, scopes, movement-transfer
and Cantrip backend suites pass: **165 tests across four targets**, with two
existing ignored tests.
All checks are terminal. The previous corpus, native and complete-module proof
inputs were not rerun or modified: this guard cannot change a default-term
corpus program, and the earlier whole-native migration captured those defaults.

Receipts under `target/movement-transfer-proof/`:

- `cycling-guard-before.log`: expected failing regression, exit 101;
  SHA256 `b6bed99c39b4464b70b3f02af640b53917ebe0904c5bb9f911547c50070257e4`.
- `cycling-guard-after.log`: successful four-suite run, exit 0;
  SHA256 `2bc3a4d6c3c2ce02ef6bd069b371b8b45f66690aaaf4b14f713ad148d5789f58`.

This is a separate review follow-up; the original checkpoint commits and their
proof report remain unchanged.
