package main

import "testing"

// Heroism's 2004-10-04 ruling places each controller's payment at ability
// resolution, before the prevention effects exist, not at a damage event.
func TestHeroismResolutionPayment(t *testing.T) {
	testSourceReferenceFixtures(t, "heroism_resolution_payment.ronl")
}
