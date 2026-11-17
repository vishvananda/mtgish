# Grammar 0.3.4 / schema 192 adoption

The Source producer now consumes published `oracle-ir` and `oracle-ir-contract`
**0.3.0**, schema **192**, associated with Grammar **0.3.4**. The release pin is
`faces-oracle-20261112220158-s192-g0.3.4-r1`; its manifest SHA256 agrees with
both the published checksums and GitHub's asset digest. The frozen MTGish
**20261001.1** input remains **33,529 records**. No new Scryfall population was
adopted, and no upstream MTGish branch was pulled.

The adapter uses the owned contract's new representations: linked
`CreatedWith`, first-destination historical death objects, historical entry
sacrifices, and grouped source-conditioned mana replacement choices. Cross-ability
“the created tokens” uses the linked acquisition identity; local “tokens created
this way” still uses its actual creation receipt. Intrinsic basic-land mana is
compared through the owned semantic-card projection rather than a local alias.
Fresh producer selectors combine nested Matching predicates without discarding
historical arrival-zone qualifications. Singleton simplifications still require
an existing scalar proof.

Exactly two Source records change: **Naked Singularity** and **Reality Twist**.
Their four/five-item replacement lists now remain one Rule with source/output
alternatives. This preserves one effect identity and the tapping player's choice
on lands with several applicable basic land types. The English and MTGish
productions, Rust/TypeScript Source types, and matching RON, tagged JSON and
pretty RON exports change together. This Source grouping issue is reported as
[MTGish #302](https://github.com/i5jb/mtgish/issues/302).

The amendment regression contains the unchanged independently prepared complete
reference records for **75 groups / 988 amended faces / 987 records** through
U0076 (U0044 withdrawn). **985 records agree** under owned comparison. Two exact,
input-checked differences remain visible:

- [Dance of Many](https://github.com/vishvananda/oracle-grammar/issues/20#issuecomment-6934199376): the reference still uses broad MadeBy in two linked-token operands; the adapter uses CreatedWith. Requested reference correction.
- [Frankenstein’s Monster](https://github.com/vishvananda/oracle-grammar/issues/33#issuecomment-6934200147): nested Matching versus one Matching with AllOf. This is an equivalent-form normalization follow-up, not a new gameplay defect; Mimeoplasm already uses the flat reference form.

Gold is not edited to eliminate these disagreements, and production comparison
still exposes them. Historical Rust Source captures remain intact. The fresh Go
parser tests explicitly adopt three already-reviewed historical Source changes
with complete before/after strings (Frankenstein’s Monster, Mage Duel and
Patrician’s Scorn). The historical test-only delta assertions for Knowledge Pool
and Prowling Geistcatcher now use the current owned canonicalizer's compact
original selectors. No diagnostic capture bypass remains.

The full Rust workspace passes **1,596 tests** (10 ignored), and `go test ./...`
passes. Fresh parser generation reproduces every checked-in Source RON record.
All **33,529** current records export validated schema-192 IR with **zero lowering
diagnostics**. Migrating the previous full schema-191 export through the owned
contract succeeds for all 33,529 records; only 47 fresh producer records differ
beyond that migration, matching the linked, historical, grouping and bounded
selector changes above. Provenance, release and input hashes, commands and log hashes are
in [the adoption proof](oracle-grammar-034-adoption-20261117.proof.json).

Cantrip tooling is maintained separately on
[`tools/oracle-codegen-20261117`](https://github.com/vishvananda/cantrip-rs/tree/tools/oracle-codegen-20261117).
Its early-set reassessment found 43 of the paused 57 cards landed upstream,
Library of Leng newly implementation-ready, and 13 still needing engine features.
Snowfall's retained zero/one/two interpretation and Aladdin’s Lamp's corrected
Source lowering remain unchanged by this adoption.
