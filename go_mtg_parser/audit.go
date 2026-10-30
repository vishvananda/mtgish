package main

import (
	"crypto/sha256"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"regexp"
	"sort"
	"strings"
)

// This audit finds source evidence, not proof of semantic correctness. Missing
// references definitely discard a capture; grouping candidates require review.
type GrammarAudit struct {
	Version       int               `json:"version"`
	Rules         int               `json:"rules"`
	Sources       map[string]string `json:"sources,omitempty"`
	SourceOrder   []string          `json:"source_order,omitempty"`
	ParserInputs  map[string]string `json:"parser_inputs,omitempty"`
	ParsedCards   int               `json:"parsed_cards,omitempty"`
	ParseFailures int               `json:"parse_failures,omitempty"`
	Findings      []AuditFinding    `json:"findings"`
	Reviewed      []AuditFinding    `json:"reviewed_findings,omitempty"`
	ReviewStatus  string            `json:"review_status,omitempty"`
}

type AuditFinding struct {
	ID           string     `json:"id"`
	File         string     `json:"file"`
	Line         int        `json:"line"`
	Rule         string     `json:"rule"`
	Kind         string     `json:"kind"`
	Severity     string     `json:"severity"`
	Capture      string     `json:"capture,omitempty"`
	Input        string     `json:"input"`
	Output       string     `json:"output"`
	Reason       string     `json:"reason"`
	Usage        []AuditUse `json:"usage,omitempty"`
	ReviewReason string     `json:"review_reason,omitempty"`
}

type AuditUse struct {
	Card        string              `json:"card"`
	MatchedText string              `json:"matched_text"`
	Captures    map[string][]string `json:"captures"`
}

// The optional usage pass consumes the parser's existing derivation tree. It
// associates a finding with real cards and each matched occurrence's text, so
// reviewers can distinguish, for example, two equal numbers from two different
// abilities accidentally emitted as the first ability twice.
type AuditUsage struct {
	Report     GrammarAudit
	byTemplate map[string][]int
}

func NewAuditUsage(report GrammarAudit) *AuditUsage {
	u := &AuditUsage{Report: report, byTemplate: make(map[string][]int)}
	for i, finding := range report.Findings {
		key := finding.Rule + "\x00" + finding.Input
		u.byTemplate[key] = append(u.byTemplate[key], i)
	}
	return u
}

func (u *AuditUsage) Record(grammar TemplatesAndResults, card CardInfo) {
	var visit func(Breadcrumb)
	text := func(b Breadcrumb) string {
		if b.TokenStart < 0 || b.TokenEnd > len(card.EnglishTokens) || b.TokenEnd < b.TokenStart {
			return ""
		}
		return strings.Join(card.EnglishTokens[b.TokenStart:b.TokenEnd], "")
	}
	visit = func(b Breadcrumb) {
		definition, ok := grammar[b.MatchReference]
		if !ok || b.MatchIndex < 0 || b.MatchIndex >= len(definition.TemplateStrings) {
			return
		}
		key := b.MatchReference + "\x00" + definition.TemplateStrings[b.MatchIndex]
		if indices := u.byTemplate[key]; len(indices) > 0 {
			use := AuditUse{Card: card.Name, MatchedText: text(b), Captures: make(map[string][]string)}
			for _, child := range b.Breadcrumbs {
				use.Captures[child.MatchReference] = append(use.Captures[child.MatchReference], text(child))
			}
			for _, index := range indices {
				u.Report.Findings[index].Usage = append(u.Report.Findings[index].Usage, use)
			}
		}
		for _, child := range b.Breadcrumbs {
			visit(child)
		}
	}
	visit(card.EnglishBreadcrumbs)
}

func (u *AuditUsage) Write(destination string) error {
	file, err := os.CreateTemp(filepath.Dir(destination), ".grammar-audit-*.tmp")
	if err != nil {
		return err
	}
	defer os.Remove(file.Name())
	encoder := json.NewEncoder(file)
	encoder.SetIndent("", "  ")
	if err := encoder.Encode(u.Report); err != nil {
		file.Close()
		return err
	}
	if err := file.Close(); err != nil {
		return err
	}
	return os.Rename(file.Name(), destination)
}

