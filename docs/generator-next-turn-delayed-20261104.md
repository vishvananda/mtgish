# Delayed steps in a specified next turn

The native foundation adds `DelayedWhen::StepOfNextTurn { step, of }` at local
checkpoint `bf342add9fd6bbe1a8a57c4dc95b5b7de1b287da`. The generator targets that
tested API with the `next-turn-delayed` feature. Older targets report the real
gap instead of silently treating a next-turn upkeep as the next upkeep.

An additional beginning phase can provide another upkeep in the creation turn.
A skipped upkeep can otherwise defer an ordinary next-upkeep trigger into a
later turn. The new native timing selects the first actual turn beginning after
creation, optionally for a particular player, and expires when that selected
turn ends. It reads actual turn-beginning events after the delayed ability's
creation; extra turns count and skipped turns do not begin. An ordinary next
upkeep keeps its existing behavior. Mishra's Bauble and Urza's Bauble are the
two existing native callers corrected alongside the engine change.

The host also exposes waiting ordinary-step and next-turn-step delayed abilities
through its existing pending-effects display. A follow-up client caption repair
removes the blanket claim that every pending ability expires at end of turn if
no spell is cast. These waits have different lifetimes.
The combined local worker checkpoint is now
`7c93fbf4052aa8074aa00b35c91d327316b2ec87`; its native workspace repeats the
same 17,158 passing tests. Queued jobs were moved to this checkpoint before
claim, and caption-blocked jobs retained their fixes on explicit retry.

Validation:

- The native Rust workspace passes **17,158 tests across 134 targets**, with
  15 existing ignored tests. Ten focused engine cases cover repeated and skipped
  upkeeps, extra turns, selected-player turns, expiration, a departed player,
  and rollback/replay. Four cases fail against the ordinary-step baseline.
  Two host tests cover both Baubles and an ordinary next-step ability.
- Consecutive 128-game benchmarks with `CANTRIP_VERIFY` unset measure
  **36.33 / 35.73 microseconds per engine decision** before/after. Earlier
  unpaired runs varied with machine load; no speedup is inferred.
- **956 generator tests pass across 92 targets**, with two existing ignored
  tests. Explicit old/new feature tests cover 23 next-turn upkeep cards,
  Hazezon Tamar as an unchanged ordinary-upkeep control, and four next-turn
  abilities carrying object lists to a named player's step.
- All **33,529 ordered output identities** remain unchanged. Exactly 51 existing
  whole modules change only their delayed timing and its field name; four
  formerly held modules become whole: Kardum, Patron of Flames; Kav Landseeker;
  Meandering Towershell; and Suppress. No whole module becomes held.
- **23 exact full-source generated modules compile** against the clean native
  checkpoint, using real existing native or worker-packet Oracle content.

Twelve blocked reviews were explicitly migrated to the validated engine base:
Enervate, Flare, Force Void, Formation, Foxfire, Krovikan Fetish, Mind Ravel,
Pyknite, Portent, Touch of Death, Infuse and Heal. Eight previously approved
candidates also used the wrong timing: Barbed Sextant, Blessed Wine,
Clairvoyance, Gravebind, Lightning Blow, Panic, Ray of Erasure and Updraft.
Their approvals were superseded by correction jobs retaining the complete
approved candidate, including previous fixes and scenarios. All twenty still
require normal Terra review and verification; shared validation is not card
approval. Touch of Vitae remains separately blocked by acquired-ability
instance identity.

Proofs and frozen before/after evidence are under
`~/card-lane/pipeline-unlimited-20261103/integration/next-turn-{proof,generator-proof}`
and `integration/blocked-followup-20261104`. Native changes remain local during
consolidation. The semantic schema remains 174; no source corpus or immutable
reference changed. .NET and browser tests are unavailable in this environment.
