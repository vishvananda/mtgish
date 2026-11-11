# Three more shared-verb counter placements

Guide of Souls, Champion of Dusan and Unexpected Fangs had the same reference
error as [issue259](https://github.com/i5jb/mtgish/issues/259): one shared “put”
instruction was lowered to two separate PutCounters events. Each now emits one
operation containing both counter-kind contributions.

| Card | Contributions retained |
| --- | --- |
| Guide of Souls | Two +1/+1 counters and one flying counter on the attacking target; its separate Angel type addition remains afterward. |
| Champion of Dusan | One +1/+1 counter and one trample counter on its target. The graveyard activation, exile cost and sorcery restriction stay intact. |
| Unexpected Fangs | One +1/+1 counter and one lifelink counter on its target. |

The correction uses the existing placement batch, not a new AST or engine feature.
It applies the same ordered/simultaneous-instruction distinction established in
[counter-placement-timing-20261110.md](counter-placement-timing-20261110.md), under
[CR608.2c/f](https://media.wizards.com/2026/downloads/MagicCompRules%2020261106.pdf).
There is one placement verb and one shared recipient, with no sequencing boundary
between the two kinds. Both contributions must survive; separate explicit “put”
instructions must not be globally coalesced.

Two duplicate grammar entries for the one-and-one wording are corrected together.
Guide's reflexive energy-payment template is corrected separately. The existing
general two-plus-one target template was already a batch and is retained.

A full parser regeneration changes exactly these three of33,529 Source records;
33,526 remain unchanged. Tagged Source JSON agrees with RON. Tests compare complete
canonical programs against the prior programs with precisely the reviewed batch
merge; unused result numbering is canonicalized. Five already-batched controls
are unchanged: Captain Marvel, Donald Blake, Kheru Goldkeeper, Quicksilver and
Sagu Pummeler. The complete8-card Go fixture and Rust program/Source JSON checks
pass. This follows the user's oracle-grammar worker finding; the new packet was
not yet present in the local report tree when the card names were supplied.

The schema189 [AST migration](ast-human-189-20261111.md) is independent. The old
split programs remain observably different and are not accepted through an alias.
Source-only changes and their regression packet are committed separately for
upstream review.

Upstream follow-up: [issue259 comment](https://github.com/i5jb/mtgish/issues/259#issuecomment-6792055025).
Fix commit: `4aecbb3c6be11d023526d8782a0c94a7cf922320`.
