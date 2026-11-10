package main

import "testing"

// CR608.2f: one shared Put instruction places both coordinated recipients'
// counters together. Keep targets and captures unchanged, even when they overlap.
func TestCounterPlacementTiming20261110(t *testing.T) {
	testSourceReferenceFixtures(t, "counter_timing_20261110.ronl", "counter_timing_controls_20261110.ronl")
}
