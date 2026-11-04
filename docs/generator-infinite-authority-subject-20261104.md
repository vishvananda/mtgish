# Infinite Authority's original creature

The English parser used `HostPermanent` for “the first creature” in Infinite Authority's delayed counter instruction. This queries the Aura's current attachment. The 2006-10-15 Oracle ruling says that moving the enchantment after the ability triggers does not change which creatures are affected.

The source now uses `Trigger_ThatPermanent`, the primary subject of the original block pair. It is the enchanted attacker for “becomes blocked by” and the enchanted blocker for “blocks.” The other creature retains its distinct combat-pair role. Rust lowering exposes the primary subject without changing the existing explicit attacker, blocker or other-creature references.

This corrects the Infinite Authority advice in upstream issue [#81](https://github.com/i5jb/mtgish/issues/81); that report's recommendation of `HostPermanent` was wrong. The other reported cards are separate issues.

The complete 33,529-card parse changes only Infinite Authority, including its RON, JSON and pretty RON representations. All 37 guarded audit findings keep their exact matched-evidence hashes. The Go suite and all 1,028 AST tests pass. A whole-card regression checks that the delayed counter names the original event subject rather than the Aura's later host; the existing combat-pair role tests still pass. No source forms, IR variants or schema stamps change.

The native card remains stopped. Correctly naming both original creatures exposes the separate generator limitation for carrying distinct object groups through nested delayed triggers. The generator now reports that gap rather than emitting the incorrect live-host lookup. The earlier compulsory-effect outcome repair remains valid, but does not solve this captured-reference limitation.
