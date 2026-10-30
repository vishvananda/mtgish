package main

import "testing"

func TestGrammarAuditMissingAbilityOccurrence(t *testing.T) {
	input := "Target {{Permanent}} gets {{PTModX}} and gains {{Hasable}} until end of turn"
	broken := "CreatePermanentLayerEffectUntil(Ref_TargetPermanent, [AdjustPTX({{PTModX[0]}}, {{X}})], UntilEndOfTurn)"
	findings := auditTemplate("ACTION", input, broken, nil)
	if len(findings) != 1 || findings[0].Capture != "Hasable[0]" {
		t.Fatalf("missing ability not found: %#v", findings)
	}
	fixed := "CreatePermanentLayerEffectUntil(Ref_TargetPermanent, [AdjustPTX({{PTModX[0]}}, {{X}}), AddAbility([{{Hasable[0]}}])], UntilEndOfTurn)"
	if got := auditTemplate("ACTION", input, fixed, nil); len(got) != 0 {
		t.Fatalf("fixed template flagged: %#v", got)
	}
	findings = auditTemplate("ACTION", "gains {{Hasable}} and {{Hasable}}", "AddAbility([{{Hasable[0]}}])", nil)
	if len(findings) != 1 || findings[0].Capture != "Hasable[1]" {
		t.Fatalf("repeated occurrence not distinguished: %#v", findings)
	}
}

func TestGrammarAuditInstructionMerge(t *testing.T) {
	input := "{{ThisPermanent}} becomes a Human Cleric. It gets {{PTMod}} and gains {{Hasable}}"
	broken := "CreatePermanentLayerEffect(ThisPermanent, [SetCreatureTypes([Human, Cleric]), AdjustPT({{PTMod[0]}}), AddAbility([{{Hasable[0]}}])])"
	findings := auditTemplate("ACTION", input, broken, nil)
	if len(findings) != 1 || findings[0].Kind != "merged_instruction_candidate" {
		t.Fatalf("merge not found: %#v", findings)
	}
	fixed := "ActionList([CreatePermanentLayerEffect(ThisPermanent, [SetCreatureTypes([Human, Cleric])]), CreatePermanentLayerEffect(ThisPermanent, [AdjustPT({{PTMod[0]}}), AddAbility([{{Hasable[0]}}])])])"
	if got := auditTemplate("ACTION", input, fixed, nil); len(got) != 0 {
		t.Fatalf("separate actions flagged: %#v", got)
	}
	input = "{{ThisPermanent}} becomes a Human Cleric and gets {{PTMod}} and gains {{Hasable}}"
	if got := auditTemplate("ACTION", input, broken, nil); len(got) != 0 {
		t.Fatalf("single instruction flagged: %#v", got)
	}
}

func TestGrammarAuditIgnoresGrantedAbilityPunctuation(t *testing.T) {
	input := "This creature gains \"This creature gets +1/+1. It gains flying.\""
	output := "CreatePermanentLayerEffect(ThisPermanent, [AdjustPT(1, 1), AddAbility([Flying])])"
	if got := auditTemplate("ACTION", input, output, nil); len(got) != 0 {
		t.Fatalf("inner instructions confused with outer: %#v", got)
	}
}

func TestGrammarAuditParameterizedCaptureAndConstant(t *testing.T) {
	if got := auditTemplate("ACTION", "draw {{GameNumber(~Permanent)}} cards", "DrawNumberCards({{GameNumber(~Permanent=ThisPermanent)[0]}})", nil); len(got) != 0 {
		t.Fatalf("parameterized reference not recognized: %#v", got)
	}
	grammar := TemplatesAndResults{"Number": LoadTemplatesFor(map[string]string{"one": "1"})}
	if got := auditTemplate("ACTION", "draw {{Number}} cards", "DrawNumberCards(1)", grammar); len(got) != 0 {
		t.Fatalf("constant capture not recognized: %#v", got)
	}
}

func TestGrammarAuditUsageRecordsRepeatedCaptureValues(t *testing.T) {
	input := "gains {{Hasable}} and {{Hasable}}"
	output := "AddAbility([{{Hasable[0]}}])"
	grammar := TemplatesAndResults{
		"ACTION":  LoadTemplatesFor(map[string]string{input: output}),
		"Hasable": LoadTemplatesFor(map[string]string{"flying": "Flying", "trample": "Trample"}),
	}
	text := "gains flying and trample"
	tree := CreateTokenReTriesFromTemplatesAndResults(grammar)
	breadcrumb, err := TokenReTriesBestMatch(grammar, tree, "ACTION", text, false)
	if err != nil {
		t.Fatal(err)
	}
	u := NewAuditUsage(GrammarAudit{Findings: auditTemplate("ACTION", input, output, grammar)})
	u.Record(grammar, CardInfo{Name: "Audit example", EnglishTokens: GetTokens(text), EnglishBreadcrumbs: breadcrumb})
	use := u.Report.Findings[0].Usage[0]
	if use.Card != "Audit example" || len(use.Captures["Hasable"]) != 2 || use.Captures["Hasable"][1] != "trample" {
		t.Fatalf("capture occurrence evidence lost: %#v", use)
	}
}
