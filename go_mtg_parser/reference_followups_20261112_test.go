package main

import "testing"

// Fresh English parsing must reproduce the complete reviewed Source packet.
func TestReferenceFollowups20261112(t *testing.T) {
	testSourceReferenceFixtures(t, "reference_followups_20261112.ronl")
}
