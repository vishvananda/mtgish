# Captured replacement values in Cantrip generation

A lasting replacement created by a resolving ability needs that particular
resolution's announced X. Reading the current source object, latest activation
or controller's history cannot distinguish two effects made by the same card.
The reviewed native API stores a u32 value in the existing replacement effect
kind and passes it explicitly to callbacks. Existing object/proposal/journal
layouts remain unchanged; no numeric domains are widened.

The generator detects reads of AnnouncedX in a newly created replacement and,
when the target supports captured replacement values, snapshots the creating
item's X. Only the deferred callback compilation reads `created_x`; the creation
expression still reads the resolving item. Applies, apply, then and damage
callbacks receive the explicit value. Nested helpers that lack a value argument
remain a reported gap instead of emitting an unbound Rust variable. Captured
prevention callbacks remain a gap; this does not claim that path implemented.

The feature is target gated and default off for the pinned older API. Its unique
marker is `pub const fn show_captured_value(self) -> GrantedReplacementDef`.
This includes the public-label prerequisite, rather than claiming compatibility
with the earlier capture-only API. Generated effects capturing publicly
announced X opt in to `.show_captured_value()`. Ordinary or private captured
values are not exposed by default. Existing targets retain the maker-X gap.

Aladdin's Lamp uses this interface, retaining the actual subsequent Draw and its
nested replacement interactions. Its independent Source correction removes the
unprinted selected-card reorder; see
[the reference decision](aladdins-lamp-leaves-card-20261114.md).
Taii Wakeen's activated damage replacement exercises the same capture contract.
Its unrelated triggered ability is deliberately excluded from the focused
second-user proof; no complete Taii card implementation is claimed.

The full supported Rust workspace passes 2,090 tests with 21 ignored. The focused
tests cover old-target gating, complete Lamp generation, the isolated Taii
activated replacement, and explicit helper scope safety. Against reviewed native engine `f8781d78d8321dcc126e394af10620cd66de3065`,
the corrected Lamp PR-form export compiled and passed three forced scenarios,
including both orders of same-source effects with different X values. The exact
isolated Taii activated-replacement PR form type-checked in that same build,
using temporary synthetic Oracle characteristics; no Taii runtime or complete
card result is claimed. Probe-only files are removed before native card work.

The tested Lamp export SHA256 is
`ecf0c05adb0377694d270f38b6d7a345f7459c6c418a57e9889e0da85881d14e`;
isolated Taii is
`dcc6dcfba95dc9319d7d7b875045d68e7d08ee43158cabbfb5dd1ebb42233584`.
Local source/input/log receipts are under
`/mnt/vizier/ice-age-completion-20261114/draw-packet/lamp-reference/` and
`lamp-author/`. Native card completeness, independent card review and the
combined native/managed gate remain separate pending work.
