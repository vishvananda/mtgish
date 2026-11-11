# Vampiric Embrace: capture the enchanted damage dealer

The counter belongs to the enchanted damage dealer named by the death trigger,
not to the creature that died. The old Source template bound “that creature” to
the victim. A live `AttachedTo(Source)` read during resolution is also insufficient:
the Aura can move after the ability triggers. The repair captures the host's exact
battlefield incarnation when the event matches and retains the Aura as the ability
source, with its own controller and trigger order.

This completes the outstanding Vampiric decision in
[the full reference review](reference-decisions-20261111.md). That earlier batch
was committed at `ab718c9` under schema 189; this follow-up adds schema 190.

## Explicit event contract

`EventSpec::WithOtherObject { event, object }` wraps an ordinary matcher.

- Match `event` and evaluate `object` once in its event view. Battlefield departures
  use the pre-departure view, including attachments before simultaneous departures.
- The inner match's ordinary bindings are available to the capture expression;
  the new secondary binding is not. Self-referential capture is invalid.
- Export the retained identity as the existing `ObjectRef::EventOther`. Preserve
  the ordinary `EventSubject`, source, ability controller and all other roles.
- Do not re-evaluate attachment or controller queries at resolution. Do not follow
  a captured permanent across subsequent zone changes or a leave-and-return cycle.
  Missing objects remain missing; retaining an identity does not make it a target.
- Reject an inner matcher that already exports a secondary object. Alternative
  events export the role only when every branch supplies it.

The Source form is `WithOtherPermanent(trigger, permanent)`. The printed Aura
rule now uses it to capture `HostPermanent` and puts the counter on
`Trigger_ThatOtherPermanent`. The original victim filter and damage-history test
remain unchanged. The grammar correction is scoped to this counter sentence;
other death follow-ups must not inherit this exceptional pronoun interpretation.

No new object-reference variant, binding-index vector or global snapshot is
introduced. Ordinary matchers gain no allocation or extra capture work. Only this
new wrapper allocates its inner matcher. Existing hot-node layout budgets pass without changing their limits.

## Evidence and boundary cases

The independent [XMage matcher at a fixed revision](https://github.com/magefree/mage/blob/87ab0a3fcc65f09d05b19af0d75efeedabca88f8/Mage/src/main/java/mage/abilities/common/DealtDamageAttachedAndDiedTriggeredAbility.java)
captures a `FixedTarget(creatureMOR)` while matching this event; Vampiric Embrace
selects that permanent pointer. This corroborates the identity requirement; it is
implementation evidence, not a new official card-specific ruling. See also the
[Comprehensive Rules](https://media.wizards.com/2026/downloads/MagicCompRules%2020261106.pdf)
for departure-trigger matching (603.10) and new-object identities (400.7).

The declared contract keeps the original host when the Aura attaches elsewhere
before resolution, still identifies that host when the Aura subsequently leaves,
and does not substitute a returned new incarnation when the host leaves and
returns. A graveyard replacement prevents a death event from matching. If host
and victim die together, the capture names the original battlefield host; it
cannot receive counters after leaving. The AST tests check the binding contract
and exact complete program; they do not claim these scenarios executed in Cantrip.

Cantrip generation explicitly reports `trigger-time secondary object capture`
as a capability gap. It does not silently use a live host, redirect the counter
to the victim, grant the trigger to the host, or claim new engine implementation.

An additional generic control wraps a damage event: its explicit secondary
capture must remain `EventOther`, distinct from the old damage-recipient alias.
The Source alias dispatcher gives an explicit secondary binding precedence, and
legacy damage-role/ambiguity inspection transparently traverses the wrapper.
The damage and scope families are rerun for this boundary; the complete corpus
comparison independently checks that existing card programs remain unchanged.

## Persistence and adoption

The event variant is appended, retaining existing enum order. Schema 189's frozen
contract stays immutable. Its typed migration to 190 changes version fields only,
with identity binding origins; it does not silently correct saved card meanings.
A schema 189 input claiming the new event syntax is rejected.

Oracle Grammar needs the compiled model provider and lockfile advanced for schema
190, separately from its reference lock. Consume a newly generated reference
artifact and verify its hash before recounting. The original worker report, gold,
source, reference/model pins and concurrent edits were not changed here. A local
handoff copy is placed under its ignored `.cache/` directory.

## Additional reference comments checked

The migrated report remains the same 36-face / 13-group packet at the reviewed
reference pin. The worker has since committed its own grammar/binding corrections
(`f811024`, `84f8946`); these are separate from new reference findings. The latest
upstream issue comments were checked read-only. No additional confirmed correction
beyond the existing packet was found. The latest external comment concerns Yenna's
self-exclusion assumption, already represented by our explicit “another” contract;
no speculative reference change was made.

Final validation and upstream receipts accompany this document in
`vampiric-embrace-capture-20261111.proof.json`. Earlier batch validation remains in
`reference-decisions-20261111.proof.json`; its schema 189 totals are historical.

Validation is complete: 1,958 passing Rust tests across the workspace and focused
follow-ups; full uncached Go suite; all 33,529 browser block/binding/Rust-Wasm
checks; 28 actual workspaces with exact IR and no JavaScript errors. The frozen
189 Kogla fixture now uses the explicit migration reader in its test; its saved
program is unchanged. Browser glue uses `wasm-bindgen --target no-modules` for the
classic worker. One Rust/Wasm build was used for this follow-up.
