# Generator adapters for the early-set foundation

The first four engine features are on Cantrip fork branch
[`engine/early-set-foundation-20261112`](https://github.com/vishvananda/cantrip-rs/tree/engine/early-set-foundation-20261112),
currently through `1617f06d8`. Each feature is a separate commit:

| Commit | Native feature | Affected early-set cards | Newly free of recorded feature gaps |
| --- | --- | ---: | ---: |
| `5c1c4d363` | Blocking as though landwalk were absent | 9 | 9 |
| `756278d37` | Independent base power/toughness settings | 4 | 4 |
| `d4bf69bb8` | No combat damage assignment after an earlier choice | 8 | 8 |
| `1617f06d8` | World supertype and continuous-interval world rule | 12 | 8 |

These 33 affected cards are distinct; 29 lose all recorded feature gaps under the
candidate profile. Caverns of Despair, Field of Dreams, Land's Edge and Revelation
retain their other blockers. This does not add any card to Cantrip or resume the
paused card campaign. The other five feature groups in the initial roadmap
remain queued; this is the first foundation batch.

## Capability-gated generation

The adapters recognize `landwalk-blocking-exceptions`, `independent-pt-settings`,
`no-combat-damage-assignment` and `world-rule`. They emit the actual foundation
interfaces, including the required `Supertypes::world` field on every candidate
card. A landwalk blocking exception does not remove a keyword or enable ignoring
unrelated abilities. A negative regression probe preserves the gap for ignoring
flying. The World clock and all ordinary game quantities stay in the engine;
there is no AST/schema change or quantity widening.

The normal snapshot still targets upstream Cantrip `6814db9c8` with API fingerprint
`31d0e482b00c413d94f371d6cde3b468b35221d47f0ca6d5cf90c64ee2612bff`.
No vocabulary pin was silently advanced to the unmerged foundation. Candidate
probes use `planned::with_additional_features`; their headers retain the pinned
snapshot's API identifier, so the exact candidate engine provenance is recorded
separately in the [validation receipt](cantrip-foundation-adoption-20261112.proof.json).
The normal corpus's generated Rust bodies and gap lists are unchanged across all
33,529 records, excluding generator metadata comments.

The existing vocabulary tool detects all four features from the foundation:

```sh
python3 rust_syntax/normalize/tools/update_cantrip_vocabulary.py \
  /path/to/foundation --out /tmp/foundation-vocab.rs
```

Regenerate the normal snapshot only when deliberately adopting that engine
revision. The candidate API/state/mechanics fingerprint is
`9af791a8f438fcce8ba2fc7209c4afbc790433f502873ef14fe3e8a9b69cc002`.

## Validation

- The complete native Rust workspace passed 22,556 tests, with 15 ignored, through
  `tools/test.sh --engine` and `CANTRIP_VERIFY=1`.
- The generator suite passed 1,569 tests, with 22 optional tests ignored.
- All 33 candidate card modules type-check. Delif's Cube needs the normal
  `CounterKind::Cube` declaration when its card is submitted. That compile probe
  uses an explicit names-only overlay appending Cube at 91, preserving IDs 0–90;
  it adds no counter behavior and is not part of the engine foundation.
- Twenty-five generated-card scenarios passed against the pristine foundation,
  including actual generated Great Wall blocking, Gaze of Pain's noncombat
  damage/no-assignment alternative, Arboria's World replacement, and Island of
  Wak-Wak's one-axis setting. The preceding 21-scenario corpus also passed
  against its unchanged older native target.
- The default target produces identical Rust bodies and gap lists for all 33,529
  corpus records. Only generator metadata comments differ, so the previous full
  corpus compile remains applicable to that target's code.
- The 128-game native benchmark measured 45.24 μs/engine decision before and
  45.44 μs after the four features, about 0.4% difference on this shared machine.
  The reported allocation rate was unchanged at 26.74 KB per decision. An
  earlier after-run during other builds measured 45.73 μs.

The full `tools/test.sh` attempt stopped before tests because `dotnet` is not
installed. No .NET pass is claimed. The native host already defines the World
supertype in its protocol; its view now includes it when appropriate. The new
features add no prompts or tactical-bot changes.

To execute the checked-in native scenario probe:

```sh
MTGISH_CANTRIP_DIR=/path/to/foundation \
MTGISH_CANTRIP_TARGET=/path/to/separate/native/target \
CARGO_BUILD_JOBS=8 RUST_MIN_STACK=16777216 cargo test \
  --manifest-path rust_syntax/Cargo.toml -p mtgish-ast --features cantrip \
  --test cantrip_foundation foundation_generated_cards_execute_in_cantrip \
  -- --ignored --nocapture
```

Evidence is under `/mnt/vizier/early-set-foundation-20261112`. The foundation
worktree and branch are retained for the next focused engine commits. The user's
older Cantrip main, card reservations and paused workers are unchanged.
