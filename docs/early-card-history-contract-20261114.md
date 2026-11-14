# Historical death selectors: Krovikan Vampire

Tracked with the canonical AST owner in
[Oracle Grammar #21](https://github.com/vishvananda/oracle-grammar/issues/21).
This is a confirmed defect in our Source-to-IR lowering, not evidence of an
upstream MTGish parser or reference defect. The frozen Source face is unchanged.

`DiedThisTurn(predicate)` currently becomes a current-graveyard filter combining
`predicate` with a death-history test. That makes the damage predicate compare a
new graveyard object with damage to its departed battlefield incarnation. The
retained forced scenario has the Vampire kill Grizzly Bears and survive, but the
end-step ability fails to return the Bears. It also loses the view needed for a
noncreature card that died while animated; that second case is an IR analysis,
not yet an executed native scenario.

The representation needs to preserve the historical death predicate as a unit,
then project the first resulting graveyard incarnation while it remains there.
Leaving and returning cannot restore the link. Matching deaths are queried again
at resolution, including a qualifying death in response to the end-step trigger.

`FromHistory` returns historical subjects, while `EventDestination` refers to an
enclosing matched event. The enclosing trigger here is an end step, so those
forms cannot simply be substituted for one another. The issue asks the AST owner
for an existing representation or an explicit projection/view contract. Do not
globally redefine `FromHistory`, patch identity only in the backend, change the
Oracle snapshot, or edit the other worker's dirty Oracle Grammar checkout.

The card remains outside the 201 published cards. Once the contract is settled,
add ordinary-creature, animated-noncreature, intervening-zone-change and
death-in-response controls, then publish any required engine change and its card
in a focused packet above the frozen foundation.
