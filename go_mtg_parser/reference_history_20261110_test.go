package main

import "testing"

func TestReferenceEventAndPaymentHistory(t *testing.T) {
	testSourceReferenceFixtures(t, "reference_history_20261110.ronl", "reference_crew_history_20261110.ronl")
}
