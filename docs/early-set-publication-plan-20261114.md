# Frozen foundation and independent early-set card packets

The foundation feature set is now frozen at upstream PR
[cantrip-rs/cantrip-rs#2875](https://github.com/cantrip-rs/cantrip-rs/pull/2875):
linear publication head `508215f0a6c2c2228b40332b590b4a5bfde92cb4`, corresponding
worker-history checkpoint `8de2bccb5f04270e217ecd8582843c59d327abde`. Their complete
source trees are equal: `dc000a056861fe16944343805c23b8fe83c9caf7`.
The full suite passed 23,962 Rust and 1,526 managed tests. Both the tapped-blocker
and player-loss repairs are already included in this checkpoint.

The campaign's `foundation_lock_manifest` pins that checkpoint. Automatic
snapshot promotion rejects a different head, including one with a successful
test receipt. Collection of confirmed card work continues without promoting the
moving integration tip into the foundation.

Publish the confirmed cards in independent PRs containing at most twenty cards.
Preserve one commit per card and its existing independent review evidence. Every
group has the same foundation parent; no group depends on a sibling card group.
Validate the combined approved export and each group's actual card scenarios and
native test fixtures. Do not repeat the completed semantic card reviews.

The integration branch is an assembly workspace, not the publication manifest:
it still contains a withdrawn Reality Twist candidate. Exclude all unconfirmed
files from the export. Its four existing-card sharing migrations (Life Chisel,
Ritual of Subdual, Spectral Shield and Time Bomb) must be accounted for explicitly,
using helpers already in the frozen foundation, without creating a dependency
between card groups.

Remaining engine-dependent cards are handled manually as focused packets based
on the frozen foundation: an engine commit followed by one card commit per
consumer. Combine cards sharing the same new engine feature in one packet. Do
not expand the common foundation to absorb these later features. Necessary
publication conflict repairs preserve this feature boundary and require the
appropriate validation; locking the feature set is not a reason to discard
upstream work or leave publication conflicts unresolved.
