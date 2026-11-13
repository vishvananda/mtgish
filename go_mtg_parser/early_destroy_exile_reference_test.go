package main

import "testing"

func TestEarlyDestroyAndExileReference(t *testing.T) {
	testSourceReferenceFixtures(t, "early_destroy_choice.ronl")
}
