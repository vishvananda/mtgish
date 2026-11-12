package main

import (
	"path/filepath"
	"strconv"
	"strings"
	"testing"
)

func TestSelfFrameReferentsHaveOneProducerForm(t *testing.T) {
	testSourceReferenceFixtures(t, "reference_self_frames_20261112.ronl")
}

func TestOtherEnteringCreatureRetainsItsOwnSpellExclusion(t *testing.T) {
	englishFiles, _ := filepath.Glob("../grammars/english_grammar*.json5")
	mtgishFiles, _ := filepath.Glob("../grammars/mtgish_grammar*.json5")
	english := ReadGrammarJsoncFiles(englishFiles)
	mtgish := ReadGrammarJsoncFiles(mtgishFiles)
	shorts := ReadJsoncMap[string]("../grammars/short_names.json5")
	previous := findReplaceReasons
	findReplaceReasons = ReadJsoncArray[FindReplaceEntry]("../grammars/find_replace.json5")
	defer func() { findReplaceReasons = previous }()
	for _, card := range ReadJsoncArray[MtgJsonEntry]("../data/oracle.json") {
		if card.Name != "Storm Entity" {
			continue
		}
		// A grammar composition control, not a new Oracle/reference record.
		card.Cards[0].Text = "Other creatures enter with an additional +1/+1 counter on them for each other spell cast this turn."
		mtgish["QUOTED_NAME"] = LoadTemplatesFor(makeSelfPairs([]string{strconv.Quote(card.Name)}))
		parsed, err := ParseCard(english, CreateTokenReTriesFromTemplatesAndResults(english), mtgish, CreateTokenReTriesFromTemplatesAndResults(mtgish), shorts, card)
		if err != nil {
			t.Fatal(err)
		}
		if !strings.Contains(parsed.MtgishText, "NumSpellsCastThisTurn(Other(ThatEnteringPermanent))") {
			t.Fatalf("the other entrant is not the source: %s", parsed.MtgishText)
		}
		return
	}
	t.Fatal("Storm Entity fixture missing")
}
