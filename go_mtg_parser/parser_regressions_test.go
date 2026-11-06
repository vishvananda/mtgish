package main

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"testing"
)

// Exercise the actual English -> mtgish pipeline using checked-in card text.
// Expected fragments assert meaning (numbers, abilities, costs and ordering),
// rather than accepting any output which happens to parse.
func TestParserSemanticRegressions(t *testing.T) {
	englishFiles, err := filepath.Glob("../grammars/english_grammar*.json5")
	if err != nil {
		t.Fatal(err)
	}
	mtgishFiles, err := filepath.Glob("../grammars/mtgish_grammar*.json5")
	if err != nil {
		t.Fatal(err)
	}
	english := ReadGrammarJsoncFiles(englishFiles)
	mtgish := ReadGrammarJsoncFiles(mtgishFiles)
	englishTree := CreateTokenReTriesFromTemplatesAndResults(english)
	shortNames := ReadJsoncMap[string]("../grammars/short_names.json5")
	previousReplacements := findReplaceReasons
	findReplaceReasons = ReadJsoncArray[FindReplaceEntry]("../grammars/find_replace.json5")
	defer func() { findReplaceReasons = previousReplacements }()
	cards := make(map[string]MtgJsonEntry)
	for _, file := range []string{"oracle", "additional_cards", "tokens", "dungeons"} {
		for _, card := range ReadJsoncArray[MtgJsonEntry]("../data/" + file + ".json") {
			cards[card.Name] = card
		}
	}
	var quotedNames []string
	for _, card := range cards {
		for _, face := range card.Cards {
			if face.Type != "Stickers" {
				quotedNames = append(quotedNames, strconv.Quote(face.Name))
			}
		}
	}
	mtgish["QUOTED_NAME"] = LoadTemplatesFor(makeSelfPairs(quotedNames))
	mtgishTree := CreateTokenReTriesFromTemplatesAndResults(mtgish)
	// Synthetic controls distinguish printed creature restrictions from a bare
	// subtype. Kindred permanents can satisfy only the latter.
	for _, text := range []struct{ name, cost string }{
		{"Explicit Subtype Creature Cost", "Tap an untapped Wizard creature you control"},
		{"Explicit Plural Subtype Creature Cost", "Tap two untapped Wizard creatures you control"},
	} {
		control := cards["Azami, Lady of Scrolls"]
		control.Cards = append([]MtgJsonAtomic(nil), control.Cards...)
		control.Cards[0].Text = text.cost + ": Draw a card."
		cards[text.name] = control
	}
	control := cards["Ashling, the Limitless"]
	control.Cards = append([]MtgJsonAtomic(nil), control.Cards...)
	control.Cards[0].Text = "Whenever you sacrifice a nontoken Elemental, this creature gets +1/+1 until end of turn."
	cards["Sacrifice Trigger Holder Control"] = control
	deathControl := cards["Titan Hunter"]
	deathControl.Cards = append([]MtgJsonAtomic(nil), deathControl.Cards...)
	deathControl.Cards[0].Text = strings.ReplaceAll(deathControl.Cards[0].Text, "if no creatures died this turn", "if a creature died this turn")
	cards["Positive Death Trigger Control"] = deathControl
	cases := []struct {
		name     string
		contains []string
		excludes []string
	}{
		{"Soul Diviner", []string{"ACounterFromAPermanent(And([Or([IsCardtype(Artifact), IsCardtype(Creature), IsCardtype(Land), IsCardtype(Planeswalker)]), ControlledByAPlayer(SinglePlayer(You))]))"}, nil},
		{"Tranquil Domain", []string{"DestroyEachPermanent(And([IsNonEnchantmentType(Aura), IsCardtype(Enchantment)]))"}, nil},
		{"Bathe in Light", []string{"And([Other(Ref_TargetPermanent), IsCardtype(Creature), SharesAColorWithPermanent(Ref_TargetPermanent)])"}, nil},
		{"Wojek Apothecary", []string{"EachPermanent(And([Other(Ref_TargetPermanent), IsCardtype(Creature), SharesAColorWithPermanent(Ref_TargetPermanent)]))"}, nil},
		{"Fated Clash", []string{"CreateEachPermanentLayerEffectUntil(Or([SinglePermanent(Ref_TargetPermanent1), SinglePermanent(Ref_TargetPermanent2)])"}, nil},
		{"Rukarumel, Biologist", []string{"StackSpellsEffect(And([IsCardtype(Creature), ControlledByAPlayer(SinglePlayer(You))])"}, nil},
		{"Tarrian's Soulcleaver", []string{"WhenAPermanentIsPutIntoAPlayersGraveyard(And([Other(ThisPermanent), Or([IsCardtype(Artifact), IsCardtype(Creature)])]), AnyPlayer)", "PTCounter(+1, +1), HostPermanent"}, []string{"Other(HostPermanent)"}},
		{"Emrakul, the Aeons Torn", []string{"Protection(FromSpells(IsColored))", "PlayerAction(OwnerOfGraveyrdCard(Trigger_ThatGraveyardCard), ShuffleGraveyardIntoLibrary)"}, []string{"Protection(FromColor(Colored))"}},
		{"Kozilek, Butcher of Truth", []string{"PlayerAction(OwnerOfGraveyrdCard(Trigger_ThatGraveyardCard), ShuffleGraveyardIntoLibrary)"}, nil},
		{"Ulamog, the Infinite Gyre", []string{"PlayerAction(OwnerOfGraveyrdCard(Trigger_ThatGraveyardCard), ShuffleGraveyardIntoLibrary)"}, nil},
		{"Satya, Aetherflux Genius", []string{"Unless(CostWasPaid, [SacrificeEachPermanent(TheTokensCreatedThisWay)])"}, []string{"Unless(CostWasntPaid,"}},
		{"Titan Hunter", []string{"ANumberOfPermanentsDiedThisTurn(EqualTo(Integer(0)), IsCardtype(Creature))"}, nil},
		{"Positive Death Trigger Control", []string{"ACreatureOrPlaneswalkerDiedThisTurn(IsCardtype(Creature))"}, []string{"Not(ACreatureOrPlaneswalkerDiedThisTurn"}},
		{"Bridgeworks Battle // Tanglespan Bridgeworks", []string{"Targeted([TargetPermanent(And([IsCardtype(Creature), ControlledByAPlayer(SinglePlayer(You))])), UptoOneTargetPermanent(And([IsCardtype(Creature), ControlledByAPlayer(Other(You))]))]"}, nil},
		{"Response // Resurgence", []string{"ThereIsAnAdditionalCombatPhaseAndAnAdditionalMainPhase"}, []string{"ThereIsAnAdditionalCombatPhase])"}},
		{"Ashling, the Limitless", []string{"TokenCopyOfPermanent(Trigger_ThatSacrificedPermanent, NoTokenCopyEffects)"}, []string{"TokenCopyOfPermanent(ThisPermanent, NoTokenCopyEffects)"}},
		{"Jenny Flint", []string{"TargetPermanent(And([Other(ThisPermanent),"}, []string{"Other(Trigger_ThatPermanent)", "Other(Trigger_ThatSacrificedPermanent)"}},
		{"Lonis, Genetics Expert", []string{"TargetPermanent(And([Other(ThisPermanent),"}, []string{"Other(Trigger_ThatPermanent)", "Other(Trigger_ThatSacrificedPermanent)"}},
		{"Sacrifice Trigger Holder Control", []string{"CreatePermanentLayerEffectUntil(ThisPermanent, [AdjustPT(+1, +1)], UntilEndOfTurn)"}, []string{"CreatePermanentLayerEffectUntil(Trigger_ThatPermanent,"}},
		{"Timecrafting", []string{"TargetPermanent(HasACounterOfType(TimeCounter))", "TargetPermanent(AnyPermanent)", "NumberCountersOfTypeOnPermanent(ValueX, TimeCounter, Ref_TargetPermanent)", "NumberCountersOfTypeOnCardInExile(ValueX, TimeCounter, Ref_TargetExiledCard)"}, nil},
		{"Arcane Artisan", []string{"PlayerAction(Ref_TargetPlayer, CreateTokens([TokenCopyOfExiledCard"}, nil},
		{"Basilica Bell-Haunt", []string{"EachPlayerAction(Opponent, DiscardACard), GainLife(Integer(3))"}, []string{"EachPlayerAction(Opponent, GainLife"}},
		{"Crash Landing", []string{"Permanent(Ref_TargetPermanent))"}, []string{"Player(ControllerOfPermanent(Ref_TargetPermanent))"}},
		{"Essence Backlash", []string{"Player(ControllerOfSpell(Ref_TargetSpell))"}, []string{"Player(ControllerOfSpell(ThisSpell))"}},
		{"Ionize", []string{"Player(ControllerOfSpell(Ref_TargetSpell))"}, []string{"Player(ControllerOfSpell(ThisSpell))"}},
		{"Mindswipe", []string{"PlayerMayCost(ControllerOfSpell(Ref_TargetSpell),"}, []string{"PlayerMayCost(ControllerOfSpell(ThisSpell),"}},
		{"Gutter Grime", nil, []string{"SinglePermanent(ThisPermanent)), TheTotalNumberOfCountersOfTypeAmongPermanents(SlimeCounter, SinglePermanent(ThePermanentThatCreatedIt))"}},
		{"The Master, Gallifrey's End", []string{"PlayerAction(You, CreateTokens([TokenCopyOfExiledCard"}, nil},
		{"Toralf, God of Fury // Toralf's Hammer", []string{"PermanentDealsDamage(ThisPermanent, Integer(3), Ref_AnyTarget)"}, []string{"PermanentDealsDamage(ThisGuest, Integer(3), Ref_AnyTarget)"}},
		{"Razor Boomerang", []string{"PermanentDealsDamage(ThisPermanent, Integer(1), Ref_AnyTarget)"}, []string{"PermanentDealsDamage(ThisGuest, Integer(1), Ref_AnyTarget)"}},
		{"Offering to Asha", []string{"Unless(CostWasPaid, [CounterSpell(Ref_TargetSpell)]), GainLife(Integer(4))"}, nil},
		{"Stymied Hopes", []string{"Unless(CostWasPaid, [CounterSpell(Ref_TargetSpell)]), Scry(Integer(1))"}, nil},
		{"Frightful Delusion", []string{"Unless(CostWasPaid, [CounterSpell(Ref_TargetSpell)]), PlayerAction(ControllerOfSpell(Ref_TargetSpell), DiscardACard)"}, nil},
		{"Sage's Dousing", []string{"Unless(CostWasPaid, [CounterSpell(Ref_TargetSpell)]), If("}, nil},
		{"Insubordination", []string{"PermanentPassesFilter(Trigger_ThatPermanent, AttackedThisTurn)", "Player(Trigger_ThatPlayer)"}, []string{"PermanentDealsDamage(ThisPermanent, Integer(2), Player(HostController))"}},
		{"Lingering Death", []string{"PlayerAction(Trigger_ThatPlayer, SacrificePermanent(Trigger_ThatPermanent))"}, []string{"PlayerAction(HostController"}},
		{"Aggression", []string{"PermanentPassesFilter(Trigger_ThatPermanent, DidntAttackThisTurn)", "DestroyPermanent(Trigger_ThatPermanent)"}, []string{"PermanentPassesFilter(HostPermanent, DidntAttackThisTurn)", "DestroyPermanent(HostPermanent)"}},
		{"Reality Scramble", []string{"SharesACardtypeWithPermanent(Ref_TargetPermanent)"}, []string{"SharesACardtypeWithPermanentFromAmongCardtypes"}},
		{"Enchantment Alteration", []string{"SharesACardtypeWithPermanentFromAmongCardtypes(HostPermanentOf(Ref_TargetPermanent), [Creature, Land])"}, []string{"SharesACardtypeWithPermanent(HostPermanentOf(Ref_TargetPermanent))"}},
		{"Ageless Sentinels", []string{"SetCreatureTypes([Bird, Giant])"}, []string{"AddCreatureType(Bird)", "AddCreatureType(Giant)"}},
		{"Ascendant Spirit", []string{"SetCreatureTypes([Spirit, Warrior])", "SetCreatureTypes([Spirit, Warrior, Angel])"}, []string{"AddCreatureType(Spirit)"}},
		{"Evolved Sleeper", []string{"SetCreatureTypes([Human, Cleric])", "SetCreatureTypes([Phyrexian, Human, Cleric])"}, []string{"AddCreatureType(Human)"}},
		{"Figure of Destiny", []string{"SetCreatureTypes([Kithkin, Spirit])", "SetCreatureTypes([Kithkin, Spirit, Warrior])"}, []string{"AddCreatureType(Kithkin)"}},
		{"Figure of Fable", []string{"SetCreatureTypes([Kithkin, Scout])", "SetCreatureTypes([Kithkin, Soldier])"}, []string{"AddCreatureType(Kithkin)"}},
		{"Tenth District Hero", []string{"SetCreatureTypes([Human, Detective])"}, []string{"AddCreatureType(Human)"}},
		{"Warden of the First Tree", []string{"SetCreatureTypes([Human, Warrior])", "SetCreatureTypes([Human, Spirit, Warrior])"}, []string{"AddCreatureType(Human)"}},
		{"Fendeep Summoner", []string{"AddCreatureType(Treefolk)", "AddCreatureType(Warrior)"}, []string{"SetCreatureTypes([Treefolk, Warrior])"}},
		{"Hedge Whisperer", []string{"AddCreatureType(Plant)", "AddCreatureType(Boar)"}, []string{"SetCreatureTypes([Plant, Boar])"}},
		{"Myth Realized", []string{"AddCreatureType(Monk)", "AddCreatureType(Avatar)"}, []string{"SetCreatureTypes([Monk, Avatar])"}},
		{"Doc Ock, Evil Inventor", []string{"AddCreatureType(Robot)", "AddCreatureType(Villain)"}, []string{"SetCreatureTypes([Robot, Villain])"}},
		{"Vihaan, Goldwaker", []string{"AddCreatureType(Construct)", "AddCreatureType(Assassin)"}, []string{"SetCreatureTypes([Construct, Assassin])"}},
		{"Gideon Jura", []string{"AddCreatureType(Human)", "AddCreatureType(Soldier)"}, []string{"SetCreatureTypes([Human, Soldier])"}},
		{"Gyruda, Doom of Depths", []string{"ReturnACardMilledThisWayToBattlefield(And([IsCardtype(Creature), ManaValueIs(Even)]),"}, []string{"ReturnACardMilledThisWayToBattlefield(AnyCard,"}},
		{"Diregraf Horde", []string{"ActionList([Reflexive_CreateTokens_WhenYouDo("}, []string{"MayAction(Reflexive_CreateTokens_WhenYouDo("}},
		{"Glissa's Retriever", []string{"ActionList([Reflexive_Exile_WhenYouDo("}, []string{"MayAction(Reflexive_Exile_WhenYouDo("}},
		{"Adrenaline Jockey", []string{"WhenAPlayerCastsASpell(AnyPlayer, AnySpell)"}, []string{"WhenAPlayerCastsASpell(SinglePlayer(You), AnySpell)"}},
		{"Glademuse", []string{"WhenAPlayerCastsASpell(AnyPlayer, AnySpell)"}, []string{"WhenAPlayerCastsASpell(SinglePlayer(You), AnySpell)"}},
		{"Roiling Vortex", []string{"WhenAPlayerCastsASpell(AnyPlayer, AnySpell)"}, []string{"WhenAPlayerCastsASpell(SinglePlayer(You), AnySpell)"}},
		{"Scytheclaw Raptor", []string{"WhenAPlayerCastsASpell(AnyPlayer, AnySpell)"}, []string{"WhenAPlayerCastsASpell(SinglePlayer(You), AnySpell)"}},
		{"Vision, Synthezoid Avenger", []string{"WhenAPlayerCastsASpell(AnyPlayer, AnySpell)"}, []string{"WhenAPlayerCastsASpell(SinglePlayer(You), AnySpell)"}},
		{"Explicit Subtype Creature Cost", []string{"IsCreatureType(Wizard), IsCardtype(Creature), ControlledByAPlayer(SinglePlayer(You))"}, nil},
		{"Explicit Plural Subtype Creature Cost", []string{"IsCreatureType(Wizard), IsCardtype(Creature), ControlledByAPlayer(SinglePlayer(You))"}, nil},
		{"Yavimaya Kavu", []string{"CDA_Power(", "CDA_Toughness("}, []string{"CDA_PowerToughness("}},
		{"Ana Battlemage", []string{"Kicker(PayMana([ManaCostGeneric(2), ManaCostU])), Kicker(PayMana([ManaCostGeneric(1), ManaCostB]))"}, []string{"Kicker(Or("}},
		{"Stronghold Arena", []string{"Kicker(PayMana([ManaCostG])), Kicker(PayMana([ManaCostW]))"}, []string{"Kicker(Or("}},
		{"Wastescape Battlemage", []string{"Kicker(PayMana([ManaCostG])), Kicker(PayMana([ManaCostGeneric(1), ManaCostU]))"}, []string{"Kicker(Or("}},
		{"Maro", []string{"CDA_PowerToughness(TheNumberOfCardsInPlayersHand(You), TheNumberOfCardsInPlayersHand(You))"}, []string{"CDA_Power(", "CDA_Toughness("}},
		{"Tarmogoyf", []string{"CDA_PowerToughness(TheNumberOfCardtypesAmongGraveyardCards(AnyCardInAnyGraveyard), Plus(TheNumberOfCardtypesAmongGraveyardCards(AnyCardInAnyGraveyard), Integer(1)))"}, []string{"CDA_Power(", "CDA_Toughness("}},
		{"Ajani Goldmane", []string{"CDA_PowerToughness(LifeTotalOfPlayer(You), LifeTotalOfPlayer(You))"}, []string{"CDA_Power(", "CDA_Toughness("}},
		{"The Jolly Balloon Man", []string{"AddAbility([Flying]), AddAbility([Haste])"}, nil},
		{"Gideon Blackblade", []string{"ChooseAnAbility([Vigilance, Lifelink, Indestructible])"}, nil},
		{"Mechtitan Core", []string{"[Flying, Vigilance, Trample, Lifelink, Haste]"}, nil},
		{"Incremental Growth", []string{"NumberCountersOfTypeOnPermanent(Integer(3), PTCounter(+1, +1), Ref_TargetPermanent3)"}, nil},
		{"Incremental Blight", []string{"NumberCountersOfTypeOnPermanent(Integer(3), PTCounter(-1, -1), Ref_TargetPermanent3)"}, nil},
		{"Brainstorm", []string{"DrawNumberCards(Integer(3)), PutNumCardsFromHandOnTopOfLibraryAnyOrder(Integer(2))"}, nil},
		{"Brainsurge", []string{"DrawNumberCards(Integer(4)), PutNumCardsFromHandOnTopOfLibraryAnyOrder(Integer(2))"}, nil},
		{"Smelting Vat", []string{"RevealTheTopNumberCardsOfLibrary(Integer(8)", "MayPutUptoNumberGroupCardsOntoTheBattlefield(Integer(2)"}, nil},
		{"Alchemist's Talent", []string{"ClassAbility([ManaCostGeneric(1), ManaCostR]", "ClassAbility([ManaCostGeneric(4), ManaCostR]"}, nil},
		{"Valki, God of Lies // Tibalt, Cosmic Impostor", []string{"BackFace: (Name: \"Tibalt, Cosmic Impostor\"", "ManaCost: [ManaCostGeneric(5), ManaCostB, ManaCostR]"}, nil},
		{"Cosima, God of the Voyage // The Omenkeel", []string{"ManaCost: [ManaCostGeneric(1), ManaCostU]", "CardPT: (Power: 3, Toughness: 3)"}, nil},
		{"Act of Treason", []string{"UntapPermanent(Ref_TargetPermanent), CreatePermanentLayerEffectUntil(Ref_TargetPermanent, [AddAbility([Haste])]"}, nil},
		{"Nim Deathmantle", []string{"AdjustPT(+2, +2)", "MayCost(PayMana([ManaCostGeneric(4)]))"}, []string{"MayCost(SacrificePermanent(ThisPermanent))"}},
		{"Agent's Toolkit", []string{"EntersWithACounter(ShieldCounter)"}, nil},
		{"Eladamri", []string{"NextAmountOfDamageThatWouldBeDealtToRecipient(Integer(1), Permanent(Ref_TargetPermanent))"}, []string{"NextTimeDamageWouldBeDealtToRecipient"}},
		{"Vassal's Duty", []string{"NextAmountOfDamageThatWouldBeDealtThisTurnToRecipient(Integer(1), Permanent(Ref_TargetPermanent))"}, []string{"NextTimeDamageWouldBeDealtThisTurnToRecipient"}},
		{"War Report", []string{"Plus(TheNumberOfPermanentsOnTheBattlefield(IsCardtype(Creature)), TheNumberOfPermanentsOnTheBattlefield(IsCardtype(Artifact)))"}, nil},
		{"Zaffai, Thunder Conductor", []string{"Scry(Integer(1))", "PermanentDealsDamage(ThisPermanent, Integer(10)"}, nil},
		{"Riding the Dilu Horse", []string{"AdjustPT(+2, +2), AddAbility([Horsemanship])"}, []string{"RegeneratePermanent"}},
		{"Crumb and Get It", []string{"PlayerAction(TheGiftedPlayer, CreateTokens([FoodToken], []))", "ControlledByAPlayer(SinglePlayer(You))", "[AdjustPT(+2, +2)], UntilEndOfTurn), CreatePermanentLayerEffectUntil(Ref_TargetPermanent, [AddAbility([Indestructible])]"}, []string{"PlayerAction(TheGiftedPlayer, DrawACard)"}},
		{"Clavileño, First of the Blessed", []string{"[AddCreatureType(Demon)]), CreatePermanentLayerEffect(Ref_TargetPermanent, [AddAbility(["}, []string{"AddAbilityAndLoseAllOtherAbilities"}},
		{"Kytheon's Tactics", []string{"[AdjustPT(+2, +1)], UntilEndOfTurn), If("}, nil},
		{"Teyo, Aegis Adept", []string{"[SetPower(ToughnessOfPermanent(ActionPermanent))]), CreatePerpetualPermanentEffect(Ref_TargetPermanent, [AddAbility(["}, nil},
		{"Puppeteer Clique", []string{"[AddAbility([Haste])]), CreateFutureTrigger(AtTheBeginningOfPlayersNextEndStep(You)"}, nil},
		{"Lazav, Dimir Mastermind", []string{"AddAbility([Hexproof]), AddAbilityVariable(ThisAbility)"}, nil},
		{"Breathstealer's Crypt", []string{"AnyPlayerMayCost(SinglePlayer(WouldDrawACard_ThatPlayer), PayLife(Integer(3))), Unless(CostWasPaid, [DiscardTheCardDrawnThisWay])"}, nil},
		{"Thundermane Dragon", []string{"MayCastSpellsFromTopOfLibraryWithEffect(And([IsCardtype(Creature), PowerIs(GreaterThanOrEqualTo(Integer(4)))])"}, []string{"IsNonCardtype(Creature)"}},
		{"Farrelite Priest", []string{"TimesThisAbilityHasBeenActivatedThisTurnIs(GreaterThanOrEqualTo(Integer(4)))"}, []string{"TimesThisAbilityHasResolvedThisTurnIs"}},
		{"Starving Revenant", []string{"LoseLifeForEach(Integer(3), TheNumberOfCardsSurveilledOnTopOfLibraryThisWay)"}, []string{"TheNumberOfCardsScryedOnTopOfLibraryThisWay"}},
		{"Ogre Chitterlord", []string{"If(PlayerPassesFilter(You, ControlsNum(GreaterThanOrEqualTo(Integer(5)), IsCreatureType(Rat)))", "[AdjustPT(+2, +0)]"}, nil},
		{"Primitive Justice", []string{"EachAdditionalManaCostPaid([ManaCostGeneric(1), ManaCostG])", "GainLife(Multiply(Integer(1), EachAdditionalManaCostPaid([ManaCostGeneric(1), ManaCostG])))"}, nil},
		{"Summon: Magus Sisters", []string{"GainLife(Integer(3))", "UptoOneTargetPermanent(And([IsCardtype(Creature), ControlledByAPlayer(Opponent)]))"}, nil},
		{"Phyrexian Portal", []string{"ActionList([If(", "LookAtTheTopNumberCardsOfPlayersLibrary(You, Integer(10)"}, nil},
		{"The Irencrag", []string{"MayCost(CreatePermanentLayerEffect", "If(CostWasPaid, [CreatePermanentLayerEffect(ThisPermanent, [AddAbilityAndLoseAllOtherAbilities"}, nil},
	}

	// Every explicitly named GitHub example is parsed, including follow-up comments.
	// Representative cards also assert the intended numbers, targets, zones and verbs.
	githubData, err := os.ReadFile("../data/github_issue_regressions.json")
	if err != nil {
		t.Fatal(err)
	}
	var githubCases []struct {
		Name     string
		Contains []string
		Excludes []string
	}
	if err := json.Unmarshal(githubData, &githubCases); err != nil {
		t.Fatal(err)
	}
	for _, entry := range githubCases {
		cases = append(cases, struct {
			name     string
			contains []string
			excludes []string
		}{entry.Name, entry.Contains, entry.Excludes})
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			card, ok := cards[test.name]
			if !ok {
				t.Fatalf("missing corpus card %q", test.name)
			}
			info, err := ParseCard(english, englishTree, mtgish, mtgishTree, shortNames, card)
			if err != nil {
				t.Fatal(err)
			}
			for _, expected := range test.contains {
				if !strings.Contains(info.MtgishText, expected) {
					t.Errorf("missing %q in %s", expected, info.MtgishText)
				}
			}
			for _, excluded := range test.excludes {
				if strings.Contains(info.MtgishText, excluded) {
					t.Errorf("unexpected %q in %s", excluded, info.MtgishText)
				}
			}
		})
	}
}
