// "You may play or cast" reads as one permission plus terms, as Forge's `MayPlay`
// does. A form's name lists those terms, so it is parsed from the name; a form with
// any word this does not know stays with its row, or unsupported, and drops nothing.
const PLAY_ACTIONS: [(&str, CardPlayAction, Option<i64>); 14] = [
    ("PlayALandOrCastASpell", CardPlayAction::Either, Some(1)),
    ("PlayOneCard", CardPlayAction::Either, Some(1)),
    ("PlayTwoCards", CardPlayAction::Either, Some(2)),
    ("PlayExiledCards", CardPlayAction::Either, None),
    ("PlayExiledCard", CardPlayAction::Either, None),
    ("PlayGraveyardCard", CardPlayAction::Either, None),
    ("PlayCards", CardPlayAction::Either, None),
    ("PlayLands", CardPlayAction::PlayLand, None),
    ("CastASpell", CardPlayAction::CastSpell, Some(1)),
    ("CastOneSpell", CardPlayAction::CastSpell, Some(1)),
    ("CastSpells", CardPlayAction::CastSpell, None),
    ("CastExiledCards", CardPlayAction::CastSpell, None),
    ("CastExiledCard", CardPlayAction::CastSpell, None),
    ("CastGraveyardCard", CardPlayAction::CastSpell, None),
];

const PLAY_ZONES: [(&str, Option<EventZone>); 12] = [
    ("FromAmongExiledCards", Some(EventZone::Exile)),
    ("FromAmongExiled", Some(EventZone::Exile)),
    ("FromAmongExile", Some(EventZone::Exile)),
    ("FromAmongCardsInPlayersGraveyard", Some(EventZone::Graveyard)),
    ("FromAmongCardsInGraveyards", Some(EventZone::Graveyard)),
    ("FromTheirGraveyard", Some(EventZone::Graveyard)),
    ("FromGraveyard", Some(EventZone::Graveyard)),
    ("FromTopOfLibrary", Some(EventZone::Library)),
    ("FromHand", Some(EventZone::Hand)),
    ("FromAmongCardsMilledThisWay", None),
    ("MilledThisWay", None),
    ("", None),
];

const PLAY_TERMS: [&str; 18] = [
    "IntoExile",
    "WithoutPaying",
    "ForAlternateCastingCost",
    "ForAlternateCost",
    "AndPayAlternateCostToCast",
    "ForAdditionalCost",
    "WithAdditionalCost",
    "AndMaySpendManaAsThoughAnyColorToCast",
    "AndMaySpendManaAsThoughAnyTypeToCast",
    "OnceDuringEachPlayersTurn",
    "OnceEachPlayersTurn",
    "OnceEachTurn",
    "EachTurn",
    "Once",
    "WithEnterActions",
    "WithEffect",
    "WithTrigger",
    "If",
];

impl Context {
    fn play_rule(&self, value: &source::PlayerEffect) -> Option<Result<PlayerRule>> {
        let inspected = crate::inspect::value(value).ok()?;
        let name = inspected["variant"].as_str()?.strip_prefix("May")?;
        let (word, action, limit) = PLAY_ACTIONS.iter().find(|a| name.starts_with(a.0))?;
        let rest = &name[word.len()..];
        let (word, zone) = PLAY_ZONES.iter().find(|z| rest.starts_with(z.0))?;
        let mut words_left = &rest[word.len()..];
        let mut words = vec![];
        while !words_left.is_empty() {
            let term = PLAY_TERMS.iter().find(|t| words_left.starts_with(**t))?;
            words.push(*term);
            words_left = &words_left[term.len()..];
        }
        let args = match &inspected["args"] {
            serde_json::Value::Null => vec![],
            serde_json::Value::Array(items) => items.clone(),
            other => vec![other.clone()],
        };
        let milled = zone.is_none() && !word.is_empty();
        Some(self.play_terms(action.clone(), zone.clone(), milled, *limit, &words, &args))
    }

