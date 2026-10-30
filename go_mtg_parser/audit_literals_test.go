package main

import (
	"fmt"
	"os"
	"path/filepath"
	"testing"
)

func writeLiteralGrammar(t *testing.T, body string) []string {
	t.Helper()
	filename := filepath.Join(t.TempDir(), "english_grammar.json5")
	if err := os.WriteFile(filename, []byte(body), 0o644); err != nil {
		t.Fatal(err)
	}
	return []string{filename}
}

func TestLiteralRulesFindWrongOutputs(t *testing.T) {
	findings := AuditGrammarLiterals(writeLiteralGrammar(t, `{"{{Actions}}": {
		"until end of turn": "UntilEndOfCombat",
		"until your next turn": "UntilEndOfTurn",
		"at the beginning of your next upkeep": "AtTheBeginningOfTheNextEndStep",
		"the greatest toughness": "TheGreatestPowerAmongPermanents",
		"until end of combat": "UntilEndOfCombat",
	}}`))
	expected := map[string]bool{
		"until end of turn":                    false,
		"until your next turn":                 false,
		"at the beginning of your next upkeep": false,
		"the greatest toughness":               false,
	}
	for _, finding := range findings {
		if _, ok := expected[finding.Template]; !ok {
			t.Fatalf("unexpected finding %+v", finding)
		}
		expected[finding.Template] = true
	}
	for template, found := range expected {
		if !found {
			t.Errorf("no finding for %q", template)
		}
	}
}

func TestLiteralMiningFindsOutlierTemplate(t *testing.T) {
	body := `{"{{Actions}}": {`
	for _, word := range []string{"alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf", "hotel", "india", "juliet", "kilo", "lima", "mike", "november", "oscar"} {
		body += `"` + word + ` regenerate": "RegeneratePermanent",`
	}
	// The pair only stands out against a background of unrelated templates.
	for i := 0; i < 200; i++ {
		body += fmt.Sprintf(`"plain %d": "Plain",`, i)
	}
	body += `"sierra": "RegeneratePermanent"}}`
	findings := AuditGrammarLiterals(writeLiteralGrammar(t, body))
	for _, finding := range findings {
		if finding.Kind == "mined_token" && finding.Token == "RegeneratePermanent" && finding.Word == "regenerate" && finding.Template == "sierra" {
			return
		}
	}
	t.Fatalf("outlier not mined: %+v", findings)
}

func TestGrammarLiteralsAreReviewed(t *testing.T) {
	filenames, err := filepath.Glob("../grammars/english_grammar*.json5")
	if err != nil {
		t.Fatal(err)
	}
	unreviewed, err := UnreviewedLiterals(AuditGrammarLiterals(filenames), "../data/grammar_literal_review.json")
	if err != nil {
		t.Fatal(err)
	}
	for _, finding := range unreviewed {
		t.Errorf("unreviewed %s: %q has %s without %s\n  %s", finding.Kind, finding.Template, finding.Token, finding.Word, finding.Result)
	}
}
