# Reopened reference consistency review

**Completed:** all 36 reopened faces now have producer-family resolutions, with
zero pending. The [last 30 cases](reference-remaining30-20261112.md) include the
final full workspace/parser gates and corpus-wide IR validation. The earlier
counts and receipts below describe their historical checkpoints.

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
initially recorded seven concrete Source families: opponent-sacrifice filters,
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

## First completed Source batch

The four template families above now have actual parser fixes. A fresh parser run
before and after the edits parsed the same 33,570 records and refused the same
1,477 inputs. Exactly 30 records changed: Bloodroot Apothecary, Graven Dominator,
Karn's Touch, and 27 self enters-or-attacks records. Every one of the latter
changes is exactly `Trigger_ThatPermanent` to `ThisPermanent` in the body supplied
by that template. All 30 prior fresh records matched the cached Source records;
no unrelated parser drift was adopted. Each of the other 33,499 cached records is
byte-identical in both line corpora. Pretty RON and tagged JSON carry the same
reviewed changes.

The four original packet faces completed here are **Bloodroot Apothecary,
Graven Dominator, Karn's Touch and Korvold, Fae-Cursed King**. The ledger keeps the
old gameplay-only decisions as history and records these new consistency fixes
separately. At that checkpoint, 32 of the reopened 36 remained pending. Sephiroth and Hei Bai,
Spirit of Balance are matching controls, not extra faces in the original count.

The comparison's existing single-self-event proof now also handles an alternative
event only if **every branch independently identifies Source**. This accompanies
the parser/body-binding fix; it does not substitute for changing production.
A mixed self-entry/haunted-creature-death event does not pass the proof. The
independent historical 58-card proposal fixture remains unedited, as do Oracle
Grammar's gold, worktree and reference pin. Only our previously reviewed producer
snapshots change for the four selected template decisions.

Regression coverage checks raw, uncanonicalized IR: the printed sacrifice filter,
the self exclusion on Graven's mixed event, source bindings for the three matching
self enters-or-attacks cards, and the target mana-value expression with Snapshot
evaluation on all four matching animations. The complete 40-record parser fixture
also verifies cached tagged JSON against Source RON, including transforming cards.
The existing returning-permanent regression remains a counterexample to blindly
folding references across nested event scopes.

### Wider binding rewrite deliberately excluded

An attempted generic lowering-context Source proof exposed a returning-permanent
scope regression in the broad gate. It also changed many historical producer
forms outside this batch. That rewrite was removed before publication. In
particular, Waterspout Warden and Storm Entity were left pending by this first batch; it does
not claim their raw production is unified. Entry's prospective controller and cast
history retain their explicit incoming-event references. No AST layout, schema,
serialized field, runtime object storage or Cantrip interface changes are included.

The failed intermediate gate logs are retained as investigation evidence, not
validation receipts for the published batch. Final validation is recorded below
only after the final code passes.

### Remaining family worklist

| Faces | Next producer investigation |
| --- | --- |
| Nemesis Mask | Confirmed Source split: it emits `EachPermanentRuleEffect(IsCardtype(Creature), [MustBlockAttacker(HostPermanent)])`; Lure emits `PermanentRuleEffect(HostPermanent, [AllCreaturesMustBlockIt(IsCardtype(Creature))])` for the same all-able-blockers clause. Prefer one shared requirement on the host. |
| Ebonblade Reaper, Personal Incarnation, Skybreen | Resolve recipient-relative `Acting` through the operation's named player; retain per-player binding for plural recipients. |
| Cryptic Coat | Unify the statically singular cloaked-permanent receipt with matching manifest/cloak result references. |
| Deathbringer Regent, Dream Thief | Remove redundant object-match wrappers only after resolving their `Candidate` bindings and conditions. |
| Delif's Cube, Long River Lurker | Share the captured-object/future-event-subject contract without losing independent event frames or post-exile incarnations. |
| Draining Whelk | Choose one targeted-spell information contract that also handles an unsuccessful counter. |
| Dawn of the Dead, Ilharg, Mirror Mockery, Wandering Graybeard | Unify optional actions and payment guards without dropping dependent follow-ups or CR 118.12 choice information. Source already shows `MayActions` versus `MayCost`/`If(CostWasPaid)` on matching optional-action families. |
| Electro, Passenger Ferry, Resplendent Marshal | Make reflexive self references consistent while retaining the creator's frame; payment does not itself identify a new object subject. |
| Forbidden Crypt, Worzel | Remove redundant zone/domain qualifications through generic selector rules, preserving meaningful type restrictions. |
| Haunted Fengraf | Choose one random-selection representation with the same empty-pool and uniform-resolution-time population semantics. |
| Hit | Common mandatory-sacrifice contract; retain original input information and explicit follow-up behavior when no sacrifice occurs. |
| In Thrall to the Pit | Use one target pronoun contract across control, untap and haste; delayed sacrifice still needs its captured identity. |
| Liliana, Death Mage | Unify scalar versus up-to-one target sets and the named player's binding. |
| Molly Hayes | Use a clear one-result versus full-result-set contract for the impulse permission. |
| Myriad Construct, Vulshok Factory | Normalize self-sacrifice information without losing the original incarnation or the before-payment view. |
| No Quarter | Use consistent explicit attacker/blocker roles in the mirrored pair events. |
| Norman Osborn | Make the conniving object's actor explicit; do not justify an inconsistent field because one backend ignores it. |
| Skeletal Swarming | Preserve text order and rules-defined layers; do not sort arbitrary changes within a layer. |
| Veteran Ghoulcaller | Unify duplicate information/copy forms only with an explicit last-known/post-move view; also audit graveyard ownership versus actual graveyard membership. |

These are consistency work items, not 30 established gameplay defects. The active
Source evidence for these investigations is retained locally; examples that exist
only because of our Rust lowering should be fixed here rather than assigned to
upstream parsing.

### Second completed Source batch

[Waterspout Warden and Storm Entity](reference-self-frames-20261112.md) now have
narrow producer fixes for their proven self referents. Self-attack intervening
conditions use ThisPermanent; own entry spell-count exclusions use ThisSpell.
Other entering-creature contexts keep their distinct subject. Exactly 18 cached
records change, with no parser success/refusal changes. No generic binding rewrite
or comparison-only alias is involved. The current reopened review is **6 completed
and 30 pending**; the earlier receipts describe their own historical checkpoints.

## Final validation

- `RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=8 cargo test --manifest-path rust_syntax/Cargo.toml --workspace --features cantrip --no-fail-fast`: **2,014 passed, 24 ignored, 196 targets, exit 0**.
- Full uncached `go test ./... -count=1`: **PASS**, 169.060 seconds with the Rust suite running concurrently.
- All 30 reviewed line-record changes are confined to the intended Source edits; the other 33,499 records are byte-identical in both line corpora.
- The 58-card independent proposal fixture, including Spider-Man's older event-subject form, remains untouched and its comparison test passes.
- No portable AST/schema, native engine, worker, browser workspace or site changes.

The [validation receipt](reference-consistency-20261112.proof.json) pins final
inputs and successful logs. The documented 16 MB debug-test stack is required;
runs without it exposed the existing parser/migration stack requirement and are
not the final validation run.

The [upstream implementation follow-up](https://github.com/i5jb/mtgish/issues/275#issuecomment-6818543002) links the pushed fixes and also flags the confirmed Nemesis Mask/Lure requirement-direction and optional-action control-form splits.
