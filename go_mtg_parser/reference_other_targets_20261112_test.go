package main

import "testing"

// An earlier artifact/enchantment target does not become the implicit creature
// source named by "other creatures". Preserve explicit target-to-target cases.
func TestOtherCreatureTargetsNameTheirActualAntecedent(t *testing.T) {
	testSourceReferenceFixtures(t, "reference_other_targets_20261112.ronl")
}
