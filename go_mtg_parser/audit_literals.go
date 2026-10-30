package main

import (
	"crypto/sha256"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"regexp"
	"sort"
	"strings"
)

// The capture audit cannot see a wrong literal on the output side of a template.
// This audit compares each template's English words with its output tokens: rules
// state the pairings that must hold, and mining finds the ones that hold for nearly
// every other template. Hits are candidates; reviewed ones are pinned to the exact
// template, so changing a template reopens its review.
type LiteralFinding struct {
	ID       string `json:"id"`
	Kind     string `json:"kind"`
	Rule     string `json:"rule"`
	Token    string `json:"token"`
	Word     string `json:"word"`
	Support  string `json:"support,omitempty"`
	Template string `json:"template"`
	Result   string `json:"result"`
	Reason   string `json:"reason,omitempty"`
}

type LiteralReview struct {
	Version  int              `json:"version"`
	Scope    string           `json:"scope"`
	Reviewed []LiteralFinding `json:"reviewed"`
}

type literalRule struct {
	token   *regexp.Regexp
	english *regexp.Regexp
}

func newLiteralRule(token, english string) literalRule {
	return literalRule{regexp.MustCompile(token), regexp.MustCompile("(?i)" + english)}
}

// An output token requires the English, or the English requires the output token.
var literalTokenRules = []literalRule{
	newLiteralRule(`UntilEndOfCombat`, `combat`),
	newLiteralRule(`UntilPlayersNextTurn|UntilEndOfTheNextTurn|UntilEndOfPlayersNextTurn`, `next turn|next end step`),
	newLiteralRule(`EntersTapped`, `tapped`),
	newLiteralRule(`Random`, `random`),
	newLiteralRule(`\bEven\b`, `even`),
	newLiteralRule(`\bOdd\b`, `odd`),
	newLiteralRule(`ToughnessIs|ToughnessOf|GreatestToughness|TotalToughness`, `toughness`),
	newLiteralRule(`PowerIs|PowerOf|GreatestPower|TotalPower`, `power`),
	newLiteralRule(`NextUpkeep|APlayersUpkeep`, `upkeep`),
	newLiteralRule(`EndStep`, `end step|end of`),
	newLiteralRule(`Shuffle`, `shuffl`),
	newLiteralRule(`EntersUnderPlayersControl\(You\)`, `under your control|gain control`),
	newLiteralRule(`UntilPlayersNextTurn`, `until your next turn|until the next turn|for as long as|next turn`),
}

var literalEnglishRules = []literalRule{
	newLiteralRule(`UntilPlayersNextTurn`, `until your next turn`),
	newLiteralRule(`Random`, `\bat random\b`),
	newLiteralRule(`Shuffle`, `\bshuffles?\b`),
	newLiteralRule(`Upkeep`, `next upkeep`),
}

// A template can repeat a phrase, or lose it from one clause of several, so each
// occurrence needs its own token. Presence alone would pass both of those.
var literalCountRules = []literalRule{
	newLiteralRule(`Tapped|EnterTapped`, `(battlefield|enters?)( under [^.,]*control)? tapped|creates? [^.,"]*\btapped\b[^.,"]*tokens?`),
	newLiteralRule(`EntersAttacking|Attacking`, `tapped and attacking`),
	newLiteralRule(`Random`, `random order`),
	newLiteralRule(`RoundedUp`, `rounded up`),
	newLiteralRule(`RoundedDown`, `rounded down`),
	newLiteralRule(`UntilTheEndOfPlayersNextTurn|UntilEndOfNextTurn`, `until the end of your next turn`),
	newLiteralRule(`UntilPlayersNextTurn`, `until your next turn`),
	newLiteralRule(`WithoutPaying|Free`, `without paying`),
	newLiteralRule(`FaceDown|Facedown`, `exiles? [^.]*face down`),
	newLiteralRule(`TopOrBottom`, `choice of the top or bottom`),
	newLiteralRule(`Bottom`, `(the rest|the other|others?) on the bottom`),
	newLiteralRule(`Twice|Double|Times`, `\btwice\b`),
}

var (
	literalCapture  = regexp.MustCompile(`\{\{[^}]*\}\}`)
	literalQuoted   = regexp.MustCompile(`"[^"]*"`)
	literalWord     = regexp.MustCompile(`[A-Za-z]+`)
	literalToken    = regexp.MustCompile(`[A-Z][A-Za-z_0-9]+`)
	literalStopword = map[string]bool{}
	literalGeneric  = map[string]bool{}
)

func init() {
	for _, word := range strings.Fields("a an the of to at in on it its is or and you your that this for with from as if then may each by their they them put target") {
		literalStopword[word] = true
	}
	for _, token := range strings.Fields("Name Rules ManaCost Card CardPT Player Permanent Integer Action ActionList Targeted If And Or Not Spell Typeline Ref_TargetPermanent You") {
		literalGeneric[token] = true
	}
}

func literalStem(word string) string {
	word = strings.ToLower(word)
	for _, suffix := range []string{"ing", "es", "ed", "s"} {
		if strings.HasSuffix(word, suffix) && len(word)-len(suffix) >= 4 {
			return strings.TrimSuffix(word, suffix)
		}
	}
	return word
}

func literalWords(template string) map[string]bool {
	words := make(map[string]bool)
	for _, word := range literalWord.FindAllString(literalCapture.ReplaceAllString(template, " "), -1) {
		words[literalStem(word)] = true
	}
	return words
}