// Article, agreement and subject words often intentionally produce no data.
// These families carry values that must be substituted or deliberately encoded.
var semanticCapture = regexp.MustCompile(`^(?:Hasable|QuotedAbility|PerpetualQuotedAbility|PTModX?|PTX?|CounterType|ManaCostX?|Number|Digits|GameNumber|Condition|Expiration|Cost|Action|Actions)(?:$|\[|\()`)
var stateVerb = regexp.MustCompile(`(?i)\b(?:becomes?|gets?|gains?|loses?)\b`)
var typeVerb = regexp.MustCompile(`(?i)\bbecomes?\b`)
var statVerb = regexp.MustCompile(`(?i)\bgets?\b`)
var abilityVerb = regexp.MustCompile(`(?i)\b(?:gains?|loses?)\b`)
var lifeChange = regexp.MustCompile(`(?i)\b(?:gains?|loses?)\s+(?:CAPTURE\s+)?life\b`)
var baseStat = regexp.MustCompile(`(?i)\bbase (?:power|toughness)\b`)
var clauseBreak = regexp.MustCompile(`\.\s+|,?\s+then\s+`)
var layerCall = regexp.MustCompile(`\b(?:Create(?:Each)?PermanentLayerEffect(?:Until)?|CreatePerpetualPermanentEffect)\(`)
var typeChange = regexp.MustCompile(`\b(?:Set|Add|Remove)(?:CreatureTypes?|Cardtypes?|LandTypes?|Color)\(`)
var statChange = regexp.MustCompile(`\b(?:AdjustPT\w*|SetPT|SetPower|SetToughness)\(`)
var abilityChange = regexp.MustCompile(`\b(?:AddAbility\w*|LosesAllAbilities|LosesAbility)\b`)

// Mask placeholder parameters, parenthetical text and quoted granted abilities:
// their punctuation does not separate outer instructions.
func outerText(text string) string {
	text = templateFinder.ReplaceAllStringFunc(text, func(capture string) string {
		name := strings.TrimSuffix(strings.TrimPrefix(capture, "{{"), "}}")
		if strings.HasPrefix(name, "QuotedAbility") || strings.HasPrefix(name, "PerpetualQuotedAbility") || strings.HasPrefix(name, "Hasable") {
			return "ABILITY_CAPTURE"
		}
		if name == "PT" || name == "PTX" {
			return "STATS_CAPTURE"
		}
		return "CAPTURE"
	})
	var result strings.Builder
	quoted, escaped, depth := false, false, 0
	for i, r := range text {
		if escaped {
			escaped = false
			continue
		}
		if r == '\\' && quoted {
			escaped = true
			continue
		}
		if r == '"' {
			if !quoted && strings.HasPrefix(text[i:], `"ABILITY_CAPTURE"`) {
				result.WriteString(" ABILITY_CAPTURE ")
			}
			quoted = !quoted
			result.WriteRune(' ')
			continue
		}
		if quoted {
			continue
		}
		if r == '(' {
			depth++
			continue
		}
		if r == ')' && depth > 0 {
			depth--
			continue
		}
		if depth == 0 {
			result.WriteRune(r)
		}
	}
	return result.String()
}

// Inspect each constructor separately, so two properly separated effect actions
// do not look like one merged action. Mask capture parameters before balancing.
func layerBodies(output string) []string {
	output = templateFinder.ReplaceAllString(output, "CAPTURE")
	var result []string
	for _, span := range layerCall.FindAllStringIndex(output, -1) {
		depth := 1
		quoted, escaped := false, false
		for i := span[1]; i < len(output); i++ {
			if escaped {
				escaped = false
				continue
			}
			if quoted && output[i] == '\\' {
				escaped = true
				continue
			}
			if output[i] == '"' {
				quoted = !quoted
				continue
			}
			if quoted {
				continue
			}
			switch output[i] {
			case '(':
				depth++
			case ')':
				depth--
			}
			if depth == 0 {
				result = append(result, output[span[1]:i])
				break
			}
		}
	}
	return result
}

func auditTemplate(rule, input, output string, grammar TemplatesAndResults) []AuditFinding {
	var findings []AuditFinding
	used := make(map[string]map[int]bool)
	for _, reference := range ExtractTemplateVariables(output) {
		if reference.MatchTemplate == "" {
			continue
		}
		if used[reference.MatchTemplate] == nil {
			used[reference.MatchTemplate] = make(map[int]bool)
		}
		used[reference.MatchTemplate][reference.MatchIndex] = true
	}
	occurrences := make(map[string]int)
	for _, reference := range templateFinder.FindAllStringSubmatch(input, -1) {
		name := reference[1]
		index := occurrences[name]
		occurrences[name]++
		if !semanticCapture.MatchString(name) || used[name][index] {
			continue
		}
		// A rule restricted to one output value may encode that value literally.
		if definition, ok := grammar[name]; ok && len(definition.ResultStrings) > 0 {
			constant := definition.ResultStrings[0]
			same := constant != "" && !strings.Contains(constant, "{{")
			for _, other := range definition.ResultStrings {
				same = same && other == constant
			}
			if same && strings.Contains(output, constant) {
				continue
			}
		}
		findings = append(findings, AuditFinding{Rule: rule, Kind: "unused_semantic_capture", Severity: "error",
			Capture: fmt.Sprintf("%s[%d]", name, index), Input: input, Output: output,
			Reason: "the output never references this semantic capture occurrence; review any intentional hardcoded equivalent"})
	}
	var clauses []int
	for _, clause := range clauseBreak.Split(outerText(input), -1) {
		clause = lifeChange.ReplaceAllString(clause, "")
		if stateVerb.MatchString(clause) {
			families := 0
			for i, pattern := range []*regexp.Regexp{typeVerb, statVerb, abilityVerb} {
				if pattern.MatchString(clause) {
					families |= 1 << i
				}
			}
			if baseStat.MatchString(clause) || (typeVerb.MatchString(clause) && strings.Contains(clause, "STATS_CAPTURE")) {
				families |= 2
			}
			if strings.Contains(clause, "ABILITY_CAPTURE") {
				families |= 4
			}
			clauses = append(clauses, families)
		}
	}
	if len(clauses) >= 2 {
		for _, body := range layerBodies(output) {
			families := 0
			for i, pattern := range []*regexp.Regexp{typeChange, statChange, abilityChange} {
				if pattern.MatchString(body) {
					families |= 1 << i
				}
			}
			fitsOneClause := false
			for _, clause := range clauses {
				fitsOneClause = fitsOneClause || clause&families == families
			}
			if families != 0 && families&(families-1) != 0 && !fitsOneClause {
				findings = append(findings, AuditFinding{Rule: rule, Kind: "merged_instruction_candidate", Severity: "review", Input: input, Output: output,
					Reason: "separate state-changing clauses produce different kinds of change inside one effect action; verify the instruction boundary"})
				break
			}
		}
	}
	return findings
}

