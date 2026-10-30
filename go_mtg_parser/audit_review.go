package main

import (
	"bytes"
	"crypto/sha256"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"strconv"
	"strings"
)

// Reviews apply to exact findings and exact parser inputs. A static audit can
// reuse the saved review cheaply; a usage audit also verifies fresh evidence.
// A changed template, card, cost, preprocessing rule, or matched use reopens it.
type AuditReview struct {
	Version     int                `json:"version"`
	InputRoot   string             `json:"input_root"`
	Inputs      map[string]string  `json:"review_inputs"`
	Sources     map[string]string  `json:"reviewed_sources"`
	SourceOrder []string           `json:"reviewed_source_order"`
	Reviewed    []AuditReviewEntry `json:"reviewed"`
}

type AuditReviewEntry struct {
	ID          string `json:"id"`
	Disposition string `json:"disposition"`
	Reason      string `json:"reason"`
	Evidence    string `json:"matched_evidence_sha256"`
}

func auditEvidence(uses []AuditUse) (string, error) {
	// Canonicalize struct fields as sorted JSON object keys, matching the saved
	// language-independent review hashes (maps are sorted by encoding/json).
	data, err := json.Marshal(uses)
	if err != nil {
		return "", err
	}
	var canonical any
	if err := json.Unmarshal(data, &canonical); err != nil {
		return "", err
	}
	var encoded bytes.Buffer
	encoder := json.NewEncoder(&encoded)
	encoder.SetEscapeHTML(false)
	if err := encoder.Encode(canonical); err != nil {
		return "", err
	}
	return fmt.Sprintf("%x", sha256.Sum256(bytes.TrimSuffix(encoded.Bytes(), []byte("\n")))), nil
}

var captureIndex = regexp.MustCompile(`^(.*)\[(\d+)\]$`)

// Only context-free scalar families qualify. Expressions with implicit subjects
// and ability recipes still need an explicit, input-checked review.
func equivalentScalarCapture(f AuditFinding) bool {
	match := captureIndex.FindStringSubmatch(f.Capture)
	if f.Kind != "unused_semantic_capture" || match == nil || len(f.Usage) == 0 {
		return false
	}
	name := match[1]
	switch name {
	case "Number", "Digits", "PT", "PTMod", "PTModX", "CounterType", "ManaCost":
	default:
		return false
	}
	index, _ := strconv.Atoi(match[2])
	var referenced []int
	for _, reference := range ExtractTemplateVariables(f.Output) {
		if reference.MatchTemplate == name {
			referenced = append(referenced, reference.MatchIndex)
		}
	}
	for _, use := range f.Usage {
		values := use.Captures[name]
		if index >= len(values) {
			return false
		}
		equivalent := false
		for _, other := range referenced {
			if other < len(values) && values[index] == values[other] {
				equivalent = true
			}
		}
		if !equivalent {
			return false
		}
	}
	return true
}

func ReviewGrammarAudit(report *GrammarAudit, filename string, withUsage bool) error {
	entries := make(map[string]AuditReviewEntry)
	validSnapshot := false
	if filename != "" {
		data, err := os.ReadFile(filename)
		if err != nil {
			return err
		}
		var review AuditReview
		if err := json.Unmarshal(data, &review); err != nil {
			return err
		}
		if review.Version != 1 {
			return fmt.Errorf("unsupported grammar review version %d", review.Version)
		}
		for _, entry := range review.Reviewed {
			switch entry.Disposition {
			case "reviewed_equivalent_in_current_corpus", "reviewed_implicit_context", "reviewed_separate_instructions":
			default:
				return fmt.Errorf("unknown grammar review disposition %q", entry.Disposition)
			}
			if entry.ID == "" || entry.Reason == "" || len(entry.Evidence) != 64 {
				return fmt.Errorf("incomplete grammar review entry %q", entry.ID)
			}
			if _, exists := entries[entry.ID]; exists {
				return fmt.Errorf("duplicate grammar review entry %q", entry.ID)
			}
			entries[entry.ID] = entry
		}
		validSnapshot = len(review.Inputs) > 0 && review.InputRoot != ""
		report.ReviewStatus = "applied"
		if !validSnapshot {
			report.ReviewStatus = "stale: review input fingerprints are missing"
		}
		root := filepath.Join(filepath.Dir(filename), review.InputRoot)
		for path, expected := range review.Inputs {
			contents, err := os.ReadFile(filepath.Join(root, path))
			if err != nil || fmt.Sprintf("%x", sha256.Sum256(contents)) != expected {
				validSnapshot = false
				report.ReviewStatus = "stale: parser input changed: " + path
				break
			}
		}
		if len(report.Sources) > 0 {
			if len(report.SourceOrder) == 0 || strings.Join(report.SourceOrder, "\x00") != strings.Join(review.SourceOrder, "\x00") {
				validSnapshot = false
			}
			if len(report.Sources) != len(review.Sources) {
				validSnapshot = false
			}
			for path, digest := range report.Sources {
				if review.Sources[path] != digest {
					validSnapshot = false
				}
			}
			if !validSnapshot && report.ReviewStatus == "applied" {
				report.ReviewStatus = "stale: different effective grammar inputs"
			}
		}
		if withUsage {
			if len(report.ParserInputs) == 0 {
				validSnapshot = false
			}
			for path, digest := range report.ParserInputs {
				if review.Inputs[path] != digest {
					validSnapshot = false
				}
			}
			if !validSnapshot && report.ReviewStatus == "applied" {
				report.ReviewStatus = "stale: different or unverified parser invocation inputs"
			}
		}
	}
	active := make([]AuditFinding, 0, len(report.Findings))
	for _, finding := range report.Findings {
		reason := ""
		// Never infer equivalence from a partial/failed parse or stale saved uses.
		if withUsage && report.ParsedCards > 0 && report.ParseFailures == 0 && equivalentScalarCapture(finding) {
			reason = "every freshly matched occurrence equals an output-referenced scalar capture"
		} else if entry, ok := entries[finding.ID]; ok && validSnapshot {
			if !withUsage {
				reason = entry.Reason
			} else {
				evidence, err := auditEvidence(finding.Usage)
				if err != nil {
					return err
				}
				if len(finding.Usage) > 0 && report.ParseFailures == 0 && evidence == entry.Evidence {
					reason = entry.Reason
				}
			}
		}
		if reason == "" {
			active = append(active, finding)
		} else {
			finding.ReviewReason = reason
			report.Reviewed = append(report.Reviewed, finding)
		}
	}
	report.Findings = active
	return nil
}
