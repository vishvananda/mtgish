package main

import "testing"

// One shared "put" creates a single event with both counter-kind contributions.
func TestSharedVerbCounterKinds(t *testing.T) {
	testSourceReferenceFixtures(t, "counter_kinds_20261111.ronl")
}
