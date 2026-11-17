package main

import "testing"

// One source-conditioned mana replacement remains one ability, including on
// lands with several applicable basic land types. It must not become several
// independently applied replacements.
func TestManaReplacementSourceOptionsRemainOnePrintedAbility(t *testing.T) {
	testSourceReferenceFixtures(t, "mana_source_options_030.ronl")
}
