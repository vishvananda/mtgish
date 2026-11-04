# Oracle-grammar schema 177 on main

Merge the published `oracle-grammar-schema-176` compatibility branch through
`c6013b8c4fe634b0af4bd0e844ce7c814c0d7072` into main. Schema 177 is now the common
contract; the unfinished Aura schema-178 prototype is preserved separately and
is not part of this merge.

`mtgish-ir` owns the normalized types, counters, generated visitors and schema
contract. `mtgish-ast` reexports them and includes the canonicalization and
presentation APIs used by oracle-grammar. The default legacy producer and the
Cantrip feature remain available. The schema-177 contract snapshot is byte-for-byte
identical to the published frozen package at `106840242`.

Main uses the local workspace source and IR crates, rather than compiling older
Git-pinned copies of itself. This keeps main's source, types and runtime consistent
as development continues. Existing immutable compatibility revisions and external
consumer pins remain available; adopting main in an external workspace requires
using its matching source, IR and runtime types together.

Validation:

- Full Rust workspace with Cantrip enabled: 1,507 passed, zero failed, four ignored.
- API-only library without default features: 57 passed, zero failed, one ignored.
- Full Go parser suite passed.
- All 33,529 source records exported and validated as unified IR. Comparing complete
  serialized records with the prior main export finds exactly the schema headers
  changing from 176 to 177 (67,872 fields), with no other differences.

Receipts and logs are in
`~/card-lane/pipeline-unlimited-20261103/integration/schema-177-main-*`.
