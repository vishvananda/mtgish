# Tapped-land Auras retain the original land

Kudzu's parsed instructions destroyed the Aura's current host and asked that host's controller to choose where to move it. Steam Vines already destroyed and damaged using the triggering land, but still asked the current host's controller to choose. Both now use the captured triggering land for those instructions. The Aura remains the independent source; the land's controller is read through its last known information after destruction.

The generator also accepts an immediately consumed host choice for attaching the ability's own source, using the existing complete attachment-legality query. It retains scope and use-count checks and refuses a source that has left the battlefield. Existing attachment escape tests remain unchanged, with two new source-choice refusal cases.

The complete 33,529-record parse and unified comparison changes only Kudzu and Steam Vines. All 185 reviewed usage records remain identical; only the two grammar-input fingerprints were refreshed. Schema remains 178. Full Source and Go suites pass.

Actual native tap-event scenarios cover moving the Aura in response, destroying the original land, preserving its new host, and asking the correct controller; Steam Vines also damages the correct player. Prior parsed references fail the moved-Aura cases. Those before modules use the new host-choice generator support to isolate the reference error; the original generator's refusal is separate evidence.

Reported upstream as [issue 199](https://github.com/i5jb/mtgish/issues/199). Source correction: `150412369c4ab152775afbc154445f5b4ef2306a`. Validation and exact scenario/module hashes are in the card-lane `integration/kudzu-host-choice-proof` folder. The probes create no claims or card approvals.
