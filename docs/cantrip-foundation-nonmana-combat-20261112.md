# Fixed nonmana attack costs

Native foundation `9265db850`, with storage follow-up `210d1eefa`, adds the shared attack-payment procedure needed by
Flooded Woodlands, Reclamation and Leviathan. The fixed 286-card audit advances
from 212 to **215 generation-ready candidates**, leaving **71 gapped cards and
88 raw diagnostic labels**. Every other card's gap list is unchanged. These
counts do not mean cards have been installed or independently reviewed.

`CardDef::attack_cost_rules` contains `AttackPayment { applies, cost }`. Each
applicable rule contributes its own fixed cost for each declared attack. The
engine determines those costs after attackers tap and optional attack costs are
announced, before mana abilities. It retains each rule's source and linked text,
so two independent providers do not accidentally read the first provider's
filters. Losing or sacrificing a provider while making mana does not change the
locked total. Costs never force a creature to attack.

Payment uses the existing spell/ability cost scheduler. A land sacrifice and an
Island sacrifice offer a payment-order choice when their candidates overlap and
differ. Repeated equivalent land sacrifices avoid unnecessary order prompts.
Enlist participates when a fixed attack cost is present: a land creature may be
tapped before it is sacrificed. An old identity cannot pay a later tap, and a
creature that left during payment never becomes attacking. Partial payment or
an abandoned declaration restores sacrifices, taps, mana and receipts through
the existing takeback journal. Combat payment receipts identify their imposing
source, attacker and component separately from spell or resolution receipts.

The generator enables this API only through the explicit
`nonmana-combat-costs` candidate feature. The usual API pin stays unchanged.
Unsupported cost generation retains a diagnostic rather than substituting a
quoted string into a live `Cost` field. Announced X in these fixed attack costs
retains an explicit gap; this feature does not invent an attack-X announcement.
Hipparion's separate blocker-payment procedure remains gapped.

Eleven new native regressions pass, as do all **1,490 rules tests** (one existing
ignored test) and the native workspace type-check. Generated runtime scenarios
exercise the actual green/black filters and Leviathan's two-Island selection,
alongside the earlier 62 foundation scenarios: **65 runtime scenarios pass**.
The foundation generator packet, feature-profile tests, complete candidate
module compile and 128-game benchmark are recorded in the accompanying proof.

No per-object field, quantity widening or portable AST/schema change was needed.
Locked costs borrow immutable definitions rather than copying full `Cost` values
for every attacker. An untaxed declaration makes no new payment lists; lookup scans live static
sources and chosen attack pairs. The existing declaration advice reports the
mana portion of an attack cost. A zero-mana entry still means a cost is owed for
requirement validation; nonmana valuation in client/AI advice remains delivery
work rather than a claim of this primitive.

The final shared-host benchmark completes all 128 games and measures 48.70 us
of engine time and 27.31 KB allocated per decision. The preceding combat-group
checkpoint measured 50.26 us and 27.30 KB. These individual runs support no
statistical speed claim. The complete checked-profile native and full generator
workspace suites remain batched until the remaining three shared families are
finished, before the requested pause. The last complete native gate is still
feature 19; its result is not being relabeled as a full feature-21 pass.

Local evidence: `/mnt/vizier/early-set-foundation-20261112/nonmana-combat`.
The [proof receipt](cantrip-foundation-nonmana-combat-20261112.proof.json) records
commands, revisions and log hashes. Native main, Oracle Grammar, paused card
workers, user `consolidation.md` and native `research/` are untouched.

For subsequent batches, build the CLI with
`--features cantrip,serde_json/preserve_order` to keep its dependency feature
union aligned with the test packet. The tests intentionally enable key-order
variation through that dev dependency; switching it off for the CLI rebuilds
the shared serialization crates. The recorded CLI audit used its ordinary
`--features cantrip` build.
