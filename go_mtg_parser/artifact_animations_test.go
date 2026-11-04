package main

import (
	"os"
	"path/filepath"
	"regexp"
	"strconv"
	"strings"
	"testing"
)

// CR 205.1b: named artifact-creature animations replace creature types;
// bare animations and explicit retention clauses preserve them. Compare the
// entire source record so colors, PT, duration, keywords and ordering survive.
func TestArtifactAnimationCreatureTypes(t *testing.T) {
	testSourceReferenceFixtures(t, "artifact_animations.ronl", "restless_cottage.ronl", "retained_animations.ronl")
}

func TestDiscardedCardReturnReference(t *testing.T) {
	testSourceReferenceFixtures(t, "discard_return.ronl")
}

func TestTapSubtypeCostsAndMaxSpeedReference(t *testing.T) {
	testSourceReferenceFixtures(t, "reference_costs.ronl")
}

func TestJohtullWurmBlockersBeyondTheFirst(t *testing.T) {
	testSourceReferenceFixtures(t, "johtull_wurm.ronl")
}

func testSourceReferenceFixtures(t *testing.T, fixtureNames ...string) {
	t.Helper()
	englishFiles, _ := filepath.Glob("../grammars/english_grammar*.json5")
	mtgishFiles, _ := filepath.Glob("../grammars/mtgish_grammar*.json5")
	english := ReadGrammarJsoncFiles(englishFiles)
	mtgish := ReadGrammarJsoncFiles(mtgishFiles)
	englishTree := CreateTokenReTriesFromTemplatesAndResults(english)
	shortNames := ReadJsoncMap[string]("../grammars/short_names.json5")
	previous := findReplaceReasons
	findReplaceReasons = ReadJsoncArray[FindReplaceEntry]("../grammars/find_replace.json5")
	defer func() { findReplaceReasons = previous }()
	cards := make(map[string]MtgJsonEntry)
	var quotedNames []string
	for _, file := range []string{"oracle", "additional_cards", "tokens", "dungeons"} {
		for _, card := range ReadJsoncArray[MtgJsonEntry]("../data/" + file + ".json") {
			cards[card.Name] = card
			for _, face := range card.Cards {
				if face.Type != "Stickers" {
					quotedNames = append(quotedNames, strconv.Quote(face.Name))
				}
			}
		}
	}
	mtgish["QUOTED_NAME"] = LoadTemplatesFor(makeSelfPairs(quotedNames))
	mtgishTree := CreateTokenReTriesFromTemplatesAndResults(mtgish)
	var fixture []byte
	for _, name := range fixtureNames {
		data, err := os.ReadFile(filepath.Join("../rust_syntax/normalize/tests/fixtures", name))
		if err != nil {
			t.Fatal(err)
		}
		fixture = append(fixture, data...)
	}
	namePattern := regexp.MustCompile(`Name: "([^"]+)"`)
	for _, expected := range strings.Split(strings.TrimSpace(string(fixture)), "\n") {
		name := namePattern.FindStringSubmatch(expected)[1]
		if _, ok := cards[name]; !ok {
			for full, card := range cards {
				if len(card.Cards) > 0 && card.Cards[0].Name == name {
					name = full
					break
				}
			}
		}
		t.Run(name, func(t *testing.T) {
			card, ok := cards[name]
			if !ok {
				t.Fatalf("missing corpus card %q", name)
			}
			parsed, err := ParseCard(english, englishTree, mtgish, mtgishTree, shortNames, card)
			if err != nil {
				t.Fatal(err)
			}
			if parsed.MtgishText != expected {
				t.Errorf("source reference changed:\n got: %s\nwant: %s", parsed.MtgishText, expected)
			}
		})
	}
}
