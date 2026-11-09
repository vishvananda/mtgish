# Explicit participants in collective coin-face checks

Mana Clash's Source already says `AllCoinsCameUpHeads`. The Rust lowerer
previously erased that meaning into `CoinFlipsWon(false) == 0`. It now emits
`All(CoinCameUp(controller, Heads), CoinCameUp(target, Heads))`, using the
participants of the preceding single-flip-per-player batch. Repeating the
process still negates that condition. The coin instructions, private receipts,
player order, damage and repeat boundary are unchanged.

The lowerer records this proof only for a loop that flips exactly once for
its bound player and whose players have stable explicit identities. Subsequent
coin producers and unknown actions invalidate it; an optional producer cannot
leave an earlier batch available. Independent ability scopes do not inherit it.
A collective check without a proven batch fails lowering. Primitive/term
escapes cannot hide the missing participant context from native or Wasm
validation. The lowering proof is not serialized into semantic IR.

Four focused tests cover the full authentic schema-184 card, named-document
roundtrip, all heads/tails combinations including an unrelated third player,
stale/absent/optional batches and malformed authoring inputs. The complete
33,529-record comparison changes exactly Mana Clash's collective condition
subtree. Schema 184's wire descriptor is byte-identical to the published
baseline. The full native IR/AST/Wasm suite passes 1,467 tests with zero failures
(15 opt-in audits ignored).

All fourteen browser suites pass against the final artifact, including all
33,529 block roundtrips, typed binding-port comparisons and Wasm validations.
The code suite covers seventeen card examples and ten malformed inputs.
With actual Oracle text, the generator produces a complete Mana Clash face
module with no gaps. Apart from fingerprint comments, its only change is the
condition above, expressed through the already-recorded individual player
flips. No backend feature or numeric-range change is required.

Evidence is in `/mnt/vizier/ast-review-20261109/mana-clash/`, particularly
`native-final.log`, `comparison-final.json`, `browser-results.json`,
`generator-before-after.json` and `generator-final.diff`. Earlier count-based
prototype captures are explicitly retained under `count-prototype-*` names;
these are not the final release evidence. The final corrected schema-184 coin
cohort is separately captured for the pending structural coin-policy migration:
76 complete records, 78 faces, explicit authoring names and printed-module
outputs/diagnostics.

This is a Rust normalization defect, not a reference grammar defect: the
Source representation is already correct, so it needs no upstream grammar
report. It is separate from the 29 confirmed reference defects queued after
completion of the broader AST/editor work. Typed coin protocols/repetition,
destination-specific serialization, consistent Rust roles/counter sources and
typed layouts remain open. No upstream changes were pulled.
