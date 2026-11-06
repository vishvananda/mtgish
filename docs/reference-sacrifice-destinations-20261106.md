# Sacrifice triggers retain their first public-zone destination

Reported upstream as [i5jb/mtgish#219](https://github.com/i5jb/mtgish/issues/219).

The old English templates for It That Betrays, Nazgûl Battle-Mace, Bag of
Devouring, and Biolume Egg encoded the sacrificed card as
`Trigger_ThatGraveyardCard`. Sacrifice is not a guarantee that the card arrived
in a graveyard: a replacement effect can send it to exile instead.

[Comprehensive Rules 400.7e and 603.6c](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt)
allow these triggered abilities to find the new object in the first public
zone it entered. A later move creates another object, which they cannot find.

The source now expresses the triggered card explicitly using appended actions
`PutTriggeredCardOntoBattlefield` and `ExileTriggeredCard`. Their lowering uses
the existing normalized `ObjectRef::EventDestination`; no normalized schema
shape, numeric range, or engine feature changed. Old source enum discriminants
are preserved. It That Betrays also loses an unprinted exclusion of the ability
source and redundant permanent type enumeration.

All four source records were regenerated through the official converters;
the other 33,525 records are unchanged. Fresh audit evidence preserves the same
182 finding identities and their complete matched uses, with updated parser
input fingerprints. No new audit exemptions were introduced.

Validation: 1,300 AST tests pass (four explicit native tests remain ignored in
the ordinary gate); all 33,529 corpus records normalize and validate. The
explicit Cantrip gate passes 12 gameplay scenarios. Its new sacrifice scenario
covers normal graveyard arrival, replacement exile, and a second move before
resolution, comparing the corrected AST against the historical graveyard-only
AST. The existing Cantrip engine executes the corrected generator output.
