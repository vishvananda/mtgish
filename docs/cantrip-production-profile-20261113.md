# Early-set production generator profile

The production Cantrip backend is paired with the rebased shared foundation
`11237af09f9f3ae9384796017c8a4135d8c2ec72`, based on upstream
`27fffc942f71e8c64854bb68da096a67b9498578`. The native checkout used to refresh
the snapshot is `/mnt/vizier/card-lane/early-set-foundation-rebase-20261113`.
Its API fingerprint is
`c65f91ea05ae924de5a2c8d2a935154f038cab8673bea0a39200c3b2d337e4cf`.

This enables the 25 implemented foundation capabilities in ordinary
`ast cantrip-files` output. The lane does not substitute candidate output for
production readiness. The native quantity profile remains
`32-bit-effect-quantities`. Upstream's Edosian subtype and Vow counter are
included. Shared Cube, Doom, Glyph, Pin and Sleight counter prerequisites are
also included; existing counter IDs are retained.

Historical foundation tests freeze the original 45-capability production
profile and add each feature group explicitly. Their negative controls therefore
continue to prove that missing interfaces are reported, even though the current
production profile supports them. Current-profile tests additionally require
the real upkeep deadlines and Winter's Chill X restriction. Backend capability
changes do not alter the AST ownership or schema contract.

Validation on this pairing:

- `cargo test --locked --manifest-path rust_syntax/Cargo.toml -p mtgish-ast
  --all-targets --features cantrip --no-fail-fast`: 1,613 passed, 20 ignored,
  across 199 targets.
- The explicitly enabled `foundation_generated_cards_execute_in_cantrip`
  probe compiles the generated definitions and passes all 77 native runtime
  scenarios against the real foundation, including its actual Pin/Sleight
  declarations rather than a separate counter overlay.
- `update_cantrip_vocabulary.py <native-checkout> --check` verifies the pinned
  source fingerprint; the runtime checks, rather than the fingerprint alone,
  provide behavioral evidence.

The Oracle inputs remain the frozen MTGish `20261001.1` corpus. No new Scryfall
snapshot is used for this campaign. The frozen early-set population has 1,320
eligible names across 14 sets, excluding ante, dexterity and Shahrazad while
retaining flavor-banned cards. A fresh intake plan must compare production rows
with the exact native base and live reservations before claiming missing cards.
Generator gaps remain unclaimed. Final native workspace and publication gates
are separate from the generator validation above.
