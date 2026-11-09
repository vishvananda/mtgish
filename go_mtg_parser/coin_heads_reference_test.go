package main

import "testing"

func TestCoinFacesAreNotFlipsWon(t *testing.T) {
	testSourceReferenceFixtures(t, "coin_heads_reference.ronl")
}
