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

Publish the confirmed cards in independent PRs, normally twenty new cards each.
The user approved putting Sorrow's Path into the last batch as its twenty-first
card rather than creating a separate one-card packet: ten PRs for 201 new cards.
Preserve one commit per card and its existing independent review evidence. Every
group has the same foundation parent; no group depends on a sibling card group.
Validate the combined approved export and each group's actual card scenarios and
native test fixtures. Do not repeat the completed semantic card reviews.

Open validated card packets ready for joint review with the foundation. The
maintainer's November 13 comment on #2875 explicitly holds foundation review
until its first consuming card PR arrives. Keeping all card PRs draft until the
foundation merges would prevent that review. Link the actual card PRs from the
foundation description as they are published, retaining a card-only comparison
in each packet so the shared engine changes can be reviewed with their consumers.

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

## Published packets

All 201 confirmed cards are published and ready for joint review with #2875.
The final audit read each actual upstream PR and verified its expected head,
base, readiness, maintainer editing, reservation-closing lines, one card commit
per card, isolated validation receipt and publication database entry.

| Batch | New cards | PR |
| --- | --- | --- |
| 01 | 20 | [#2993](https://github.com/cantrip-rs/cantrip-rs/pull/2993) |
| 02 | 20 | [#2994](https://github.com/cantrip-rs/cantrip-rs/pull/2994) |
| 03 | 20 | [#2995](https://github.com/cantrip-rs/cantrip-rs/pull/2995) |
| 04 | 20 | [#2996](https://github.com/cantrip-rs/cantrip-rs/pull/2996) |
| 05 | 20 | [#2997](https://github.com/cantrip-rs/cantrip-rs/pull/2997) |
| 06 | 20 | [#2998](https://github.com/cantrip-rs/cantrip-rs/pull/2998) |
| 07 | 20 | [#2999](https://github.com/cantrip-rs/cantrip-rs/pull/2999) |
| 08 | 20 | [#3000](https://github.com/cantrip-rs/cantrip-rs/pull/3000) |
| 09 | 20 | [#3001](https://github.com/cantrip-rs/cantrip-rs/pull/3001) |
| 10 | 21 | [#3002](https://github.com/cantrip-rs/cantrip-rs/pull/3002) |

The complete export passed `tools/test.sh --engine`: 24,872 Rust tests across
301 targets, with 15 ignored and no failures. Every packet separately passed its
changed scenarios, focused card corpuses, native fixtures and layer contract.
All 45 pairs of sibling branches merge cleanly. The frozen foundation is unchanged.

Publication checks repaired Oracle-word metadata in Chaos Moon and Seraph,
restored Rabid Wombat’s independently approved synthetic-board test entry, and
made the three Tron probe registries explicitly shadow the real companion cards.
These repairs are folded into their respective card commits.

Twenty reserved cards remain outside these ten PRs. Their engine or generator
work proceeds as the focused manual packets described above; the broader card
completion goal remains active.
