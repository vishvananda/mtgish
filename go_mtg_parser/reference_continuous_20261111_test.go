package main

import "testing"

func TestContinuousEffectReferenceCorrections(t *testing.T) {
	testSourceReferenceFixtures(t, "reference_continuous_20261111.ronl")
}

func TestDiscardedSourceShuffleReference(t *testing.T) {
	testSourceReferenceFixtures(t, "reference_kogla_20261111.ronl")
}
