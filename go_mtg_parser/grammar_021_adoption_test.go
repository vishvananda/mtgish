package main

import "testing"

// #169's rejection restores the unconditional private choice. The token
// fixture also checks the actual shared input, rather than a gift-only patch.
func TestGrammar021SourceCorrections(t *testing.T) {
	testSourceReferenceFixtures(t, "bison_whistle_169.ronl", "octopus_token_021.ronl")
}

func TestRejected170AndPartlyAccepted172SourceContracts(t *testing.T) {
	testSourceReferenceFixtures(t, "rejected_170.ronl", "rejected_172.ronl")
}
