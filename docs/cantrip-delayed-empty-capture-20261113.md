# Delayed abilities with empty captures

Necropotence unconditionally creates its end-step delayed triggered ability after
attempting to exile the top library card (CR 603.7, 609.3). An empty library does
not cancel that separate instruction. The future ability can trigger and resolve
without moving a card. The emitter previously suppressed registration whenever
a single captured object list was empty and the future effect moved cards to
exile or hand. That guard was unrelated to any condition in the semantic model.

Removed that implicit guard. Explicit surrounding instructions, delayed start
conditions, expiration, and intervening-if checks retain their existing behavior.
No parser, public AST, or native engine interface changed.

The production-profile regression lowers actual Necropotence and verifies that
creation is unconditional. A control wraps the same creation instruction in a
real condition and verifies that the generated call remains conditional.

Actual generated definitions execute four native scenarios: empty library,
ordinary discard and delayed return, only the controller's draw step skipped,
and two independent activations returning distinct cards. All four pass. The
empty-library scenario fails against the original output: `Delayed.Count` is
zero instead of one. Durable scenarios are in
`rust_syntax/normalize/tests/fixtures/early_delayed_creation_native/`; install them
under a Cantrip checkout's scenarios directory with generated Necropotence and
run `cargo run --profile cards -p cantrip-script --bin corpus -- --card Necropotence`.
Proof logs: `/mnt/vizier/early-set-campaign-plan-20261113/necropotence-{baseline,fixed}.log`.
