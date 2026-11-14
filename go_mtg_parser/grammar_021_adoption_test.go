package main

import "testing"

// #169's rejection restores the unconditional private choice. The token
// fixture also checks the actual shared input, rather than a gift-only patch.
func TestGrammar021SourceCorrections(t *testing.T) {
	testSourceReferenceFixtures(t, "bison_whistle_169.ronl", "octopus_token_021.ronl")
}
