# Bands with other foundation

The sixteenth foundation feature adds the parameterized form of banding
(CR 702.22). It clears the recorded gaps for Adventurers' Guildhouse,
Cathedral of Serra, Master of the Hunt, Mountain Stronghold, Seafarer's Quay,
Shelkin Brownie, Tolaria and Unholy Citadel. The frozen 286-card scope now has
201 generation-ready candidates and 85 still gapped, with 95 raw diagnostic
labels. No cards have been installed or submitted by this step.

`AbilityDef::BandsWithOthers` carries a quality predicate in an existing ability
slot. `cantrip_mechanics::banding::with_others` constructs it. The five lands'
color restrictions select which legendary creatures receive the ability; the
band's quality is legendary creatures of any color. Master of the Hunt names
creatures called Wolves of the Hunt, rather than every Wolf.

Formation requires a qualifying provider and matching members. A partial
selection may add a provider later. Damage assignment instead requires a
qualifying provider and one matching peer, and permits unrelated creatures in
the combat group. Existing band membership survives ability loss. Removing
ordinary banding also removes every parameterized instance, including from a
creature that had only the latter (CR 702.22b). Shelkin Brownie's narrower loss
keeps ordinary banding, and later grants work in timestamp order.

The generator's `bands-with-other` candidate flag selects the actual engine
interface. Context-dependent qualities remain explicit gaps. The normal
upstream API pin remains unchanged; this is an unmerged foundation capability,
not a claim that the pinned upstream engine supports it. AST schema 190 and all
game quantity bounds remain unchanged. No per-object fields were added.

Nine native regressions cover formation, partial selections, assignment,
grants/loss, copying, declared band persistence, shared blocking and actual
combat damage on both sides. Three new generated runtime scenarios exercise
all eight cards through real declarations, token creation and activated
abilities. The complete generated runtime probe passes 51 scenarios, and all
201 ready modules type-check against the native foundation without an overlay.

The 128-game before/after benchmark completed every game. Allocation remained
27.04 KB per decision. Engine time measured 44.35 versus 45.61 microseconds per
decision on this shared machine; a single pair does not establish a performance
change. No .NET pass is claimed because the SDK is unavailable.

Evidence is under `/mnt/vizier/early-set-foundation-20261112`: `bands-tests-resumed.log`,
`bands-workspace-check.log`, `generator-bands-native-runtime.log`,
`generator-bands-ready-compile.log`, `generator-bands-scope.log` and
`benchmark-bands-{control,after}.log`. The progress ledger records the exact
native revision, API fingerprint and batched full-suite receipts.

The next foundation work includes computed X constraints and persistent public
visibility. Delayed event abilities limited to a combat also need an engine
lifetime: the existing engine supports turn-limited event watchers but not
combat-limited ones. This is separate from an ability that triggers at the
beginning of the end-of-combat step.