func literalTokens(result string) map[string]bool {
	tokens := make(map[string]bool)
	stripped := literalQuoted.ReplaceAllString(literalCapture.ReplaceAllString(result, " "), " ")
	for _, token := range literalToken.FindAllString(stripped, -1) {
		tokens[token] = true
	}
	return tokens
}

type literalTemplate struct {
	rule, input, output string
	words, tokens       map[string]bool
}

// strongLiteralPair is the mining threshold: the pair holds for nearly all carriers,
// few templates break it, and it is far more common than chance.
func strongLiteralPair(paired, carriers int, lift float64) bool {
	missing := carriers - paired
	if carriers < 6 || missing == 0 {
		return false
	}
	ratio := float64(paired) / float64(carriers)
	return (carriers >= 8 && ratio >= 0.93 && missing <= 3 && lift >= 8) || (ratio >= 0.85 && missing <= 4 && lift >= 25)
}

func AuditGrammarLiterals(filenames []string) []LiteralFinding {
	grammar := ReadGrammarJsoncFiles(filenames)
	var templates []literalTemplate
	for rule, definitions := range grammar {
		for i, input := range definitions.TemplateStrings {
			if output := definitions.ResultStrings[i]; output != "" {
				templates = append(templates, literalTemplate{rule, input, output, literalWords(input), literalTokens(output)})
			}
		}
	}
	findings := make(map[string]LiteralFinding)
	add := func(kind string, t literalTemplate, token, word, support string) {
		id := fmt.Sprintf("%x", sha256.Sum256([]byte(strings.Join([]string{kind, t.rule, t.input, t.output, token, word}, "\x00"))))
		findings[id] = LiteralFinding{ID: id, Kind: kind, Rule: t.rule, Token: token, Word: word, Support: support, Template: t.input, Result: t.output}
	}
	for _, t := range templates {
		for _, rule := range literalTokenRules {
			if rule.token.MatchString(t.output) && !rule.english.MatchString(t.input) {
				add("token_rule", t, rule.token.String(), rule.english.String(), "")
			}
		}
		for _, rule := range literalEnglishRules {
			if rule.english.MatchString(t.input) && !rule.token.MatchString(t.output) {
				add("english_rule", t, rule.token.String(), rule.english.String(), "")
			}
		}
		for _, rule := range literalCountRules {
			if len(rule.english.FindAllString(t.input, -1)) > len(rule.token.FindAllString(t.output, -1)) {
				add("count_rule", t, rule.token.String(), rule.english.String(), "")
			}
		}
	}
	wordCount := make(map[string]int)
	tokenCount := make(map[string]int)
	pairCount := make(map[[2]string]int)
	for _, t := range templates {
		for word := range t.words {
			wordCount[word]++
		}
		for token := range t.tokens {
			tokenCount[token]++
			for word := range t.words {
				pairCount[[2]string{token, word}]++
			}
		}
	}
	total := float64(len(templates))
	for pair, paired := range pairCount {
		token, word := pair[0], pair[1]
		if literalStopword[word] || literalGeneric[token] {
			continue
		}
		byToken := strongLiteralPair(paired, tokenCount[token], float64(paired)/float64(tokenCount[token])/(float64(wordCount[word])/total))
		byWord := strongLiteralPair(paired, wordCount[word], float64(paired)/float64(wordCount[word])/(float64(tokenCount[token])/total))
		if !byToken && !byWord {
			continue
		}
		for _, t := range templates {
			if byToken && t.tokens[token] && !t.words[word] {
				add("mined_token", t, token, word, fmt.Sprintf("%d/%d", paired, tokenCount[token]))
			}
			if byWord && t.words[word] && !t.tokens[token] {
				add("mined_word", t, token, word, fmt.Sprintf("%d/%d", paired, wordCount[word]))
			}
		}
	}
	report := make([]LiteralFinding, 0, len(findings))
	for _, finding := range findings {
		report = append(report, finding)
	}
	sort.Slice(report, func(i, j int) bool { return report[i].ID < report[j].ID })
	return report
}

// UnreviewedLiterals applies the pinned reviews, which ignore mining counts so that
// unrelated grammar growth cannot reopen a template that did not change.
func UnreviewedLiterals(findings []LiteralFinding, reviewFilename string) ([]LiteralFinding, error) {
	data, err := os.ReadFile(reviewFilename)
	if err != nil {
		return nil, err
	}
	var review LiteralReview
	if err := json.Unmarshal(data, &review); err != nil {
		return nil, err
	}
	reviewed := make(map[string]bool, len(review.Reviewed))
	for _, entry := range review.Reviewed {
		reviewed[entry.ID] = true
	}
	active := []LiteralFinding{}
	for _, finding := range findings {
		if !reviewed[finding.ID] {
			active = append(active, finding)
		}
	}
	return active, nil
}

func WriteLiteralAudit(filenames []string, output io.Writer, reviewFilename string) error {
	findings := AuditGrammarLiterals(filenames)
	if reviewFilename != "" {
		var err error
		if findings, err = UnreviewedLiterals(findings, reviewFilename); err != nil {
			return err
		}
	}
	encoder := json.NewEncoder(output)
	encoder.SetIndent("", "  ")
	encoder.SetEscapeHTML(false)
	return encoder.Encode(findings)
}
