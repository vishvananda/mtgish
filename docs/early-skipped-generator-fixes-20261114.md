# Generator paths for skipped early-set cards

The 65-card audit is saved at
`/mnt/vizier/early-set-campaign-plan-20261113/skipped-card-audit-20261114/report.md`.
The follow-up uses small card batches on current Cantrip upstream, with one warm
workspace and independent Terra review. It does not restart the retired pipeline
or rewrite the published foundation/221-card branches.

## Producible colors

`ManaProduction::CouldProduce { colors_only: true }` can use the existing
`Query::producible_types` capability query. The generator now unions its color
bits, preserving the distinction between colors and colorless mana, rather than
emitting the nonexistent `mana_could_produce` interface and an unconditional gap.

When the native target contains `mana::opponents_land_colors`, the exact
opponent-land query uses that shared recipe. Cantrip batch 1 extracts it from
Exotic Orchard and uses it for Fellwar Stone. Different controller filters keep
their own queries; a type query such as Reflecting Pool remains a gap because a
`colors_from` callback cannot express colorless production.

Both capabilities are detected from the target. The existing private reference
`8de2bccb5f04270e217ecd8582843c59d327abde` already contains `producible_types`, so
its vocabulary gains that feature without changing its API fingerprint. Its
other pending capabilities and counter names are preserved. A snapshot of
current upstream is **not** substituted for that complete private reference:
doing so would remove interfaces still used by pending packets.

## Variable sacrifice activation costs

Current Cantrip supports `SacrificeX` and `SacrificeOneOrMore` on ordinary
activated abilities. The generator recognizes that documented native contract
as `variable-sacrifice-activations`. It emits those existing cost variants for
explicit X and one-or-more costs; an any-number activation with no independent
mana X uses the activation's native sacrifice count.

The counterexample matters: `{X}, Sacrifice any number ...` makes two independent
choices. It must not silently tie the number sacrificed to the mana X. That
shape remains a gap, as do older target profiles lacking the activation
contract. Resolving payments are not changed by this activation support.

Tests cover Sword of the Ages, Krav, the Unredeemed, Radiant Lotus, the
independent-X counterexample and the old-profile boundary. Native card scenarios
remain necessary before calling any particular card supported.

## Other findings

Heroism's source-template correction is documented separately in
[reference-heroism-20261114.md](reference-heroism-20261114.md).

Infernal Denizen's current IR still conflates the chosen opponent's action with
the original controller's `you control` filter. It also represents the mandatory
two-Swamp instruction as an all-or-nothing payment, so the one-Swamp case needs
explicit attention. Its manual Cantrip implementation is checked against Oracle
directly; do not treat that implementation as proof that these adapter issues
are fixed. No source rule or AST contract for those issues was changed in this
generator batch.

Validation logs and source-format comparison receipts are under
`/mnt/vizier/early-set-campaign-plan-20261113/skipped-work-20261114`.
