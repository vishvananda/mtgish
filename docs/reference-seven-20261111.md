# Seven reference corrections, 2026-11-11

The latest oracle-grammar review catalog (`current-reviews-ordinary-reference-forms.json`)
contains four confirmed outstanding reference defects. The chosen-player templates
also affect three additional cards, found by regenerating the complete corpus.

| Cards | Correction |
| --- | --- |
| Lost Order of Jarkeld, Pallimud, Skyshroud War Beast | Chosen-player self power/toughness definitions are characteristic-defining abilities. |
| Entropic Specter, Haunting Apparition, Sewer Nemesis | Same template defect; their formulas and other abilities are retained. |
| Trespasser's Curse | Capture the entering creature's controller as the trigger's player subject; use that player for life loss. |

The six self definitions meet CR604.3a; CR208.2a even uses Lost Order of Jarkeld
as its example. Their classification must not turn into an ordinary battlefield
static merely because the formula refers to a chosen player. Existing `CDA_Power`
and `CDA_PowerToughness` Source constructors and normalized characteristic-defining
statics already express this. The fix changes four template outputs, with no new
AST form. See the [current Comprehensive Rules](https://media.wizards.com/2026/downloads/MagicCompRules%2020261106.pdf).

For Curse, the live enchanted-player query remains in the event filter, where it
belongs. The life-loss instruction instead reads `PlayerRef::EventSubject`, captured
when a creature enters under that player's control. Gaining life still reads the
ability controller. Moving the Aura before resolution must not redirect the loss.
The existing player-binding entry event is required in Source: merely replacing
`HostPlayer` in the action leaves the older generic entry event without a bound
`ThatPlayer` during strict lowering. No new event or engine interface is needed.

The full Go suite passes. Twenty-three Rust tests pass across the new seven-card
regressions, optional delayed-scope controls, characteristics, delayed triggers and
Blinding Beam. Tests compare complete canonical ability programs, assert CDA and
captured-player types, and check that all seven tagged Source JSON records equal
their RON counterparts. Fixture expectations for the original four come from the
independent oracle-grammar proposals; the three additional expectations preserve
baseline operands while changing only static classification.

A fresh parser run and native export each produce 33,529 records: exactly these
seven change and 33,522 remain unchanged. Schema188 is unchanged. Audit review
entries are untouched; only the two input fingerprints for the edited grammar
are refreshed. The [proof receipt](reference-seven-20261111.proof.json) records
these checks. Evidence is in `/mnt/vizier/reference-seven-20261111`.

The [optional delayed cases](optional-delayed-scope-20261111.md) are resolved
separately as retaining the guarded native programs, rather than new reference
fixes. The [AST review](ast-ui-review-20261111.md) proposes future semantic cleanup;
no schema migration is included in this correction batch.

The refreshed browser catalog passes all 33,529 exact block roundtrips, independent
binding-port comparisons and Rust/Wasm validations. Fourteen actual rendered
workspaces (the seven corrected cards and seven delayed controls) preserve exact
IR and validate; all six CDA cards appear in Characteristics / all zones. No
browser JavaScript errors were observed. The existing Wasm binary is reused.

Reported upstream: [chosen-player CDAs #260](https://github.com/i5jb/mtgish/issues/260)
and [Curse event player #261](https://github.com/i5jb/mtgish/issues/261).
The fix commit is `7fdef2f0b88686380955c9b200e1f98beeac78a7`.
