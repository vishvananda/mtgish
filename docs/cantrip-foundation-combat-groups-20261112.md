# Complete combat declarations

The twentieth focused foundation commit is native `a09922634`. The generator's
`combat-group-constraints` candidate profile emits `CardDef::combat_groups`.
Caverns of Despair, Errantry and Orcish Conscripts become generation-ready:
**212 ready / 74 gapped / 89 raw requirement labels** in the frozen 286-card
scope. Every other card's gap list is unchanged. These remain candidates,
not installed or fully reviewed cards.

The restriction counts distinct creatures in the complete declaration, including
blockers that can block several attackers. Membership is fixed while declaring;
source, controller and linked text are preserved. Global caps share membership.
The solver maximizes requirements under these restrictions and asks whether each
proposed addition has a legal completion. Ordinary attack declarations retain
the assignment solver; live group restrictions use a bounded exact search.
Blocking extends its existing exact solver to include otherwise unrelated
companions. Paying a cost never becomes an attack or block requirement.

Independent exhaustive small-position checks cover all combinations of the three
printed restrictions with requirement weights, forbidden attacks, per-target
caps and costs, plus menace and multiple blockers. A 128-creature capped search
stays below 1,000 nodes. Three native source/context tests pass. All **1,479
rules unit tests** and **84 state unit tests** pass (one rules test ignored).
All **62 generated runtime scenarios** pass, including actual casts and combat
answers for the three cards. All **212 ready modules**, plus Reflecting Mirror's
partial callback control, type-check. The foundation generator suite passes
22 tests (one ignored). Names-only Pin/Sleight prerequisites remain separate.

The final 128-game benchmark completes every game at 50.26 µs per engine
decision, versus 48.06 µs on the preceding checkpoint. Allocation is 27.30 KB
per decision versus 27.04 KB; declaration questions now carry their group data.
These shared-host single runs do not establish a statistical timing change.
The complete workspace gates will be batched after the four remaining shared
families. No .NET pass is claimed. The upstream API pin, AST schema 190,
quantity bounds and per-object fields are unchanged.

The [receipt](cantrip-foundation-combat-groups-20261112.proof.json) records
hashes and the exact scope. Four shared three-card families remain in the
[stopping milestone](cantrip-foundation-shared-stop-20261112.md).
