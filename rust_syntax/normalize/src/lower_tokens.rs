// Included in the typed source adapter so token bodies share ordinary ability,
// query, cost, and expression lowering, rather than a second semantic language.
fn predefined_tokens() -> std::result::Result<
    &'static std::collections::BTreeMap<String, source::CreatableToken>,
    Diagnostic,
> {
    static RECIPES: std::sync::OnceLock<
        std::result::Result<std::collections::BTreeMap<String, source::CreatableToken>, String>,
    > = std::sync::OnceLock::new();
    RECIPES
        .get_or_init(|| {
            let mut tokens = std::collections::BTreeMap::new();
            for (i, line) in include_str!(concat!(env!("OUT_DIR"), "/token_definitions.ronl"))
                .lines()
                .enumerate()
            {
                let source::OracleCard::TokenDefinition { name, token } = ron::from_str(line)
                    .map_err(|e| format!("predefined-token record {}: {e}", i + 1))?
                else {
                    return Err("predefined-token catalog contains another record layout".into());
                };
                if tokens.insert(name, token).is_some() {
                    return Err("duplicate predefined-token definition".into());
                }
            }
            Ok(tokens)
        })
        .as_ref()
        .map_err(|message| invalid(message))
}

impl Context {
    fn copy_adjustments(&self, effects: &source::TokenCopyEffects) -> Result<Vec<Operand>> {
        let inspected = crate::inspect::value(effects)
            .map_err(|_| invalid("copy adjustments do not serialize"))?;
        match self.term("TokenCopyEffects", &inspected) {
            Some(term) => Ok(vec![term]),
            None => Err(unsupported(effects)),
        }
    }

