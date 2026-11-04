# Step-delayed references

The `delayed-subject-members` capability detects native `Game::delayed_trigger_with`. It exposes the existing
singular subject and real-object list together, for step delays only. The API pin is unchanged by this source
change; an older profile reports the missing capability instead of emitting a complete module.

Step closures collect creator-scope expressions with proved cardinalities. A singular `ObjectRef` is at most one;
collections require `one_object` or existing single-selection evidence before they can use the subject. After all
reads, including nested closures, a deterministic layout reserves the subject for a captured player or chooses one
proved singular object group. At most one remaining group uses members. Per-closure readers retain that mapping
independently of discovery order. Distributed counter shares retain their existing real-object multiplicities.

Infinite Authority retains the original event subject separately from the opposing creature, and passes the first
creature forward through its nested end-combat/end-step callbacks. Gift of Immortality captures the Aura's first
graveyard incarnation while the creating enchanted-creature departure is still in scope, separately from the
returned host. Next of Kin uses the same layout for each creature put onto the battlefield; its independent
`engine: no commander` requirement remains explicit.

No fabricated IDs, separators, source substitutions, or runtime truncation of plural groups are used. Multiple
plural groups, excess independent references, and captured object reads in step-trigger target filters remain
explicit gaps. Event and reflexive capture requirements are not landed by the new step capability. The obsolete
Endless Whispers source-following shortcut is replaced by independent player and first-graveyard-card captures.
