package main

import "testing"

func TestInvestigatedReferencePaymentsAndDrawCapture(t *testing.T) {
	testSourceReferenceFixtures(t, "reference_investigations.ronl")
}

func TestSacrificeTriggersKeepPublicDestinations(t *testing.T) {
	testSourceReferenceFixtures(t, "sacrifice_public_destinations.ronl")
}

func TestReferenceFollowupOperands(t *testing.T) {
	testSourceReferenceFixtures(t, "reference_followup_20261106b.ronl")
}
