# Early-set production generator profile

The production Cantrip backend is paired with the rebased shared foundation
`6b080fb69a8055815d61d192250855500f68f64e`, based on upstream
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

The native Rust implementation is unchanged from `11237af09`; the final commit
corrects existing client prompts and LLM band selection. The campaign uses the
validated `6b080fb69` snapshot for new work. Its complete `tools/test.sh` run
passes 23,665 Rust tests and 1,525 managed tests (15 Rust ignores and one managed
skip). Five added client regressions fail against the old client implementation.
No numeric types, prompt kinds or rendering capabilities changed.

Initial production-profile validation at `11237af09`:

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

## Repairs discovered by the campaign

The current generated rows use `8390186911cea152032d1f15a8f8455fbbc5f1ad`, with the
same native base and API fingerprint. The original full-package receipt above
records the initial profile adoption. Subsequent targeted checks cover:

- Four reversed damage-prevention grammar templates, changing exactly nine
  Source records without importing new Oracle data. The unchanged Balduvian
  Hydra scenario fails before the correction and passes afterward. The entire
  33,529-record parser audit has 182 reviewed findings and none unresolved.
- Correct citations for the actual banding, activation-player, hand-size and
  untap-limit callback roles. Production-render checks cover six predicates
  plus an ordinary resolving-effect control.
- Recognition of the existing full-range u32 conversion by the style gate.
  Full-range i32 conversions are also recognized. Arbitrary rule caps and
  mismatched casts remain rejected; the 169 card-lane tests pass.
- Call to Arms now requires a uniquely most common color, including its tie
  branch, and Cleansing identifies the individual land when offering payment.
  Native before/after witnesses prove both corrections.
- Optional mill costs require the full payment to be possible. Deep Spawn's
  one-card library refuses mill-two, while its two-card control can pay.
- Choosing colors accepts a nonempty subset instead of requiring all five.
  Dream Coat's single, multiple and all-color native controls pass; zero and
  duplicate colors are refused. A Mind Bend control preserves the actual colors
  in the choice rather than changing its labels with the card's text.
- Draw-and-discard choices are restricted to the cards actually drawn and
  still in hand. Krovikan Sorcerer's original witness fails before and passes
  after; generator controls also cover Casting of Bones and Soldevi Sage.
  Native draw history excludes replaced/prevented draws; a replacement-specific
  runtime witness has not yet been run.

The second rollout preserved the previous rows by generator revision, waited
for all implementation processes to finish, and retried those five stopped
cards. Existing reviews continue against their immutable candidates. Blood of
the Martyr now reports the missing optional granted-replacement interface
instead of silently generating a mandatory replacement; its scope decision is
pending. No engine quantity types were widened.

The lane also preserves incomplete author work for independent review through
an explicitly unapproved WIP commit. The reviewer must finish the full gates,
and a separate verifier must confirm the final card before collection. The
Ashen Ghoul pilot completed that entire path. Its historical implementation
outcome is preserved without counting the approved card as unfinished.

The separate named-turn model proposal in Oracle Grammar PR #3 is not yet a
published dependency of this generator. Arboria's actor/controller correction
awaits that registry release and consumer adoption.
