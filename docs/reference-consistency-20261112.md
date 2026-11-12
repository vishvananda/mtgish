# Reopened reference consistency review

The previous 53-face review answered whether a proposed replacement was required
for game behavior. That was not sufficient to close the 36 retained
equivalent/contract cases. Matching Oracle wording should have a common producer
form unless a demonstrated semantic context requires a distinction.

All 36 are reopened in the [per-face ledger](reference-decisions-20261112.json).
Satya is separate: token multiplication disproves its proposed scalar receipt.
The previous decisions and evidence remain historical; they are not waivers for
alternate syntax. The user provided fresh upstream-parser comparisons confirming
that several differences depend on template parameters rather than Oracle words.

The [upstream consistency issue](https://github.com/i5jb/mtgish/issues/275)
currently records seven concrete Source families: opponent-sacrifice filters,
artifact-animation referents, self enters-or-attacks references, Graven Dominator's
implicit self exclusion, target pronouns after control changes, countered-spell
mana-value references, and counters on a sacrificed source. Additional confirmed
Source families will be added there. Rust-only producer differences are fixed
locally rather than misreported as upstream Source defects.

## Policy

- Select one generic template or lowering contract for equivalent wording.
- Preserve actual actor, object incarnation, last-known information, cost guards,
  event frames and replacement multiplicity. Different meanings require explicit
  operands, rather than unexplained alternative operations.
- Fix actual production. Do not add scoring aliases or edit independent gold.
- Compare the affected family against matching controls, including a counterexample
  to any overbroad rewrite. Run broad gates in a batch.
- Keep runtime AST representation compact: prefer resolved bindings and existing
  variants over additional wrappers, fields or allocation-bearing nodes.

## First confirmed template-context differences

| Family | Current producer difference | Common-contract investigation |
| --- | --- | --- |
| Bloodroot Apothecary | Only the opponent-sacrifice template whose body consumes `ThatPlayer` invents `Other(ThisPermanent)`; player-plus-permanent and unbound variants do not. | The printed event filter must be independent of the body's bindings. Remove the unprinted exclusion. |
| Graven Dominator | Its phrase template expands “each other creature” to `Other(Trigger_ThatPermanent)`. | Make the intended self exclusion explicit, and test entry and haunted-creature-death branches separately. |
| Korvold / Sephiroth / Hei Bai | A parameterized self enters-or-attacks body excludes the event subject; another body path excludes the source. | Choose a common self-reference contract; a compound event has the same guaranteed source binding only when every branch proves it. |
| Chittering Host comparison control | Existing canonicalization folds Source/EventSubject for a single self event but does not prove the same binding across compound events. | Audit actual producer output and the comparison separately. Do not repair only the score. |
| Karn's Touch / Karn / Sydri / Toymaker | The identical animation template reads mana value from Affected in one grammar section and Target in another. | Use one information-timing/referent contract; standalone spell versus activated ability is not a reason for a different referent. |
| In Thrall to the Pit / Act of Treason | The same control/untap/haste sentences use a recorded control receipt versus the original target. | Preserve the correct attempted-control identity without making an unrelated kicker paragraph change preceding pronouns. |
| Draining Whelk / Spell Swindle / Plasm Capture | A special counter-receipt quantity versus the targeted spell's mana value. | Both must preserve information when countering fails. Select one clear contract. |
| Vulshok Factory / self-sacrifice counter controls | An explicit sacrifice receipt versus Source with last-known information. | Select one self-reference contract while retaining the exact original incarnation and time of the count. |

The rest of the 36 still require family-level producer review. In particular,
optional-action/payment forms, reflexive and delayed event references, random
selection, copy information, recipient-bound quantities, redundant predicates,
blocking requirements and control-flow ordering are not closed merely because
the previous review found no differing outcome.

## Foundation work retained

The foundation is pushed through native `4494834c7` and generator `1fa344b`:
202 ready candidates, 84 still gapped in the frozen 286-card scope. The complete
native gate covers the preceding banding checkpoint; the combat-watcher step has
focused rules/runtime, workspace and benchmark validation. Computed-X work is
uncommitted in the isolated native foundation tree; its first ten native tests
pass, but no generator adapter or completed-family claim is made yet.

After this higher-priority reference consistency work, finish the shared five-card
and three-card foundation families and then pause as requested. Independent
two-card and single-card features remain deferred for focused engine-plus-card
PRs. Card workers stay paused, upstream is not pulled, and user-owned work is
preserved.