func AuditGrammarFiles(filenames []string) (GrammarAudit, error) {
	grammar := ReadGrammarJsoncFiles(filenames)
	// A later file replaces an entire rule in the parser. Audit its effective
	// definition, retaining the source location instead of reporting shadowed rules.
	owners := make(map[string]string)
	contents := make(map[string]string)
	sources := make(map[string]string)
	for _, filename := range filenames {
		data, err := os.ReadFile(filename)
		if err != nil {
			return GrammarAudit{}, err
		}
		contents[filename] = string(data)
		sources[auditSourcePath(filename)] = fmt.Sprintf("%x", sha256.Sum256(data))
		for rule := range ReadJsoncMap[map[string]string](filename) {
			owners[strings.TrimSuffix(strings.TrimPrefix(rule, "{{"), "}}")] = filename
		}
	}
	report := GrammarAudit{Version: 1, Sources: sources, Findings: []AuditFinding{}}
	for _, filename := range filenames {
		report.SourceOrder = append(report.SourceOrder, auditSourcePath(filename))
	}
	for rule, definitions := range grammar {
		for i, input := range definitions.TemplateStrings {
			report.Rules++
			for _, finding := range auditTemplate(rule, input, definitions.ResultStrings[i], grammar) {
				finding.File = owners[rule]
				encoded, _ := json.Marshal(input)
				if offset := strings.Index(contents[finding.File], string(encoded)); offset >= 0 {
					finding.Line = 1 + strings.Count(contents[finding.File][:offset], "\n")
				}
				finding.File = auditSourcePath(finding.File)
				finding.ID = fmt.Sprintf("%x", sha256.Sum256([]byte(strings.Join([]string{finding.File, finding.Rule, finding.Kind, finding.Capture, finding.Input, finding.Output}, "\x00"))))
				report.Findings = append(report.Findings, finding)
			}
		}
	}
	sort.Slice(report.Findings, func(i, j int) bool {
		a, b := report.Findings[i], report.Findings[j]
		if a.File != b.File {
			return a.File < b.File
		}
		if a.Line != b.Line {
			return a.Line < b.Line
		}
		return a.ID < b.ID
	})
	return report, nil
}

func auditSourcePath(filename string) string {
	if filepath.IsAbs(filename) {
		if cwd, err := os.Getwd(); err == nil {
			if relative, err := filepath.Rel(cwd, filename); err == nil && relative != ".." && !strings.HasPrefix(relative, ".."+string(filepath.Separator)) {
				return filepath.ToSlash(relative)
			}
		}
	}
	return filepath.ToSlash(filepath.Clean(filename))
}

func AuditParserInputs(filenames []string) (map[string]string, error) {
	inputs := make(map[string]string)
	for _, filename := range filenames {
		data, err := os.ReadFile(filename)
		if err != nil {
			return nil, err
		}
		inputs[auditSourcePath(filename)] = fmt.Sprintf("%x", sha256.Sum256(data))
	}
	return inputs, nil
}

func WriteGrammarAudit(filenames []string, output io.Writer, reviewFilename string) error {
	report, err := AuditGrammarFiles(filenames)
	if err != nil {
		return err
	}
	if err := ReviewGrammarAudit(&report, reviewFilename, false); err != nil {
		return err
	}
	encoder := json.NewEncoder(output)
	encoder.SetIndent("", "  ")
	return encoder.Encode(report)
}
