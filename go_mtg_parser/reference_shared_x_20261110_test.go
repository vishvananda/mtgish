package main

import "testing"

// A shared where-clause computes X once, before its first action. Cast X and
// life actually lost remain distinct contracts in the unchanged controls.
func TestReferenceSharedXSnapshotBeforePairedActions(t *testing.T) {
	testSourceReferenceFixtures(t, "reference_shared_x_20261110.ronl", "reference_shared_x_controls_20261110.ronl")
}
