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
the final Lamp PR-form export compiled and passed nine forced scenarios,
including both orders of same-source effects with different X values, source
control change, cleanup expiry, a short and empty library, nested draws and a
real draw trigger. The exact
isolated Taii activated-replacement PR form type-checked in that same build,
using temporary synthetic Oracle characteristics; no Taii runtime or complete
card result is claimed. Probe-only files are removed before native card work.

The tested Lamp export SHA256 is
`43d2642ec868a831e41216eea16c24b901579ad577ad77e9a91ef497923655fb`;
isolated Taii is
`dcc6dcfba95dc9319d7d7b875045d68e7d08ee43158cabbfb5dd1ebb42233584`.
Local source/input/log receipts are under
`/mnt/vizier/ice-age-completion-20261114/draw-packet/lamp-reference/` and
`lamp-author/`. Native card completeness, independent card review and the
combined native/managed gate remain separate pending work.


A follow-up client inspection found that generic choices whose results are read
later were labeled `ChooseAsEnters`, even during ordinary resolution. That
purpose adds entry and battlefield wording inappropriate to a library choice.
The generator now derives that neutral purpose from the actual entry callback
context. Constrained choices, earlier references and counter distribution use
the same context. Real entry choices keep their entry wording; ordinary choices
use the truthful neutral `Choose`. The hand-written Lamp can additionally name
the choice as a card to leave on top, but no presentation field or new semantic
AST variant is needed to correct the generator.

The 2,090-test full run applies to the captured-X baseline77c7afe7. After the
prompt follow-up, focused validation passes94 library unit tests,3 captured
replacement tests and1 counter test, including ordinary versus actual entry
contexts with and without selection constraints. The final native9-scenario
probe above uses the corrected prompt. Its PR-form source differs from the
previous three-scenario probe only by `ChooseAsEnters` becoming `Choose`;
the isolated Taii form is unchanged. Earlier runtime passes alone did not
validate the old prompt's meaning. This is our backend inference bug, with no
additional upstream Source reference or portable AST change required.
