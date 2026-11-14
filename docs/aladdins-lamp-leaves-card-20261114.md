# Aladdin's Lamp retains the chosen card in place

Aladdin's Lamp moves all but one of the inspected cards to the bottom of the
library in a random order, then draws. It does not put the retained card on top.
The English template inserted `PutAGenericCardOnTopOfLibrary`, and our lowering
faithfully emitted an additional top reorder. Selecting a card to leave is a
different operation from putting/reordering it. The latter also has separate
revealed-top handling under CR701.20d; no runtime visibility witness is claimed.

The correction uses `LeaveAGenericCardOnTopOfLibrary`, appended to both the
ordinary inspection and draw-replacement Source action enums. Existing Source
bincode discriminants remain unchanged. The new action selects one card as
possible, excludes its Selected receipt from the later remaining-card step, and
does not emit a move/reorder or fabricate a transition receipt. Existing
`PutAGenericCardOnTopOfLibrary` retains its explicit-reorder behavior. Existing
portable IR represents the corrected program; no AST schema change is needed.

The subsequent action remains a real Draw. Moving the retained card into hand
would skip draw rules and other waiting draw replacements. Capturing the
announced X and distinguishing multiple pending replacements are separate
Cantrip engine/generator prerequisites, not reasons to alter the reference.

Reports: [MTGish #290](https://github.com/i5jb/mtgish/issues/290) and
[Oracle Grammar #26](https://github.com/vishvananda/oracle-grammar/issues/26).

The full parser consumed the same frozen33,529-record Oracle, supplemental,
token and dungeon input. A complete-record multiset comparison changes Lamp
alone. RON, tagged JSON and pretty RON change only its action; untouched export
bytes are preserved. All182 existing audit findings and matched evidence are
identical (comparison by finding ID, since iteration order may vary). Grammar
fingerprints advance without adding capture-audit exceptions. One newly mined
literal correlation is explicitly reviewed: the three-card instruction that
puts one into hand, one on top and one on bottom correctly uses a put-on-top
operation without saying “rest.” The detector is unchanged.

The active Source digest advances, and approved changed records increases7→8.
Oracle input, published Oracle Grammar bundle/model pins and population remain
fixed. The other worker should apply this approved correction through the
existing MTGish adapter update process, preserving its pinned export in place.
No unrelated Oracle Grammar working files were edited.

Regressions compare the whole Source card, validate the lowered typed program,
assert select-one/random-bottom-rest/real-draw without a top reorder, and retain
a positive control where an explicit put-on-top still emits that reorder.
The original generated Native module compiled and passed one forced X=2 case;
that narrow result did not test the corrected distinction. Final corrected
Native proof and complete card validation remain part of card integration.
