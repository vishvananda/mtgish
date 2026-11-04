# Compulsory effect outcomes

Some source records use `MustCost` for an ordinary effect followed by a test of whether it happened. Cosmic Horror and Infinite Authority test an actual destruction; Puppet Master tests an actual return to hand. Treating these as payments made the test succeed merely because the mandatory action was attempted.

The normalizer now lowers a compulsory, single-action destruction or return to its owner's hand as an ordinary effect. Later `CostWasPaid` references in that scope inspect its result. A return to hand also requires the resulting object to be in a hand. This does not change `PaymentChosen`: genuine costs still follow CR 118.11–118.12, including optional payments whose effect is replaced.

Validation:

- All 1,027 AST tests pass, including four whole-card and normalized-IR regressions. The tests retain genuine optional payment checks on Cosmic Horror and Puppet Master.
- Regenerating the 153 unfinished implementation records changes exactly Cosmic Horror, Infinite Authority and Puppet Master; all 153 still emit complete modules.
- Native scenarios on the validated integration foundation exercise failed and successful destruction, a missing dead creature, and a successful creature return followed by declining payment. Fresh generated modules pass those six cases.

Puppet Master has a separate remaining bug: after paying to return the Aura, the generated move still names its old battlefield identity. CR 400.7f permits this trigger to find the Aura's graveyard incarnation. This outcome repair does not resolve that reference, and Puppet Master must remain stopped until that path is fixed and verified.
