package main

import (
	"crypto/sha256"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"testing"
)

func TestGrammarAuditRecognizesAnimationInstructions(t *testing.T) {
	tests := []struct{ input, output string }{
		{"you gain {{Digits}} life. If {{Condition}}, it becomes {{a}} {{PT}} {{Zombie}} creature with {{Hasable}} in addition to its other types until end of turn", "ActionList([GainLife({{Digits[0]}}), If({{Condition[0]}}, [CreatePermanentLayerEffectUntil(ThisPermanent, [SetPT({{PT[0]}}), AddCreatureType({{Zombie[0]}}), AddCardtype(Creature), AddAbility([{{Hasable[0]}}])], UntilEndOfTurn)])])"},
		{"gain control of target {{Creature}} until end of turn. Untap that creature. Until end of turn, it has base power and toughness {{PT}} and gains {{Hasable}}", "Targeted([TargetPermanent({{Creature[0]}})], ActionList([GainControlOfPermanentUntil(Ref_TargetPermanent, UntilEndOfTurn), UntapPermanent(Ref_TargetPermanent), CreatePermanentLayerEffectUntil(Ref_TargetPermanent, [SetPT({{PT[0]}}), AddAbility([{{Hasable[0]}}])], UntilEndOfTurn)]))"},
		{"put {{Number}} {{CounterType}} counters on a land. That land becomes {{a}} {{PT}} {{Zombie}} creature in addition to its other types. It gains {{Hasable}} until your next turn", "ActionList([PutCounters([NumberCountersOfTypeOnPermanent({{Number[0]}}, {{CounterType[0]}}, Ref_TargetPermanent)]), CreatePermanentLayerEffect(Ref_TargetPermanent, [SetPT({{PT[0]}}), AddCreatureType({{Zombie[0]}}), AddCardtype(Creature)]), CreatePermanentLayerEffectUntil(Ref_TargetPermanent, [AddAbility([{{Hasable[0]}}])], UntilPlayersNextTurn(You))])"},
	}
	for _, test := range tests {
		if findings := auditTemplate("ACTION", test.input, test.output, nil); len(findings) != 0 {
			t.Errorf("already separated instructions flagged: %#v", findings)
		}
	}
	// The same animation split over two instructions must still be flagged.
	input := "It becomes {{a}} {{PT}} {{Zombie}} creature. It gains {{Hasable}} until end of turn"
	output := "CreatePermanentLayerEffect(ThisPermanent, [SetPT({{PT[0]}}), AddCreatureType({{Zombie[0]}}), AddAbility([{{Hasable[0]}}])])"
	if findings := auditTemplate("ACTION", input, output, nil); len(findings) != 1 || findings[0].Kind != "merged_instruction_candidate" {
		t.Fatalf("actual merge lost: %#v", findings)
	}
}

func TestGrammarAuditFreshScalarEquivalence(t *testing.T) {
	finding := AuditFinding{ID: "scalar", Kind: "unused_semantic_capture", Capture: "Number[1]", Output: "DrawNumberCards({{Number[0]}})", Usage: []AuditUse{{Card: "Example", Captures: map[string][]string{"Number": {"two", "two"}}}}}
	report := GrammarAudit{ParsedCards: 1, Findings: []AuditFinding{finding}}
	if err := ReviewGrammarAudit(&report, "", true); err != nil {
		t.Fatal(err)
	}
	if len(report.Findings) != 0 || len(report.Reviewed) != 1 {
		t.Fatalf("equal values not recognized: %#v", report)
	}
	finding.Usage[0].Captures["Number"][1] = "three"
	report = GrammarAudit{ParsedCards: 1, Findings: []AuditFinding{finding}}
	if err := ReviewGrammarAudit(&report, "", true); err != nil {
		t.Fatal(err)
	}
	if len(report.Findings) != 1 {
		t.Fatal("different counts were hidden")
	}
	finding.Usage[0].Captures["Number"][1] = "two"
	report = GrammarAudit{ParsedCards: 1, ParseFailures: 1, Findings: []AuditFinding{finding}}
	if err := ReviewGrammarAudit(&report, "", true); err != nil {
		t.Fatal(err)
	}
	if len(report.Findings) != 1 {
		t.Fatal("failed parse used as evidence")
	}
	report = GrammarAudit{Findings: []AuditFinding{finding}}
	if err := ReviewGrammarAudit(&report, "", false); err != nil {
		t.Fatal(err)
	}
	if len(report.Findings) != 1 {
		t.Fatal("static audit treated saved uses as fresh evidence")
	}
}

