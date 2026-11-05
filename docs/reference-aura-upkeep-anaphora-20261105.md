# Aura upkeep references

The upkeep templates now distinguish the object and player named by the trigger from the Aura's current attachment. For example, Essence Flare and Unstable Mutation refer to “that creature”; the source parser now emits `Trigger_ThatPermanent`, while an explicitly current `HostPermanent` remains dynamic. This uses existing schema 178 values.

Fresh before and after parser passes each contain 33,529 records. The raw and typed unified exports each change exactly 14 cards: Apathy, Curse Artifact, Dance of the Dead, Erosion, Essence Flare, Mind Whip, Paralyze, Paroxysm, Slow Motion, Soul Tithe, Takklemaggot, Unnatural Hunger, Unstable Mutation, and Venarian Gold. A dynamic-host control and generated-module tests distinguish the two references. The full 37 contextual audit findings and 148 automatic reviewed findings, including their usage records, are unchanged; the grammar-input fingerprint is refreshed.
