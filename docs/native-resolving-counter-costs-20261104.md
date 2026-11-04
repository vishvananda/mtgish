# Resolving costs on selected counter recipients

Thelon's Chant and Tourach's Chant need a counter payment on a creature selected by the player who put the land onto
the battlefield. That player need not control the entering land or the Chant. The trigger continues to read the recorded
`ZoneChange.put_by`; the resolving ability keeps the Chant as its source.

The native prerequisite is local checkpoint `982f76c52ea66a9ab413240cf5310aaf4fee4d7b`, containing the independently
reviewed core feature `2cf00a4b097050294f84e8bd16096f78931c3a71`. `Game::pay_counter_cost(payer, placements)` validates
all declared recipients before payment and uses the existing counter-placement authority with `How::Cost`.
Sheltering Ancient's cumulative upkeep also uses it, choosing all components before paying them separately.

Existing `may_pay` could put counters on its source. Rebinding its source to the chosen creature corrected the quantities
but misidentified the source in payment metadata. Treating placement as an ordinary effect incorrectly lets Doubling
Season double it. Calling the placement blight also uses the wrong keyword and selection purpose.

The shared `counters::may_pay_on_one` recipe asks the original-source May question, filters out unavailable recipients,
asks a non-targeting `Purpose::PutCounters` choice, and pays through the core primitive. The payer, ability source and
recipient remain separate. Replacement to zero still pays the cost under CR 118.11. No mana-ability window is opened.
The caller supplies the printed creature/control population.

The compiler recognizes a resolving optional counter cost with one fixed-count recipient chosen by its payer. It keeps
the entire object filter and counter kind and records both the paid result and recipient. The
`resolving-counter-costs` feature requires the shared native recipe; an older target receives an explicit gap.
Actual blight costs retain their keyword path. Additional casting costs use their existing lowering and are outside this
new resolving-payment API; this checkpoint does not claim to correct that separate selection procedure.

Validation at this checkpoint:

- Native checked workspace: 17,320 passed, 15 existing ignored, 139 test summaries.
- Shared/core controls: 17 passed. Replacing the shared payment with effect placement produces one runtime assertion
  failure and 16 passing controls; restored cost placement passes the full workspace.
- Compiler workspace: 1,418 passed, two existing ignored, 106 test summaries. The new printed-card, missing-feature and
  actual-blight controls all pass. The controller-entry isolation test holds counter-payment support constant.
- Both complete freshly generated Chant modules compile against the native checkpoint with `-D unused_must_use`.
- Whole .NET build: zero warnings/errors; fast tests pass, including 1,216 passed and one existing skipped test.

Detailed receipts are in `~/card-lane/pipeline-unlimited-20261103/integration/counter-combined-proof/` and
`/mnt/vizier/card-lane/generator-resolving-counter-cost-proof-20261104/`.
