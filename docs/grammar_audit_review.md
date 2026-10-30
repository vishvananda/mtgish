# Grammar audit review

The initial audit found 352 unused semantic captures and 13 possible instruction merges. Reviewing the matched card text confirmed errors in 159 templates. The repairs resolve 170 findings and change 212 records beyond the earlier Kessig Wolf Run, Tovolar, and Subversive Acolyte repairs.

Examples of corrected behavior:

| Cards | Correction |
| --- | --- |
| The Jolly Balloon Man; Mechtitan Core; Gideon Blackblade | Preserve every granted keyword or ability choice. |
| Brainstorm; Brainsurge; Smelting Vat | Keep distinct quantities for successive instructions. |
| Cosima; Valki; other affected modal double-faced cards | Use each face's own mana cost and characteristics. |
| Nim Deathmantle | Preserve the power/toughness bonus and pay mana to reanimate. |
| Crumb and Get It | Gift a Food, require a creature you control, and separate the pump from conditional indestructible. |
| Clavileño, First of the Blessed | Add Demon, then grant the triggered ability while retaining existing abilities. |
| Teyo, Aegis Adept; kicked pumps; The Irencrag | Retain separate instructions and conditional success gates. |
| Eladamri; Vassal's Duty | Redirect the next **1 damage**, rather than the entire next damage event. |
| Farrelite Priest; Dragon Whelp and related templates | Count activations separately from resolutions. |
| Starving Revenant | Count cards left on top by surveil and use the separate life-loss amount. |

The Rust source AST gained three forms to preserve distinctions that existing forms could not express: an activation-count condition, a surveil result count, and amount-limited redirection without an end-of-turn limit. Their JSON and TypeScript exports are synchronized. These source repairs do not extend the normalized operation schema or its coverage claims.

Ten grouping candidates required repairs. Answered Prayers, Flayer of Loyalties, and Rootwise Survivor already separate the relevant instructions. Their flags resulted from the heuristic comparing change families across different actions. Pumping and granting abilities in one instruction remains grouped when the source text calls for that.

## Review evidence

[The current audit](../data/grammar_audit.json) has no active findings and retains 192 findings under `reviewed_findings` for inspection. [The review record](../data/grammar_audit_review.json) includes the original and final grammar hashes, repaired templates, resolved finding identities, and an explanation and matched-evidence hash for every reviewed finding.

The revised detector handles the three grouping false positives directly: animation can specify stats and abilities with “becomes … with …,” base stats can use “has,” and gaining life is distinct from gaining an ability. With actual derivation evidence, it also clears 155 equivalent scalar captures automatically. It checks every matched occurrence against an output-referenced occurrence; different values remain findings. Implicit expression subjects and ability recipes do not qualify for this shortcut.

The other 37 findings, across 36 templates, currently need explicit context reviews. No detector rule checks card names. These reviews concern cost/result context, ability-sharing recipes, or expressions whose evaluation context matters. The earlier manual classification of 167 equivalent/literal captures and 25 implicit-context captures is preserved in the review history.

The standard `just parser_audit_templates` and corpus-generation recipes pass `--audit-review data/grammar_audit_review.json`. The fast static audit reuses the saved reviews only while all parser inputs and the effective grammar file order match their fingerprints. A usage audit additionally verifies the actual invocation's input files and fresh matched evidence. Changed templates, source cards, costs, preprocessing, grammar order, or context evidence reopen the affected review; a stale snapshot keeps context findings active. Omitting `--audit-review` gives the raw static findings. This review does not establish general semantic correctness for every grammar rule.

## Validation

- The full Go parser successfully processed all 33,529 records. The two damage-limit changes made afterward were checked by re-parsing their complete records with the final grammars and replacing those records by identity.
- Rust deserialized the complete final corpus and verified its identities and layouts against the original baseline.
- The complete corpus passed RON → binary → JSON export and pretty RON export. All three text exports contain the same 33,529 records and change the same 212 records.
- The Go test suite includes 32 real-card semantic regressions and completes in about two seconds. Thirteen focused Rust tests cover normalization, corrected source values, instruction grouping, and RON/binary round trips. A separate focused test compares all union members in the three changed TypeScript declarations to declarations generated from Rust.
- Detector regressions reuse the saved full-corpus derivation evidence to verify the 155 automatic cases and 37 context reviews without re-parsing the corpus. Negative cases retain unequal counts and real merges, and reopen changed input files, templates, costs, or matched uses. The expanded Go suite completes in about four seconds.
- The slow full parser pass ran once for this repair batch; focused rechecks covered later changes. A full TypeScript corpus compilation, full normalization coverage rescan, and Cantrip suites were not needed to validate these source repairs.

Run focused checks with:

```sh
(cd go_mtg_parser && go test ./...)
just ast_test
cargo test --locked --manifest-path rust_syntax/Cargo.toml -p rust_syntax \
  --features write_out_json --test json_bindings
```

Full normalization and Cantrip code generation remain outstanding work in [the AST plan](../AST_PLAN.md).


## GitHub reconciliation checkpoint

The [GitHub review](github_issue_review/README.md) includes all seven original issues and their five comments. It repairs 262 further templates and updates 371 additional corpus records. The new regression table includes 632 candidates, 146 with explicit semantic assertions. All 33,529 records survive typed Rust, binary, JSON, and pretty-RON export checks.

Four reviewed capture findings have now been repaired. The current detector totals are 151 automatic scalar equivalences and 37 guarded context reviews across 36 templates, with zero active findings and zero card-name exceptions. Earlier counts above describe the preceding checkpoint. The detector still has limited capture-family coverage; the GitHub regressions check additional target, filter, action, and zone errors.
