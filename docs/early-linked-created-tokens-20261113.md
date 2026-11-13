# Created-token references across abilities

The Tetravus retry on the repaired counter-payment engine exposed an independent
lowering error: `TheCreatedTokens` outside a current creation receipt used only
`MadeBy(Source)`, admitting nontokens put onto the battlefield by the source.
The forced scenario grants Tetravus Starfleet Communicator's attack ability,
puts an Island onto the battlefield, then checks that the reclaim choice refuses
that Island while accepting the Tetravite. The counter-payment proof remains valid;
it did not cover this distinct choice-filter defect.

Both fallback paths now intersect `Token` with `MadeBy(Source)`: an object set
and a filter inside an optional cost must agree. Generic put-onto-battlefield
references retain their broader `MadeBy` meaning. Current-resolution creation
references still use `Created(ResultId)` and branch scope checks are unchanged.

This is a necessary partial correction. Tetravus's abilities are linked under
CR 607.2c and separately acquired pairs remain distinct under CR 607.5. A token
created by an unrelated ability granted to the same object must also be excluded.
The canonical `MadeBy(ObjectRef)` contract cannot express that distinction, and
our native event scan currently knows only the creating source/AbilityRef rather
than captured acquisition. Do not retry or approve Tetravus based only on adding
`Token`, or globally change generic `MadeBy` to mean linked creation.

[Oracle Grammar #20](https://github.com/vishvananda/oracle-grammar/issues/20)
asks the AST owner to select the explicit linked-creation relation. No new MTGish
parser defect is asserted: the Source `TheCreatedTokens` can acquire its meaning
from the card's linked abilities. The same-resolution receipt and generic
put-onto-battlefield relation should stay distinct. A matching native solution
must support multiple tokens over multiple upkeeps and separate copy/grant
acquisitions, rather than a single remembered token or a card-name special case.

The retained runtime harness and both forced scenarios are in
`rust_syntax/normalize/tests/fixtures/early_counter_payment_native/`. The
nontoken counterexample is executed evidence; the granted-token/copy-acquisition
cases in the issue are additional rules-derived requirements, not claimed test
passes. Production queue rows remain pinned to the eleventh rollout until the
next explicitly audited refresh.

Validation: 1,651 generator tests pass (20 ignored, 219 targets). With the same
fixed counter-payment engine, the old token filter passes the ordinary payment
scenario and fails the Island refusal; the new filter passes both. The initial
scratch probe was on the old payment code as well, so that patch was restored
before making the actual token-filter comparison. These passes do not cover the
separate linked-acquisition requirement and do not approve the card.
