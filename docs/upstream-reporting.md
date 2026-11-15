# Reporting MTGish Source issues

MTGish owns its English → Source parser, Source syntax and supporting input data.
This fork owns Source → Oracle IR lowering and comparison. Oracle Grammar owns
the published IR model and its independent references. Cantrip owns the backend
generator and runtime; Oracle Blocks owns the editor.

## Establish the producer defect first

1. Preserve the Oracle wording and run an unmodified upstream parser at an
   identified commit. Find the producing template, including its implicit
   parameters. A difference from our fork is a lead, not proof of a bug.
2. Read the entire Source program around the disputed expression. It may already
   encode the player, candidate, zone, event identity, phase ordering, chosen
   value or historical information that our lowerer failed to preserve.
3. Establish the Source constructor's contract before inferring behavior from
   its name. Maintainer explanations take precedence over our consumer's
   implementation. Examples already clarified upstream include:
   - `ItsController`: the current candidate's controller.
   - `APlayerMayAction`: the ability controller chooses the player, who then
     chooses whether to act.
   - `PayManaAnyX`: choosing X is part of the payment operation.
   - `AdjustPTForEach`: a nonpositive repeat count performs zero adjustments.
   - `FindNumberCardsOfType`: matching library searches permit failure to find.
   - `ManaCostsOfCombatCreatures`: a combat-history query.
   - `CantBeBlockedUnlessCost`: the cost applies per declared blocker.
4. Demonstrate the discrepancy between the printed instruction and that
   established contract. A failing generated game test, changed gold file or
   agreement between two programs written to our IR is not independent evidence
   of an upstream Source defect.

When Source already carries the needed meaning, fix the lowerer or backend.
Do not request redundant producer wrappers solely to compensate for our scope,
receipt, identity or variable-capture mistakes. If the Source contract is still
unclear, label the report a contract question or explicit-representation
proposal, and state that no gameplay defect has been established.

## Make the requested patch reviewable

Every actionable report should contain:

- The affected card and relevant Oracle clause, plus an unchanged control when
  a shared template has callers with different semantics.
- The upstream commit and immutable link to the producing grammar, script or
  data record.
- The current Source expansion and the exact requested operand, grouping,
  capture, timing or syntax change. Preserve unaffected captures and scopes.
- An acceptance example that distinguishes the old and intended contract.
  Distinguish a synthetic generic-template counterexample from a failure of a
  real printed card.
- Any corresponding Source syntax/type registrations needed by a new Source
  member. Do not ask upstream MTGish to change owned Oracle IR types or Cantrip
  engine APIs.

Equivalent lowerings can justify a consistency proposal when the intended
canonical form is stated. They do not automatically establish a rules bug.
Keep normalization and Source-classification requests visibly separate from
incorrect-recipient, missing-instruction or similar parser corrections.

## Maintain the report after feedback

Update the issue's title and opening body when narrowing or withdrawing a
claim. A correction buried in a later comment leaves the original request
misleading. Correct our follow-up comments too when they repeat the withdrawn
claim; preserve the prior text as historical material.

For mixed reports, name the withdrawn cards and the remaining requested edits.
Close reports already fixed upstream as completed. Close downstream-only or
unproven bug claims without asking the upstream maintainer to implement our
consumer fix. Consolidate duplicate normalization proposals with a link to the
surviving request.

The [2026-11-15 audit](mtgish-report-actionability-20261115.md) supersedes older
local reports where their upstream disposition or Source-contract interpretation
differs. Historical validation counts describe our implementation at the time;
they do not establish a still-open upstream defect.