    fn token(&self, value: &source::CreatableToken, depth: usize) -> Result<TokenSpec> {
        use source::CreatableToken as S;
        if depth > 64 {
            return Err(invalid("recursive token definition"));
        }
        let named_recipe = match value {
            S::BloodToken => Some("Blood Token"),
            S::ClueToken => Some("Clue Token"),
            S::FishToken => Some("Fish Token"),
            S::FoodToken => Some("Food Token"),
            S::GoldToken => Some("Gold Token"),
            S::JunkToken => Some("Junk Token"),
            S::LanderToken => Some("Lander Token"),
            S::MapToken => Some("Map Token"),
            S::MutagenToken => Some("Mutagen Token"),
            S::OctopusToken => Some("Octopus Token"),
            S::PowerstoneToken => Some("Powerstone Token"),
            S::IncubatorToken => Some("Incubator Token"),
            S::ShardToken => Some("Shard Token"),
            S::TreasureToken => Some("Treasure Token"),
            S::VibraniumToken => Some("Vibranium Token"),
            S::WalkerToken => Some("Walker Token"),
            S::HeartwoodToken => Some("Heartwood Token"),
            S::VirtuousRoleToken => Some("Virtuous Role Token"),
            S::WickedRoleToken => Some("Wicked Role Token"),
            S::YoungHeroRoleToken => Some("Young Hero Role Token"),
            S::CursedRoleToken => Some("Cursed Role Token"),
            S::MonsterRoleToken => Some("Monster Role Token"),
            S::RoyalRoleToken => Some("Royal Role Token"),
            S::SorcererRoleToken => Some("Sorcerer Role Token"),
            S::OracleToken(name) => Some(name.as_str()),
            _ => None,
        };
        let copy = |recipe| TokenSpec {
            amount: Expr::Constant(1),
            words: TokenWords::CreatingEffect,
            recipe,
        };
        match value {
            S::TokenCopyOfCommander(effects) => {
                return Ok(copy(TokenRecipe::Copy {
                    object: ObjectRef::Primitive {
                        name: "CreatableToken::TokenCopyOfCommander".into(),
                        operands: vec![],
                    },
                    adjustments: self.copy_adjustments(effects)?,
                }))
            }
            S::TokenCopyOfEachCardInLibrary(cards, effects) => {
                return Ok(copy(TokenRecipe::CopyEach {
                    objects: self.private_cards(
                        EventZone::Library,
                        PlayerSet::All,
                        self.library_filter(cards)?,
                    ),
                    adjustments: self.copy_adjustments(effects)?,
                }))
            }
            S::TokenCopyOfPermanent(object, effects) => {
                return Ok(copy(TokenRecipe::Copy {
                    object: self.object(object)?,
                    adjustments: self.copy_adjustments(effects)?,
                }))
            }
            S::TokenCopyOfExiledCard(card, effects) => {
                return Ok(copy(TokenRecipe::Copy {
                    object: self.exiled_card(card)?,
                    adjustments: self.copy_adjustments(effects)?,
                }))
            }
            S::TokenCopyOfGraveyardCard(card, effects) => {
                return Ok(copy(TokenRecipe::Copy {
                    object: self.grave_card(card)?,
                    adjustments: self.copy_adjustments(effects)?,
                }))
            }
            S::TokenCopyOfDiscardedCard(card, effects) => {
                return Ok(copy(TokenRecipe::Copy {
                    object: self.hand_card(card)?,
                    adjustments: self.copy_adjustments(effects)?,
                }))
            }
            S::TokenCopyOfSpell(spell, effects) => {
                return Ok(copy(TokenRecipe::Copy {
                    object: self.spell(spell)?,
                    adjustments: self.copy_adjustments(effects)?,
                }))
            }
            S::TokenCopyOfEachPermanent(objects, effects) => {
                return Ok(copy(TokenRecipe::CopyEach {
                    objects: self.objects(objects)?,
                    adjustments: self.copy_adjustments(effects)?,
                }))
            }
            S::TokenCopyOfEachExiledCard(cards, effects) => {
                return Ok(copy(TokenRecipe::CopyEach {
                    objects: self.exile_cards(cards)?,
                    adjustments: self.copy_adjustments(effects)?,
                }))
            }
            S::TokenCopyOfEachGraveyardCard(cards, effects) => {
                return Ok(copy(TokenRecipe::CopyEach {
                    objects: self.grave_cards(cards)?,
                    adjustments: self.copy_adjustments(effects)?,
                }))
            }
            S::TokenCopyOfAPermanent(objects, effects) => {
                return Ok(copy(TokenRecipe::CopyOne {
                    objects: self.objects(objects)?,
                    adjustments: self.copy_adjustments(effects)?,
                }))
            }
            S::TokenCopyOfAnExiledCard(cards, effects) => {
                return Ok(copy(TokenRecipe::CopyOne {
                    objects: self.exile_cards(cards)?,
                    adjustments: self.copy_adjustments(effects)?,
                }))
            }
            S::TokenCopyOfNamedCard(name, effects) => {
                return Ok(copy(TokenRecipe::CopyNamed {
                    name: name.clone(),
                    adjustments: self.copy_adjustments(effects)?,
                }))
            }
            S::TokenCopyOfACardAtRandom(cards) => {
                let inspected = crate::inspect::value(cards)
                    .map_err(|_| invalid("oracle filter does not serialize"))?;
                let Some(cards) = self.term("CardsInOracle", &inspected) else {
                    return Err(unsupported(value));
                };
                return Ok(copy(TokenRecipe::CopyAtRandom { cards }));
            }
            _ => {}
        }
        if let S::OracleToken(name) = value {
            if !predefined_tokens()?.contains_key(name) {
                return Ok(TokenSpec {
                    amount: Expr::Constant(1),
                    words: TokenWords::Rules,
                    recipe: TokenRecipe::Named(name.clone()),
                });
            }
        }
        if let Some(name) = named_recipe {
            let recipe = predefined_tokens()?
                .get(name)
                .ok_or_else(|| invalid("missing parsed predefined-token recipe"))?;
            let mut token = self
                .token(recipe, depth + 1)
                .map_err(|e| e.at(format!("predefined[{name:?}]")))?;
            token.words = TokenWords::Rules;
            return Ok(token);
        }
        match value {
            S::NumberTokens(n, inner) | S::NumberTokensForEach(n, _, inner) => {
                let mut token = self.token(inner, depth + 1)?;
                let mut amount = self.number(n)?;
                if let S::NumberTokensForEach(_, each, _) = value {
                    amount = Expr::multiply(amount, self.number(each)?);
                }
                token.amount = Expr::multiply(amount, token.amount);
                return Ok(token);
            }
            _ => {}
        }
        let (name, pt, colors, supertypes, card_types, subtypes, rules) = match value {
            S::Token(c, s, t, sub, rules) => (TokenName::FromSubtypes, None, c, s, t, sub, rules),
            S::TokenWithPT(pt, c, s, t, sub, rules) => {
                (TokenName::FromSubtypes, Some(pt), c, s, t, sub, rules)
            }
            S::NamedToken(name, c, s, t, sub, rules) => {
                (TokenName::Explicit(name.clone()), None, c, s, t, sub, rules)
            }
            S::NamedTokenWithPT(name, pt, c, s, t, sub, rules)
            | S::NamedTokenWithPTAndWithCopyEffects(
                name,
                pt,
                c,
                s,
                t,
                sub,
                rules,
                source::TokenCopyEffects::NoTokenCopyEffects,
            ) => (
                TokenName::Explicit(name.clone()),
                Some(pt),
                c,
                s,
                t,
                sub,
                rules,
            ),
            _ => return Err(unsupported(value)),
        };
        let chosen_color = matches!(colors, source::TokenColorList::TheChosenColor);
        let chosen_creature_type = matches!(subtypes, source::TokenSubtypes::TheChosenCreatureType);
        let colors = match colors {
            source::TokenColorList::Colorless | source::TokenColorList::TheChosenColor => {
                Vec::new()
            }
            source::TokenColorList::AllColors => vec![
                ManaColor::White,
                ManaColor::Blue,
                ManaColor::Black,
                ManaColor::Red,
                ManaColor::Green,
            ],
            source::TokenColorList::Colors(colors) => colors
                .iter()
                .map(|c| match c {
                    source::SimpleColor::White => ManaColor::White,
                    source::SimpleColor::Blue => ManaColor::Blue,
                    source::SimpleColor::Black => ManaColor::Black,
                    source::SimpleColor::Red => ManaColor::Red,
                    source::SimpleColor::Green => ManaColor::Green,
                })
                .collect(),
        };
        let subtypes = match subtypes {
            source::TokenSubtypes::SubtypesList(subtypes) => subtypes.clone(),
            source::TokenSubtypes::TheChosenCreatureType => vec![],
            source::TokenSubtypes::AllBasicLandTypes => vec![
                source::SubType::Plains,
                source::SubType::Island,
                source::SubType::Swamp,
                source::SubType::Mountain,
                source::SubType::Forest,
            ],
        };
        let power_toughness = pt
            .map(|pt| -> Result<_> {
                Ok(match pt {
                    source::PT::PT(p, t) => {
                        (Expr::Constant((*p).into()), Expr::Constant((*t).into()))
                    }
                    source::PT::ManualPT(p, t) => (self.number(p)?, self.number(t)?),
                    source::PT::PTX(p, t, n) => {
                        let n = self.number(n)?;
                        let component = |v: &source::PTXValue| match v {
                            source::PTXValue::Integer(n) => Expr::Constant((*n).into()),
                            source::PTXValue::X => n.clone(),
                        };
                        (component(p), component(t))
                    }
                })
            })
            .transpose()?;
        let abilities = rules
            .iter()
            .enumerate()
            .map(|(i, rule)| {
                self.fresh()
                    .ability(rule)
                    .map_err(|e| e.at(format!("definition.abilities[{i}]")))
            })
            .collect::<Result<_>>()?;
        Ok(TokenSpec {
            amount: Expr::Constant(1),
            words: TokenWords::CreatingEffect,
            recipe: TokenRecipe::Defined(TokenDefinition {
                name,
                colors,
                chosen_color,
                chosen_creature_type,
                supertypes: supertypes.clone(),
                card_types: card_types.clone(),
                subtypes,
                power_toughness,
                abilities,
            }),
        })
    }

