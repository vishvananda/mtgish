# Aura identity after its enchanted permanent leaves

Schema 178 adds `ObjectRef::AuraAfterEnchantedLeft { aura, host }`. Returning an
Aura after its enchanted permanent dies names the Aura's first graveyard
incarnation allowed by CR 400.7f, separately from the host's original battlefield
object. It cannot use the Aura's old battlefield `Source`, follow arbitrary later
moves, or rediscover a different attachment.

Lowering grants this reference only in the scope of an actual enchanted-permanent
battlefield departure. A replacement proposal does not establish that scope. The
Cantrip backend requires the `found_aura_after_enchanted_left` query; older pinned
engines receive an explicit capability gap. Step-delayed abilities must capture
this graveyard identity when created; multiple capture roles remain a separate
generator task rather than a reason to follow an object at resolution.

Four English productions also preserve the printed "from your graveyard"
restriction, before offering the optional return. This affects the six Genjus,
Reins of the Vinesteed, and Screams from Within. The native identity query uses the
Aura owner's graveyard; this separate predicate checks the ability controller's
graveyard, which matters when an opponent controls the Aura.

The new native Snow mana field also requires ordinary generated mana-cost
literals to include `snow: 0`. Capability detection selects the correct native
struct shape and counts Snow symbols directly; hybrid Phyrexian symbols remain
unsupported. Existing u32 mana fields no longer inherit an artificial 255 cap;
X retains its native byte bound and generic addition remains checked.

The parser fixture covers all 23 affected cards. Full source regeneration changes
exactly the eight restricted cards. Comparing all 33,529 unified records with
schema 177 identifies exactly 25 repaired return operands across 23 cards and
eight explicit graveyard predicates; every other field is identical after schema
headers are excluded. All 37 guarded grammar-usage evidence hashes are unchanged.
The full Rust workspace passes 1,515 tests, with zero failures and four ignored;
the API-only library passes 57 tests, and the complete Go suite passes.

The missing English qualifier is reported upstream as
[MTGish issue 196](https://github.com/i5jb/mtgish/issues/196).

The raw generated Puppet Master module executed three retained checked-engine
scenarios successfully: failed creature return, successful return with payment
declined, and successful return with payment returning the Aura. This runtime
probe is separate from final card review and publication. Its local commit is
`8d6d1d466` above composed engine `95f72aaabb661043779cf4c0804200f4dbed6662`.

Logs, serialized comparisons, before failures and fixtures are preserved in
`~/card-lane/pipeline-unlimited-20261103/integration/aura-source-identity-proof/`.
