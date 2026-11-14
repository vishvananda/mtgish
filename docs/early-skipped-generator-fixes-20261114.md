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

Runtime preflight supersedes the source-only Stangg candidate assessment. One
delay must watch the entire multiplied token group and trigger only once. A
separate delay per token is observably wrong when both leave in separate events
and the first sacrifice trigger is countered. The existing member-carrying
source watcher covers Stangg leaving, but cannot match an inverse group departure.
`UntilEvent` also has immediate waiting-effect timing, not Stangg's ordinary
delayed-trigger stack timing. The gap is tracked by Cantrip issue #3028.

The backend now has a target-gated path for the proposed narrow native
`DelayedWhen::AnyOfObjectsLeaves` mode. It matches an immediate, unexpired,
unconditional `LeavesBattlefield` event over `InSet` of a snapshot group. The
waiting body receives no selected-event identity/data; an additional distinct
closed-over set remains a gap. It creates one group watcher, never a per-token
loop. The ordinary source-departure watcher remains a separate `WhenEvent`.

This feature stays **off in the pinned vocabulary** until an actual reviewed
native implementation is adopted. Positive generator tests exercise the future
interface explicitly; they are not a claim that current Cantrip already runs it.
Tests also keep older profiles and Mechtitan Core's additional capture needs
explicitly held. The portable AST and schema do not change.

The two delayed abilities also retain their own printed captions: "Exile that
token when Stangg leaves the battlefield" and "Sacrifice Stangg when that token
leaves the battlefield." Suffix `when` clauses are recognized, and a body-action
word distinguishes the two sentences instead of assigning the original enters
trigger's text to both. The printed-data regression writes the emitted module
for subsequent checks against the native engine packet.

The native compile probe passed after selecting the actual target's capabilities:
`group-departure-delays`, `event-delayed-object-lists` and
`32-bit-effect-quantities`. Its upstream-based `Supertypes` has three fields;
the private foundation also has `world`. The initial synthetic candidate had
inherited the private `world-rule` feature and therefore failed on the different
target. This was a probe-profile mismatch: field emission was already gated.
Regressions now verify both shapes, and the actual-target emitted artifact has
no `world` field. The default private API snapshot retains its World support.

Heroism's source-template correction is documented separately in
[reference-heroism-20261114.md](reference-heroism-20261114.md).
Its source timing correction does not finish automatic generation: per-object
controller payments still need a generic choice-order and batching contract.
That structural program remains an explicit backend gap; the manually written
native card implements the choices directly.

Infernal Denizen's current IR still conflates the chosen opponent's action with
the original controller's `you control` filter. It also represents the mandatory
two-Swamp instruction as an all-or-nothing payment, so the one-Swamp case needs
explicit attention. Its manual Cantrip implementation is checked against Oracle
directly; do not treat that implementation as proof that these adapter issues
are fixed. No source rule or AST contract for those issues was changed in this
generator batch.

The concrete two-part reference report is
[oracle-grammar #24](https://github.com/vishvananda/oracle-grammar/issues/24).
Its evidence uses the existing corpus, the single-record normalized/compact
exports, and the reviewed native packet's partial-sacrifice and multiplayer
scenarios. The opponent must be the chooser and new controller, while the
eligible-creature filter retains the original ability controller. Rebinding all
`AbilityController` reads cannot implement both. Existing explicit actor/bound
references may suffice; no new AST variant is asserted necessary. The Source
record already retains the opponent action and the original `You` filter, so
this actor issue is not a confirmed upstream MTGish grammar error.

Validation logs and source-format comparison receipts are under
`/mnt/vizier/early-set-campaign-plan-20261113/skipped-work-20261114`.
