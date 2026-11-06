# Computed payments and independent counterspell instructions

The current-producer review confirms two different causes in the worker's
reference mismatches. Neither requires a new engine feature or schema change.

Rethink, Spell Rupture and Thassa's Rebuff already have correct source records:
they define X, then refer to it in `PayManaX`. The Rust cost-recipe binder discarded
that reference and emitted plain `Mana([X])`. It now emits `ManaWithX` using the
existing bound value (or a captured activation X). Ordinary casting X retains its
existing representation. A full export found one more caller, Primordial Ooze;
its upkeep payment now shares the counter snapshot already used for its damage.

Offering to Asha, Stymied Hopes, Frightful Delusion and Sage's Dousing instead have
English-template scope mistakes. Their following life gain, scry, discard or
conditional draw belongs after the counter-unless instruction. Paying prevents
the counter attempt but does not skip that following sentence. All four templates
are reported upstream in [issue 204](https://github.com/i5jb/mtgish/issues/204).

Fresh parsing changes exactly those four source records. The complete typed
export changes eight card records: those four and the four computed payments.
For the computed payments, the full-record comparison proves that only the mana
cost's X binding changes. For the parser fixes, the structural comparison proves
that only the following instruction moves out of the nonpayment branch.

All 33,529 records remain structurally preserved, fully semantic and valid at
schema 178. All 185 template-audit findings retain identical complete usage
evidence. The full Go suite passes, as do 1,223 AST tests (three existing ignored
tests). A further focused run passes all four regressions, including Primordial
Ooze's payment/damage snapshot and the unchanged Condescend/Overrule casting-X
controls. The original computed-payment and independent-followup regressions
failed before the fixes.

Other entries in the [reference review queue](reference-followup-20261106.json)
remain under review; this batch does not declare every reported mismatch fixed.
