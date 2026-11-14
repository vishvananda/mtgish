package main

import "testing"

// The snow-only "may ... instead" retains the original one-blue option.
// Extra mana choices are zero/one/two for snow and zero/one otherwise.
func TestSnowfallOptionalInsteadRetainsOriginalMana(t *testing.T) {
	testSourceReferenceFixtures(t, "snowfall_optional_mana.ronl")
}
