# Heroism: payment at resolution, then a waiting shield

Heroism's English template placed `{2}{R}` payment inside a future combat-damage
replacement. Its [2004-10-04 Wizards ruling](https://api.scryfall.com/cards/08ee87a0-a7eb-4472-9045-85d11e8a1501/rulings)
places the payment when the activated ability resolves. The creature set is
also selected then: the earlier activation cannot pick up creatures that become
red or attacking afterward.

The source template now enumerates the attacking red creatures with
`ActionForEachPermanent`. Each creature's controller may pay; only a decline
creates a prevention effect about that exact creature. The existing AST uses
`ForEachObject`, `PayCost`, `If`, and `CreateReplacement` with
`SameObject(Bound(...))`. Its waiting body contains only `PreventDamage`.

The native engine already supports the handwritten implementation. A subsequent
native preflight exposed a remaining IR/backend question: the object loop does
not encode APNAP payment choices, each controller's choice of order (CR 101.4c),
or gathering unpaid creatures before creating the shields. With two differently
powered red attackers and only one affordable payment, battlefield iteration
must not decide which creature's payment is offered first. The backend now keeps
this program explicitly held instead of claiming that the timing correction
alone makes automatic generation correct. Oracle Grammar owns the decision about
representing the remaining choice and batching semantics; no schema or numeric
domain was changed here. The old comment
in `cantrip/lane_n.rs` is not evidence that Heroism needs a new damage-time
payment API; that API remains a gap for any actual card that requires it.

Fresh parsing of the existing 33,529-record source input changes exactly Heroism.
RON, tagged JSON and pretty RON each update only its record. Existing source
compatibility aliases elsewhere are left untouched rather than reserialized as
unrelated changes. All 182 active grammar-audit findings retain their IDs and
complete matched usage; only the grammar-input fingerprints are refreshed, and
all 193 historical reviews are preserved. No new audit exception is introduced.

`data/oracle_grammar_reference.json` retains its published bundle pin. Its active
source checksum follows the corrected local source, and its approved-change
count becomes six: five earlier adoption changes plus this correction. This
does not replace the frozen Oracle population with another Scryfall snapshot.

Reported as [MTGish #288](https://github.com/i5jb/mtgish/issues/288) and
[oracle-grammar #23](https://github.com/vishvananda/oracle-grammar/issues/23).
The grammar worker's dirty working tree, pinned exports and update ledger were
not edited. The reference owner should reconcile the corrected output through
its normal independently pinned adapter-update process.

The parser regression checks the actual English parse against the corrected
source fixture. The IR regression verifies that payment precedes shield creation
and that the waiting body cannot ask for late payment. A backend regression
checks that automatic generation retains the explicit ordering gap, including
when the exact same program is given a different card name.
