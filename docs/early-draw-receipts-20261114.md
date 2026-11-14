# Original draw receipts for linked follow-up actions

Sindbad and Fa'adiyah Seer draw a card, reveal it, and discard it if it is not a
land. CR 121.6c excludes cards drawn by a replacement from those additional
actions. Taking the difference between the hand before and after the draw
incorrectly includes Alhammarret's Archive's two replacement draws.

The forced Sindbad scenario on the frozen foundation put Lightning Bolt and
Grizzly Bears into the graveyard instead of retaining them in hand. The same
scenario passes with the generated receipt-based callback and the isolated
native engine change. The two ordinary-draw controls verify that a land is
revealed and retained, and that a nonland is revealed before its new graveyard
incarnation is recorded as discarded.

The backend now uses `Game::draw_with_receipt(player, count)` when later
instructions consume the drawn object identities. The `draw-result-receipts`
capability is detected from that method; a target without it reports
`original-draw-card-receipts` rather than publishing the hand-difference code as
supported. The tracked placeholder retains the future interface. An unconsumed
draw and a result used only as an actual draw count continue using the existing
draw and history interfaces. A draw cost's requested count and payment decision
remain separate from its original-card receipt.

Independent Terra review found two further backend corrections for these two
printed consumers. Draw-and-reveal must finish before the instruction boundary;
the conditional discard must use `discard_revealed`, preserving the revelation
through a replacement into a hidden zone (CR 701.20a). The backend now recognizes
the complete draw/reveal/land-test/discard data flow with the same result and
incarnation throughout. It does not mark an unrelated discard as revealed merely
because an earlier action revealed something.

The isolated native implementation is committed at
[`5b73d5c62`](https://github.com/vishvananda/cantrip-rs/commit/5b73d5c6276b30e39b0afd84d480525d31be9aba),
on `packet/early-set-draw-followups-20261114`, directly above frozen foundation
`508215f0a6c2c2228b40332b590b4a5bfde92cb4`. It collects original draw results only
on the receipt path. An actual instruction replacement also excludes its whole
replacement instruction, even when the resulting count is unchanged; declining
an optional replacement preserves the original receipt. No persistent field or
numeric domain changes are introduced.

Validation so far:

- Full generator suite: 1,658 passed, 21 ignored, across 221 targets.
- Four focused generator tests cover both printed consumers, explicit missing
  native support, ordinary draws, count-only results and an unrelated discard.
- Eight native draw-receipt tests pass, covering exact identities/order,
  replacement draws, a later unreplaced draw, nondraw replacements, unchanged
  replacement counts, optional decline, unrelated replacements, limits and empty
  libraries.
- Three native Sindbad controls pass with the actual generated callback and
  forced scenarios, including reveal/discard event identity and ordering.
- The committed engine alone passes the full native and managed suite: 23,970
  Rust tests, 15 ignored, across 250 targets, and 1,526 managed tests.
- Three interleaved normal-runtime benchmark pairs preserve 29.34 KB allocated
  per decision in all six runs. Median engine time is 84.58 microseconds before
  and 78.68 after; shared-host variation does not establish a speedup.
- Terra independently reviewed the engine and card, fixed the two card-level
  issues above, and added casting-color, source-removal and revealed-discard
  replacement controls. Its five scenarios, four integration tests and all 11
  instrumented card lines pass. Replacing `discard_revealed` with ordinary
  discard is caught by the new interaction test.

The final packet is published as
[Cantrip #3003](https://github.com/cantrip-rs/cantrip-rs/pull/3003), with engine
commit `5b73d5c62` followed by card commit `09faa5bb9`. The exact independently
reviewed files pass the deterministic corpus, Oracle, lint, citation, coverage
and mutation gates, then the full checked Rust workspace suite: 23,979 passed,
15 ignored, across 251 targets. Both commits are pushed and the ready PR depends
on the frozen foundation, not another card batch.

The production lane's rows and default worker base are unchanged; do not roll
this interface into the locked shared foundation. No AST schema, Oracle source
or face bundle change is required for this backend correction.
