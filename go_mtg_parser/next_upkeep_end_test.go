package main

import "testing"

func TestNextUpkeepEndPreservesThePrintedBoundary(t *testing.T) {
	testSourceReferenceFixtures(t, "next_upkeep_end.ronl")
}