    fn play_terms(
        &self,
        action: CardPlayAction,
        zone: Option<EventZone>,
        milled: bool,
        limit: Option<i64>,
        words: &[&str],
        args: &[serde_json::Value],
    ) -> Result<PlayerRule> {
        let read = |value: &serde_json::Value| {
            self.bound_operand(value).ok_or_else(|| unsupported(value))
        };
        let has = |word: &str| words.contains(&word);
        let each_turn = ["OnceDuringEachPlayersTurn", "OnceEachPlayersTurn", "OnceEachTurn", "EachTurn"]
            .iter()
            .any(|w| has(w));
        let mut terms = PlayTerms {
            limit: match limit {
                Some(n) if each_turn => Some(PlayLimit::EachTurn(n)),
                Some(n) => Some(PlayLimit::Total(n)),
                None => each_turn.then_some(PlayLimit::EachTurn(1)),
            },
            during: None,
            cost: if has("WithoutPaying") {
                PlayCost::Free
            } else {
                PlayCost::Normal
            },
            spend_as: if has("AndMaySpendManaAsThoughAnyColorToCast") {
                Some(SpendAs::AnyColor)
            } else if has("AndMaySpendManaAsThoughAnyTypeToCast") {
                Some(SpendAs::AnyType)
            } else {
                None
            },
            into_exile: has("IntoExile"),
            condition: None,
            effects: vec![],
            flash: false,
            may_look: false,
            then: None,
            face_down: None,
        };
        let alternate = has("ForAlternateCastingCost")
            || has("ForAlternateCost")
            || has("AndPayAlternateCostToCast");
        let additional = has("ForAdditionalCost") || has("WithAdditionalCost");
        let turns = words.iter().any(|w| w.contains("PlayersTurn"));
        let mut filters = vec![];
        let mut cards = None;
        let mut from = zone.clone().unwrap_or(EventZone::Graveyard);
        let mut grave_filter = ObjectFilter::Any;
        let mut owners = PlayerSet::One(PlayerRef::Affected);
        for arg in args {
            let kind = arg["enum"].as_str().unwrap_or_default();
            match kind {
                "Spells" => filters.push(self.spell_filter(&decode_step(arg)?)?),
                "Permanents" => filters.push(self.filter(&decode_step(arg)?)?),
                "Cards" => filters.push(self.card_filter(&decode_step(arg)?)?),
                "CardsInExile" => {
                    from = EventZone::Exile;
                    cards = Some(self.exile_cards(&decode_step(arg)?)?);
                }
                "CardInExile" => {
                    from = EventZone::Exile;
                    cards = Some(ObjectSet::One(self.exiled_card(&decode_step(arg)?)?));
                }
                "CardsInGraveyards" => grave_filter = self.grave_filter(&decode_step(arg)?)?,
                "CardInGraveyards" => {
                    cards = Some(ObjectSet::One(self.grave_card(&decode_step(arg)?)?))
                }
                "Player" if turns => {
                    terms.during = Some(PlayerSet::One(self.player(&decode_step(arg)?)?))
                }
                "Player" => owners = PlayerSet::One(self.player(&decode_step(arg)?)?),
                "Players" if turns => terms.during = Some(self.players(&decode_step(arg)?)?),
                "Players" => owners = self.players(&decode_step(arg)?)?,
                "Cost" if alternate => terms.cost = PlayCost::Alternate(read(arg)?),
                "Cost" if additional => terms.cost = PlayCost::Additional(read(arg)?),
                "Condition" => terms.condition = Some(self.condition(&decode_step(arg)?)?),
                _ if additional && arg[0]["enum"] == "Action" => {
                    terms.cost = PlayCost::Additional(read(arg)?)
                }
                _ if arg.is_array() || kind == "Actions" => terms.effects.push(read(arg)?),
                _ => return Err(unsupported(arg)),
            }
        }
        let cards = match (cards, zone) {
            (Some(cards), _) => cards,
            (None, Some(EventZone::Library)) => {
                self.library_slice(PlayerRef::Affected, Expr::Constant(1), SequenceEnd::Top)
            }
            (None, Some(zone)) => self.private_cards(zone, owners, grave_filter),
            (None, None) if milled => self.this_way_set(Done::Milled, ObjectPhase::After, None)?,
            (None, None) => return Err(unsupported(&args)),
        };
        let filter = match filters.len() {
            0 => ObjectFilter::Any,
            1 => filters.remove(0),
            _ => ObjectFilter::AnyOf(filters),
        };
        Ok(PlayerRule::PlayPermissionWith {
            permission: Box::new(PlayerRule::PlayPermission {
                cards,
                from,
                action,
                filter,
            }),
            terms,
        })
    }
}