func TestGrammarAuditReviewInvalidatesOnChangedInputsTemplatesAndUses(t *testing.T) {
	root := t.TempDir()
	corpus := filepath.Join(root, "corpus.json")
	if err := os.WriteFile(corpus, []byte("original card and cost"), 0600); err != nil {
		t.Fatal(err)
	}
	finding := AuditFinding{ID: "exact-template-id", Kind: "unused_semantic_capture", Capture: "GameNumber(ThatCreature)[0]", Usage: []AuditUse{{Card: "Example", MatchedText: "its power", Captures: map[string][]string{"GameNumber(ThatCreature)": {"its power"}}}}}
	evidence, err := auditEvidence(finding.Usage)
	if err != nil {
		t.Fatal(err)
	}
	review := AuditReview{Version: 1, InputRoot: ".", Inputs: map[string]string{"corpus.json": fmt.Sprintf("%x", sha256.Sum256([]byte("original card and cost")))}, Reviewed: []AuditReviewEntry{{ID: finding.ID, Disposition: "reviewed_implicit_context", Reason: "power is already represented", Evidence: evidence}}}
	reviewPath := filepath.Join(root, "review.json")
	encoded, err := json.Marshal(review)
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(reviewPath, encoded, 0600); err != nil {
		t.Fatal(err)
	}
	apply := func(f AuditFinding, withUsage bool) GrammarAudit {
		report := GrammarAudit{ParsedCards: 1, ParserInputs: review.Inputs, Findings: []AuditFinding{f}}
		if err := ReviewGrammarAudit(&report, reviewPath, withUsage); err != nil {
			t.Fatal(err)
		}
		return report
	}
	for _, withUsage := range []bool{false, true} {
		if report := apply(finding, withUsage); len(report.Findings) != 0 || len(report.Reviewed) != 1 {
			t.Fatalf("review not applied: %#v", report)
		}
	}
	altered := finding
	altered.ID = "changed-template-id"
	if report := apply(altered, false); len(report.Findings) != 1 {
		t.Fatal("changed template hidden")
	}
	altered = finding
	altered.Usage = append(append([]AuditUse{}, finding.Usage...), AuditUse{Card: "New card"})
	if report := apply(altered, true); len(report.Findings) != 1 {
		t.Fatal("new matched use hidden")
	}
	differentInvocation := GrammarAudit{ParsedCards: 1, ParserInputs: map[string]string{"unreviewed.json": "digest"}, Findings: []AuditFinding{finding}}
	if err := ReviewGrammarAudit(&differentInvocation, reviewPath, true); err != nil {
		t.Fatal(err)
	}
	if len(differentInvocation.Findings) != 1 {
		t.Fatal("unreviewed input file was trusted")
	}
	if err := os.WriteFile(corpus, []byte("changed cost"), 0600); err != nil {
		t.Fatal(err)
	}
	for _, withUsage := range []bool{false, true} {
		if report := apply(finding, withUsage); len(report.Findings) != 1 {
			t.Fatal("changed card/cost snapshot hidden")
		}
	}
}

func TestGrammarAuditReviewRequiresFingerprints(t *testing.T) {
	path := filepath.Join(t.TempDir(), "review.json")
	review := AuditReview{Version: 1, Reviewed: []AuditReviewEntry{{ID: "id", Disposition: "reviewed_implicit_context", Reason: "review", Evidence: fmt.Sprintf("%064d", 0)}}}
	data, _ := json.Marshal(review)
	if err := os.WriteFile(path, data, 0600); err != nil {
		t.Fatal(err)
	}
	report := GrammarAudit{Findings: []AuditFinding{{ID: "id"}}}
	if err := ReviewGrammarAudit(&report, path, false); err != nil {
		t.Fatal(err)
	}
	if len(report.Findings) != 1 {
		t.Fatal("unverified review applied")
	}
}

func TestGrammarAuditReviewedCorpusEvidence(t *testing.T) {
	// Reuse the already validated derivation evidence. Detector edits do not
	// change parsing, so this check needs no second full parser pass.
	data, err := os.ReadFile("../data/grammar_audit.json")
	if err != nil {
		t.Fatal(err)
	}
	var saved GrammarAudit
	if err := json.Unmarshal(data, &saved); err != nil {
		t.Fatal(err)
	}
	var review AuditReview
	reviewBytes, err := os.ReadFile("../data/grammar_audit_review.json")
	if err != nil {
		t.Fatal(err)
	}
	if err := json.Unmarshal(reviewBytes, &review); err != nil {
		t.Fatal(err)
	}
	report := GrammarAudit{ParsedCards: saved.ParsedCards, ParserInputs: review.Inputs, Sources: review.Sources, SourceOrder: review.SourceOrder}
	all := append(saved.Findings, saved.Reviewed...)
	automatic := 0
	for _, f := range all {
		// Re-evaluate instruction recognition with the revised detector.
		stillFlagged := false
		for _, current := range auditTemplate(f.Rule, f.Input, f.Output, nil) {
			if current.Kind == f.Kind && current.Capture == f.Capture {
				stillFlagged = true
			}
		}
		if !stillFlagged {
			continue
		}
		f.ReviewReason = ""
		report.Findings = append(report.Findings, f)
		if equivalentScalarCapture(f) {
			automatic++
		}
	}
	if len(report.Findings) != 182 || automatic != 145 {
		t.Fatalf("expected 145 scalar cases plus 37 context cases, got %d total and %d scalar", len(report.Findings), automatic)
	}
	if err := ReviewGrammarAudit(&report, "../data/grammar_audit_review.json", true); err != nil {
		t.Fatal(err)
	}
	if len(report.Findings) != 0 || len(report.Reviewed) != 182 {
		t.Fatalf("%d unresolved reviewed findings; review status: %s", len(report.Findings), report.ReviewStatus)
	}
}

func TestGrammarAuditEvidenceCanonicalJSON(t *testing.T) {
	uses := []AuditUse{{Card: "Example & <名>", MatchedText: "draw two", Captures: map[string][]string{"Number": {"two", "two"}}}}
	expected := `[{"captures":{"Number":["two","two"]},"card":"Example & <名>","matched_text":"draw two"}]`
	digest, err := auditEvidence(uses)
	if err != nil {
		t.Fatal(err)
	}
	if digest != fmt.Sprintf("%x", sha256.Sum256([]byte(expected))) {
		t.Fatal("review fingerprint is not canonical JSON")
	}
}
