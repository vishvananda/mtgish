# Creation text for lasting rules

Island Sanctuary and Tower of Coireall exposed the same engine authority gap:
a resolved rule read a source object's live words, or the targeted creature's
words, instead of the text which created that rule. The core now records Words
for LastingRule and BlockRestriction in the existing ContinuousEffect field.
No state field, callback signature, numeric width, or new allocation is added.

LastingRule callbacks read the creation text through WithWords in both open-set
queries and the attack-player restriction query. The set remains open: current
characteristics decide membership. Exhaustion, Falter and other existing lasting
rules keep their existing interface. For a LastingBlockRule the callback's source
argument explicitly names the affected creature. Its rule words are supplied
under that argument, while its identity and characteristics remain those of the
affected creature. Changing the Tower before activation therefore changes Wall
to Elf for that restriction; changing the source afterwards or changing text on
the affected creature does not change the already-created restriction. Tower of
Coireall and Joven's Tools are printed consumers of this blocking interface.

Core regressions fail before the change and pass afterwards, testing empty and
nonempty captured words, unrelated changes on the affected creature, the
unaffected second attacker, open membership, and unchanged live source words.
The broader focused text suite passes 44 tests. The same retained native card
scenarios fail 2/pass 2 on the old readers and pass all four on the repaired
readers, covering actual Magical Hack and Artificial Evolution interactions.

Runtime evidence is retained in
rust_syntax/normalize/tests/fixtures/early_lasting_words_native/. Install its
island_sanctuary.rs and tower_of_coireall.rs and .cantrip files under
crates/cantrip-script/tests/fixtures/early-lasting-words/, and its harness as
crates/cantrip-script/tests/early_lasting_words_regressions.rs, then run
cargo test --profile cards -p cantrip-script --test early_lasting_words_regressions.
These provisional card modules retain the stopped workers' behavior; their
Oracle constants are replaced only with generated printed characteristics to
register them outside the production card table. They are test fixtures, not
production card registrations or approvals.

Spoils of War's separate client blocker used the generic caption “The player
you choose Define X”. The existing player-choice prompt now says to choose a
player to determine X and shows the source's Oracle text. For Spoils that text
identifies artifact/creature cards in the selected opponent's graveyard. This
keeps other printed definitions valid without hardcoding a particular card into
the generic XRule interface. A forced browser fixture checks that definition
and the selected seat's submitted answer. No protocol field or engine X rule
changes. The gallery's test output now includes the chosen seat so the assertion
checks the payload rather than just its message type.

The targeted real-browser test passes, including PlayerChosenAnswer seat 3.
Three interleaved benchmark pairs all finish 128 games and allocate 29.34 KB per
decision on both versions. Median engine time is 57.51 µs before and 53.75 µs
after under changing shared-machine load; this is not evidence of a speedup.
The adapter adds no stored state or heap allocation. Full-suite validation is
recorded separately before promoting this foundation. Ordinary independent card review/confirmation is still
required. Tetravus's linked-created-token gap is separate (Oracle Grammar #20).