    fn token_entry(&self, flags: &[source::TokenFlag]) -> Result<EntrySpec> {
        let mut entry = EntrySpec::default();
        for (i, flag) in flags.iter().enumerate() {
            match flag {
                source::TokenFlag::EntersTapped => entry.tapped = true,
                source::TokenFlag::EntersWithACounter(kind) => entry
                    .counters
                    .push((crate::counter::lower(kind), Expr::Constant(1))),
                source::TokenFlag::EntersWithNumberCounters(amount, kind) => entry
                    .counters
                    .push((crate::counter::lower(kind), self.number(amount)?)),
                source::TokenFlag::EntersAttacking => entry.states.push(EntryState::Attacking {
                    player: None,
                    or_planeswalker: false,
                }),
                source::TokenFlag::EntersAttackingPlayer(p) => {
                    entry.states.push(EntryState::Attacking {
                        player: Some(self.player(p)?),
                        or_planeswalker: false,
                    })
                }
                source::TokenFlag::EntersAttackingPlayerOrPlaneswalkerControlledBy(p) => {
                    entry.states.push(EntryState::Attacking {
                        player: Some(self.player(p)?),
                        or_planeswalker: true,
                    })
                }
                source::TokenFlag::EntersBlockingAttacker(p) => {
                    entry.states.push(EntryState::Blocking(self.object(p)?))
                }
                source::TokenFlag::EntersAttachedToPermanent(p) => {
                    entry.states.push(EntryState::AttachedTo(self.object(p)?))
                }
                source::TokenFlag::EntersAttachedToAPermanent(p) => entry
                    .states
                    .push(EntryState::AttachedToChosen(self.filter(p)?)),
                source::TokenFlag::EntersAttachedToPlayer(p) => entry
                    .states
                    .push(EntryState::AttachedToPlayer(self.player(p)?)),
                _ => {
                    let (name, operands) = self
                        .primitive("TokenFlag", flag)
                        .ok_or_else(|| unsupported(flag).at(format!("entry[{i}]")))?;
                    entry.states.push(EntryState::Primitive { name, operands });
                }
            }
        }
        Ok(entry)
    }
}
