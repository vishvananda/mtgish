package main

import "testing"

// Template parameter lists must not invent event filters or change the
// referent of identical Oracle wording. Includes all 30 changed records and
// matching animation / self-event controls from the whole-corpus parser diff.
func TestReferenceTemplateContextDoesNotChangeOracleReferents(t *testing.T) {
	testSourceReferenceFixtures(t, "reference_consistency_20261112.ronl")
}
