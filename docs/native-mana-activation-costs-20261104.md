# Mana activation costs and search destination captions

Native checkpoint `6816f11edcf2693cf0a00f34d8dd744ebacfe535` is validated and promoted locally. Cantrip publication remains paused until the remaining blocked cards and campaign consolidation are finished.

## Activation costs

Commit `3b8e4bc9463d9801ce33cb087300f8adb1b51232` fixes a rules gap exposed by Power Artifact enchanting Celestial Prism. Mana abilities previously checked and paid their printed mana costs, bypassing the ordinary activation cost changes. Availability, mana planning, actual payment and host cost captions now read those changes. The actual total is locked after X and payment announcements, before tapping or exiling the source. Hybrid announcements reuse existing activation procedures; advice remains open for several distinct possible totals, as it does for other unmodeled payment choices.

Power Artifact and Forensic Gadgeteer are two printed users of mana-ability reductions. Oppressive Rays is an example of a mana-ability increase. Anointed Peacekeeper explicitly excludes mana abilities; its pre-existing callback incorrectly relied on the core skipping them all. The callback now expresses that exclusion itself. Its existing scenario exposed this during the first workspace run and is unchanged.

Eleven focused core tests cover reduced-cost offers, actual payment, planner conversion, increases, the one-mana floor, free abilities, colored symbols, X expansion, locking before a tap, and added life. Nine fail before the core fix, while two controls already pass; all eleven pass afterward. A host regression proves the displayed total and added life, then removal of the modifier. The reviewer's exact retained Power Artifact/Celestial Prism card module and scenario also pass against the final core.

The final verification-enabled native workspace run passes **17,194 tests across 135 targets**, with 15 existing ignored tests. The first run's sole failing scenario was Anointed Peacekeeper's explicit mana exemption; the final run includes its card filter correction. A 128-game benchmark with verification unset observes 39.87 microseconds per decision before and 34.90 afterward, with the same game/decision statistics. This single pair shows no slowdown; it is not a speedup claim.

## Search recipient

Commit `28b88f436` fixes the existing web search presenter: cards going to the searched owner's hand name that owner, including Jester's Mask choosing from an opponent's library. The same owner wording applies to graveyards and the hand branch of a land-or-hand search. Existing own-library wording remains covered.

The real browser regression fails on the original caption (it says your hand), then passes after the correction; an own-hand control also passes. The web project builds successfully. The rendered screenshot was inspected and shows both Bot 2's library and Bot 2's hand. No protocol field or new client capability was added.

## Recovery and proof

Power Artifact and Jester's Mask have new independent review candidates based on the validated checkpoint. Their card, content and scenario bytes are identical to the retained rejected revisions. Previous jobs, reports, rejection histories and full patches are preserved; this is an explicit recovery on a materially fixed foundation. Ordinary review, machine gates and independent confirmation still apply.

Machine receipts and hashes are retained under:
`/home/vishvananda/card-lane/pipeline-unlimited-20261103/integration/mana-ability-cost-proof/`
and `integration/search-destination-owner-proof/`. `validation.json`, `promotion.json` and `requeued-cards.json` are the checkpoint and recovery records.

The API/mechanics fingerprint remains `152f4651d82b88dd6aeea358f46fd1678369a58d84985e650ca051dedbf63979`; no generator vocabulary refresh is needed for these rules/host/client changes. The independent AST consolidation checkpoint is on fork/main at `cf3ae92` (schema 176), with 995 merged generator tests passing. Normalized binary caches must be regenerated for that schema; do not relabel old binary data. No released held cards were re-reserved.
