# Aura and Equipment spell filters

Oracle Grammar's current `adapters/mtgish/reference-issues.md` contains one new
dispute: Codsworth, Handy Helper. It is confirmed and reported as
[MTGish issue 276](https://github.com/i5jb/mtgish/issues/276).

Both `{{PotentialSpells}}` templates, “Aura and Equipment spells” and “Aura
and/or Equipment spells”, incorrectly produce an intersection. These are two
eligible classes: an Aura without Equipment qualifies, and an Equipment without
Aura qualifies. A spell with neither subtype does not. Existing `Or` Source and
`AnyOf` IR represent this directly; no AST layout, schema or engine change is
needed.

The same producer defect affects four cached records:

| Card | Corrected filter |
| --- | --- |
| Codsworth, Handy Helper | Mana spend restriction |
| Danitha Capashen, Paragon | Generic spell cost reduction |
| Golden-Tail Trainer | Power-based spell cost reduction |
| Sigarda's Aid | Permission to cast as though the spell had flash |

The generic templates now produce `Or([IsEnchantmentType(Aura),
IsArtifactType(Equipment)])`. All three current Source representations carry
exactly these four record changes; the other 33,525 records are unchanged.
Existing complete-record fixtures containing Golden-Tail Trainer are
updated to follow the corrected producer. Historical proposal fixtures are not
rewritten.

The eight-record parser regression includes the four corrected cards, Mantle of
the Ancients, Unfinished Business, Danitha, New Benalia's Light, and Mycosynth
Golem. The first three controls already use the Aura/Equipment union. Golem's
“artifact creature spells” remains an intersection: changing the two class-list
templates must not change qualifications requiring both card types.

Raw typed-IR assertions cover the actual mana, cost and timing filters on all
four changed cards, validate each complete card, and check its round trip.
The tagged Source JSON regression compares the complete eight records with RON.

For the Oracle Grammar worker: this is a confirmed producer bug, not another
equivalent-form dispute from the earlier 36-face consistency review. Adopt the
corrected reference when advancing the MTGish pin and then remove the Codsworth
dispute allowance. Oracle Grammar's worktree, independent gold and pin were not
edited here. The 32 pending consistency cases remain separate work.

Validation results and input hashes are recorded in the companion
[receipt](reference-codsworth-20261112.proof.json).

Final validation: the complete Rust workspace with Cantrip passes 2,018 tests
(24 ignored, 197 targets), and the uncached complete Go suite passes in
90.692 seconds. Both commands exit successfully.
