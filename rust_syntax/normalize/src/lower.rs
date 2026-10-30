//! Lower legacy Rust syntax into semantic operations. A record is complete only
//! when every part lowers; unsupported forms are reported with an AST path.

use crate::{ir::*, mtg_types as source};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Diagnostic {
    pub path: String,
    pub source_type: String,
    pub form: String,
    pub message: String,
}

impl Diagnostic {
    fn at(mut self, path: impl AsRef<str>) -> Self {
        self.path = if self.path.is_empty() {
            path.as_ref().into()
        } else {
            format!("{}.{}", path.as_ref(), self.path)
        };
        self
    }
}

type Result<T> = std::result::Result<T, Diagnostic>;

fn unsupported(value: &impl Serialize) -> Diagnostic {
    let inspected = crate::inspect::value(value).expect("source AST serialization");
    Diagnostic {
        path: String::new(),
        source_type: inspected["enum"].as_str().unwrap_or("value").into(),
        form: inspected["variant"].as_str().unwrap_or("value").into(),
        message: "semantic lowering has not been implemented for this form".into(),
    }
}

fn invalid(message: &str) -> Diagnostic {
    Diagnostic {
        path: String::new(),
        source_type: "invariant".into(),
        form: "invalid".into(),
        message: message.into(),
    }
}

#[derive(Default, Clone)]
struct Context {
    prepared: crate::pipeline::PreparedRecipes,
    targets: Vec<TargetSpec>,
    modes: Option<ModeSelection>,
    result: u32,
    affected: bool,
    keyword_host: bool,
    candidate: bool,
    event: crate::semantics::EventBindings,
    created_tokens: Option<ResultId>,
    payment: Option<ResultId>,
    paid: std::collections::BTreeMap<crate::number_spec::PaymentAction, Option<PaidBinding>>,
    announcing: bool,
    you: Option<PlayerRef>,
    payer: Option<PlayerRef>,
    cast_x: Option<ObjectRef>,
    binding_depth: u32,
    state_subject: Option<ObjectRef>,
    exiled: Option<ResultId>,
    exiled_permanents: bool,
    ambiguous_damage_permanent: bool,
    damage_permanent_recipient: bool,
    entered: Option<ResultId>,
    open_searches: Vec<PlayerRef>,
    inspection_hand: Option<Vec<ResultId>>,
}

impl Context {
    fn fresh(&self) -> Self {
        Self {
            prepared: self.prepared.clone(),
            ..Self::default()
        }
    }
    fn target(&self, kind: TargetKind, ordinal: usize, element: u16) -> Result<Slot> {
        let found = self
            .targets
            .iter()
            .filter(|t| t.kind == kind)
            .nth(ordinal)
            .ok_or_else(|| invalid("reference has no target slot in this ability"))?;
        if let Count::Exactly(Expr::Constant(n)) | Count::UpTo(Expr::Constant(n)) = &found.count {
            if i64::from(element) >= *n {
                return Err(invalid("reference exceeds target slot cardinality"));
            }
        }
        Ok(found.slot.clone())
    }

    fn effect(&mut self, operation: Operation) -> Instruction {
        // A later ordinary action of the same kind shadows the shorthand.
        // Until its own event-history binder exists, fail rather than silently
        // referring back to an unrelated activation/payment component.
        use crate::number_spec::PaymentAction;
        let shadow = match &operation {
            Operation::Sacrifice { .. } => Some(PaymentAction::Sacrifice),
            Operation::Discard { .. } => Some(PaymentAction::Discard),
            Operation::Tap { tapped: true, .. } => Some(PaymentAction::Tap),
            Operation::Tap { tapped: false, .. } => Some(PaymentAction::Untap),
            _ => None,
        };
        if let Some(action) = shadow {
            self.paid.insert(action, None);
        }
        if let Operation::Search { library, .. } = &operation {
            self.open_searches.push(library.clone());
        }
        if let Operation::Shuffle { players, .. } = &operation {
            self.open_searches
                .retain(|p| !crate::semantics::shuffle_closes(p, players));
        }
        if matches!(operation, Operation::CreateTokens { .. }) {
            self.created_tokens = Some(ResultId(self.result));
        }
        if matches!(
            operation,
            Operation::Move {
                destination: Destination::Exile,
                ..
            }
        ) {
            self.exiled = Some(ResultId(self.result));
            self.exiled_permanents = false;
        }
        if matches!(
            operation,
            Operation::Move {
                destination: Destination::Battlefield(_) | Destination::OwnersBattlefield,
                ..
            }
        ) {
            self.entered = Some(ResultId(self.result));
        }
        if let Operation::PayCost { cost, .. } = &operation {
            self.payment = Some(ResultId(self.result));
            self.install_paid_history(cost, PaymentRef::Resolution(ResultId(self.result)));
        }
        let result = ResultId(self.result);
        self.result += 1;
        Instruction::Effect { result, operation }
    }

    fn invalidate_child_bindings(&mut self, child: &Self) {
        for (action, binding) in &child.paid {
            if serde_json::to_value(self.paid.get(action)).unwrap()
                != serde_json::to_value(Some(binding)).unwrap()
            {
                self.paid.insert(*action, None);
            }
        }
        if self
            .inspection_hand
            .as_ref()
            .map(|v| v.iter().map(|r| r.0).collect::<Vec<_>>())
            != child
                .inspection_hand
                .as_ref()
                .map(|v| v.iter().map(|r| r.0).collect::<Vec<_>>())
        {
            self.inspection_hand = None;
        }
        for library in &child.open_searches {
            if !self
                .open_searches
                .iter()
                .any(|p| crate::semantics::same_player(p, library))
            {
                self.open_searches.push(library.clone());
            }
        }
        if self.created_tokens.as_ref().map(|r| r.0) != child.created_tokens.as_ref().map(|r| r.0) {
            self.created_tokens = None;
        }
        if self.payment.as_ref().map(|r| r.0) != child.payment.as_ref().map(|r| r.0) {
            self.payment = None;
        }
        if self.exiled.as_ref().map(|r| r.0) != child.exiled.as_ref().map(|r| r.0) {
            self.exiled = None;
            self.exiled_permanents = false;
        }
        if self.entered.as_ref().map(|r| r.0) != child.entered.as_ref().map(|r| r.0) {
            self.entered = None;
        }
    }

    fn object(&self, value: &source::Permanent) -> Result<ObjectRef> {
        use source::Permanent as S;
        if matches!(value, S::Trigger_ThatPermanent) && self.ambiguous_damage_permanent {
            return Err(invalid("damage trigger needs an explicit source/recipient role for this legacy permanent reference"));
        }
        if matches!(value, S::Trigger_ThatPermanent)
            && self.damage_permanent_recipient
            && self.event.damage_recipient
        {
            return Ok(ObjectRef::Damage(DamageSubject::Recipient));
        }
        if matches!(value, S::Trigger_ThatCreature) && self.event.damage_source_permanent {
            return Ok(ObjectRef::Damage(DamageSubject::Source));
        }
        if matches!(value, S::Trigger_ThatPermanent | S::Trigger_ThatCreature) {
            if let Some(subject) = &self.state_subject {
                return Ok(subject.clone());
            }
        }
        let (ordinal, index) = match value {
            S::ThePermanentSacrificedThisWay
            | S::TheSacrificedPermanent
            | S::ThisSacrificedPermanent => {
                return self.paid_object(crate::number_spec::PaymentAction::Sacrifice)
            }
            S::ThePermanentTappedThisWay => {
                return self.paid_object(crate::number_spec::PaymentAction::Tap)
            }

            S::ThePermanentPutOnTheBattlefieldThisWay => {
                return self.result_object(self.entered.as_ref(), 0)
            }
            S::ThisPermanent | S::ThisPermanentOrThisCommandCard => {
                return Ok(if self.keyword_host {
                    ObjectRef::KeywordHost
                } else {
                    ObjectRef::Source
                })
            }
            S::HostPermanent => return Ok(ObjectRef::AttachedTo(Box::new(ObjectRef::Source))),
            S::HostPermanentOf(p) => return Ok(ObjectRef::AttachedTo(Box::new(self.object(p)?))),
            S::ActionPermanent if self.affected => return Ok(ObjectRef::Affected),
            S::Self_It if self.candidate => return Ok(ObjectRef::Candidate),
            S::Ref_TargetPermanent | S::Ref_TargetPermanent1 | S::Ref_TargetPermanents_0 => (0, 0),
            S::Ref_TargetPermanent2 => (1, 0),
            S::Ref_TargetPermanent3 => (2, 0),
            S::Ref_TargetPermanent4 => (3, 0),
            S::Ref_TargetPermanent5 => (4, 0),
            S::Ref_TargetPermanents_1 => (0, 1),
            S::Trigger_ThatPermanent
            | S::Trigger_ThatCreature
            | S::Trigger_ThatDeadPermanent
            | S::ThatEnteringPermanent
                if self.event.permanent =>
            {
                return Ok(ObjectRef::EventSubject)
            }
            S::Trigger_ThatOtherPermanent
            | S::Trigger_ThatOtherCreature
            | S::Trigger_ThatCreatureOrPlaneswalker
                if self.event.damage_recipient =>
            {
                return Ok(ObjectRef::Damage(DamageSubject::Recipient))
            }
            _ => return Err(unsupported(value)),
        };
        Ok(ObjectRef::Target {
            slot: self.target(
                TargetKind::Object(ObjectDomain::permanent()),
                ordinal,
                index,
            )?,
            index,
        })
    }

    fn player(&self, value: &source::Player) -> Result<PlayerRef> {
        use source::Player as S;
        Ok(match value {
            S::You => self.you.clone().unwrap_or(PlayerRef::AbilityController),
            S::TheActivePlayer => PlayerRef::Active,
            S::HostPlayer => PlayerRef::EnchantedBy(Box::new(ObjectRef::Source)),
            S::ControllerOfPermanent(p) => PlayerRef::ControllerOf(Box::new(self.object(p)?)),
            S::OwnerOfPermanent(p) => PlayerRef::OwnerOf(Box::new(self.object(p)?)),
            S::ControllerOfSpell(s) => PlayerRef::ControllerOf(Box::new(self.spell(s)?)),
            S::OwnerOfSpell(s) => PlayerRef::OwnerOf(Box::new(self.spell(s)?)),
            S::ControllerOfTargetSpell => {
                PlayerRef::ControllerOf(Box::new(self.spell(&source::Spell::Ref_TargetSpell)?))
            }
            S::Ref_TargetPlayer | S::Ref_TargetPlayer1 => PlayerRef::Target {
                slot: self.target(TargetKind::Player, 0, 0)?,
                index: 0,
            },
            S::Ref_TargetPlayer2 => PlayerRef::Target {
                slot: self.target(TargetKind::Player, 1, 0)?,
                index: 0,
            },
            S::Ref_TargetPlayer3 => PlayerRef::Target {
                slot: self.target(TargetKind::Player, 2, 0)?,
                index: 0,
            },
            S::Trigger_ThatPlayer if self.event.player => PlayerRef::EventSubject,
            S::Trigger_ControllerOfThatSource if self.event.damage_source => {
                PlayerRef::ControllerOf(Box::new(ObjectRef::Damage(DamageSubject::Source)))
            }
            S::Trigger_ControllerOfThatPermanent if self.event.permanent => {
                PlayerRef::ControllerOf(Box::new(
                    self.object(&source::Permanent::Trigger_ThatPermanent)?,
                ))
            }
            S::Trigger_ControllerOfThatSpell if self.event.spell => {
                PlayerRef::ControllerOf(Box::new(ObjectRef::EventSubject))
            }
            _ => return Err(unsupported(value)),
        })
    }

    fn players(&self, value: &source::Players) -> Result<PlayerSet> {
        use source::Players as S;
        Ok(match value {
            S::Trigger_ThosePlayers if self.event.damage_players => PlayerSet::DamageRecipients,
            S::Or(values) => PlayerSet::Union(
                values
                    .iter()
                    .map(|p| self.players(p))
                    .collect::<Result<_>>()?,
            ),
            S::AnyPlayer => PlayerSet::All,
            S::SinglePlayer(player) => PlayerSet::One(self.player(player)?),
            S::Opponent => PlayerSet::OpponentsOf(self.player(&source::Player::You)?),
            S::OpponentOf(player) => PlayerSet::OpponentsOf(self.player(player)?),
            S::Other(player) => PlayerSet::OthersThan(self.player(player)?),
            _ => return Err(unsupported(value)),
        })
    }

    fn objects(&self, value: &source::Permanents) -> Result<ObjectSet> {
        if matches!(
            value,
            source::Permanents::ThePermanentsSacrificedThisWay
                | source::Permanents::TheSacrificedPermanents
        ) {
            return self.paid_objects(
                crate::number_spec::PaymentAction::Sacrifice,
                CostObjectView::Performed,
            );
        }
        if matches!(value, source::Permanents::ThePermanentsTappedThisWay) {
            return self.paid_objects(
                crate::number_spec::PaymentAction::Tap,
                CostObjectView::Performed,
            );
        }
        if matches!(
            value,
            source::Permanents::Trigger_ThosePermanents
                | source::Permanents::Trigger_ThoseCreatures
        ) {
            if let Some(role) = self.event.damage_subject {
                if role == DamageSubject::Source || self.event.damage_objects {
                    return Ok(ObjectSet::Damage(role));
                }
            }
        }
        if matches!(value, source::Permanents::TheTokensCreatedThisWay) {
            return self
                .created_tokens
                .clone()
                .map(ObjectSet::Created)
                .ok_or_else(|| invalid("tokens-created reference has no creation in this scope"));
        }
        if let source::Permanents::SinglePermanent(object) = value {
            return Ok(ObjectSet::One(self.object(object)?));
        }
        if matches!(value, source::Permanents::Ref_TargetPermanents) {
            return Ok(ObjectSet::Targets(self.target(
                TargetKind::Object(ObjectDomain::permanent()),
                0,
                0,
            )?));
        }
        Ok(ObjectSet::Battlefield(self.filter(value)?))
    }

    fn battlefield_objects(&self, value: &source::Permanents) -> Result<ObjectSet> {
        let objects = self.objects(value)?;
        Ok(if matches!(objects, ObjectSet::Battlefield(_)) {
            objects
        } else {
            ObjectSet::InZone {
                objects: Box::new(objects),
                zone: EventZone::Battlefield,
            }
        })
    }

    fn comparison(&self, value: &source::Comparison) -> Result<(Comparison, Expr)> {
        use source::Comparison as S;
        let (comparison, number) = match value {
            S::EqualTo(n) => (Comparison::Equal, n),
            S::NotEqualTo(n) => (Comparison::NotEqual, n),
            S::LessThan(n) => (Comparison::Less, n),
            S::LessThanOrEqualTo(n) => (Comparison::LessOrEqual, n),
            S::GreaterThan(n) => (Comparison::Greater, n),
            S::GreaterThanOrEqualTo(n) => (Comparison::GreaterOrEqual, n),
            _ => return Err(unsupported(value)),
        };
        Ok((comparison, self.number(number)?))
    }

    fn filter(&self, value: &source::Permanents) -> Result<ObjectFilter> {
        let mut context = self.clone();
        context.candidate = true;
        context.filter_inner(value)
    }

    fn filter_inner(&self, value: &source::Permanents) -> Result<ObjectFilter> {
        if let Some(result) =
            self.pipeline_selector(crate::pipeline::selectors::SelectorRef::Permanents(value))
        {
            return result;
        }
        self.cast_history_filter(value)
    }

    fn number(&self, value: &source::GameNumber) -> Result<Expr> {
        if !self.prepared.numbers.is_empty() {
            if let Some(quantity) = self
                .prepared
                .numbers
                .get(&crate::pipeline::numbers::number_key(value))
            {
                return self.pipeline_quantity(quantity);
            }
        }
        if let Some(quantity) = crate::pipeline::numbers::rewrite_number(value) {
            return self.pipeline_quantity(&quantity);
        }
        use source::GameNumber as S;
        Ok(match value {
            S::AmountOfManaSpentToCastSpell(spell)
            | S::TheNumberOfColorsOfManaSpentToCastSpell(spell) => Expr::ManaSpent {
                object: self.spell(spell)?,
                filter: SpentManaFilter {
                    colors: None,
                    snow: false,
                },
                aggregation: if matches!(value, S::AmountOfManaSpentToCastSpell(_)) {
                    SpentManaAggregation::Count
                } else {
                    SpentManaAggregation::DistinctColors
                },
            },
            S::ValueX if self.event.activation_x => Expr::EventActivationX,
            S::ValueX => self
                .cast_x
                .clone()
                .map(Expr::CastX)
                .unwrap_or(Expr::AnnouncedX),
            S::Trigger_ThatMuch | S::Trigger_AmountOfDamageDealt if self.event.amount => {
                Expr::EventAmount
            }
            S::Trigger_ValueXOfThatSpell if self.event.spell => Expr::ObjectProperty {
                object: ObjectRef::EventSubject,
                property: Property::SpellX,
                information: Information::CurrentOrLastKnown,
            },
            S::Trigger_AmountOfCreatures if self.event.damage => {
                Expr::CountObjects(Box::new(ObjectSet::Damage(DamageSubject::Source)))
            }
            S::NumPermanentsExiledThisWay => {
                if !self.exiled_permanents {
                    return Err(invalid(
                        "permanent exile count requires an exclusively permanent batch",
                    ));
                }
                Expr::CountObjects(Box::new(ObjectSet::Transition {
                    result: self
                        .exiled
                        .clone()
                        .ok_or_else(|| invalid("exile count has no movement in this scope"))?,
                    phase: ObjectPhase::Before,
                }))
            }
            _ => return self.characteristic_number(value),
        })
    }

    fn condition(&self, value: &source::Condition) -> Result<Condition> {
        if let Some(crate::pipeline::operands::OperandRecipe::Condition(recipe)) =
            self.operand_recipe(crate::pipeline::operands::OperandRef::Condition(value))
        {
            return self.bind_check_recipe(&recipe);
        }

        use source::Condition as S;
        Ok(match value {
            S::CastSpellPassesFilter(filter) => Condition::ObjectMatches {
                object: if self.event.spell {
                    ObjectRef::EventSubject
                } else {
                    ObjectRef::Source
                },
                filter: self.spell_filter(filter)?,
            },
            S::GraveyardCardPassesFilter(card, filter) => Condition::ObjectMatches {
                object: self.grave_card(card)?,
                filter: self.card_filter(filter)?,
            },
            S::EnteringPermanentPassesFilter(filter) => Condition::ObjectMatches {
                object: if self.event.permanent {
                    ObjectRef::EventSubject
                } else {
                    self.result_object(self.entered.as_ref(), 0)?
                },
                filter: self.filter(filter)?,
            },
            S::TheCardExiledThisWayIsStillExiled => Condition::Compare {
                left: Expr::CountObjects(Box::new(self.still_exiled(ObjectSet::One(
                    self.exiled_card(&source::CardInExile::TheCardExiledThisWay)?,
                )))),
                comparison: Comparison::Greater,
                right: Expr::Constant(0),
            },
            S::AnExiledCardPassesFilter(cards, filter) => Condition::Compare {
                left: Expr::CountObjects(Box::new(ObjectSet::Matching {
                    objects: Box::new(self.still_exiled(self.exile_cards(cards)?)),
                    filter: self.exile_filter(filter)?,
                })),
                comparison: Comparison::Greater,
                right: Expr::Constant(0),
            },
            S::ACardWasPutIntoHandThisWay(cards) => {
                let results = self
                    .inspection_hand
                    .as_ref()
                    .ok_or_else(|| unsupported(value))?;
                let objects = ObjectSet::Union(
                    results
                        .iter()
                        .map(|result| ObjectSet::Transition {
                            result: result.clone(),
                            phase: ObjectPhase::Before,
                        })
                        .collect(),
                );
                Condition::Compare {
                    left: Expr::CountObjects(Box::new(ObjectSet::Matching {
                        objects: Box::new(objects),
                        filter: self.card_filter(cards)?,
                    })),
                    comparison: Comparison::Greater,
                    right: Expr::Constant(0),
                }
            }
            S::ACardWasExiledThisWay(cards) => {
                let result = self.exiled.clone().ok_or_else(|| unsupported(value))?;
                Condition::Compare {
                    left: Expr::CountObjects(Box::new(ObjectSet::Matching {
                        objects: Box::new(ObjectSet::Transition {
                            result,
                            phase: ObjectPhase::Before,
                        }),
                        filter: self.card_filter(cards)?,
                    })),
                    comparison: Comparison::Greater,
                    right: Expr::Constant(0),
                }
            }
            _ => return Err(unsupported(value)),
        })
    }

    fn duration(&self, value: &source::Expiration) -> Result<Duration> {
        Ok(match value {
            source::Expiration::UntilEndOfTurn => Duration::EndOfTurn,
            source::Expiration::UntilEndOfCombat => Duration::EndOfCombat,
            source::Expiration::UntilEndOfGame => Duration::Indefinite,
            source::Expiration::UntilPlayersNextTurn(p) => Duration::UntilNextTurn(self.player(p)?),
            _ => return Err(unsupported(value)),
        })
    }

    fn modifier(&self, value: &source::ModX, x: &Expr) -> Expr {
        match value {
            source::ModX::Integer(n) => Expr::Constant((*n).into()),
            source::ModX::PlusX => x.clone(),
            source::ModX::MinusX => Expr::multiply(Expr::Constant(-1), x.clone()),
            source::ModX::Zero => Expr::Constant(0),
        }
    }

    fn changes(&self, values: &[source::LayerEffect]) -> Result<Vec<Change>> {
        let mut context = self.clone();
        context.affected = true;
        context.layer_changes(values)
    }

    fn layer_changes(&self, values: &[source::LayerEffect]) -> Result<Vec<Change>> {
        use source::LayerEffect as S;
        let mut result = Vec::new();
        for (index, value) in values.iter().enumerate() {
            let changes: Result<Vec<Change>> = (|| {
                Ok(match value {
                    S::AdjustPT(power, toughness) => vec![Change::Pump {
                        power: Expr::Constant((*power).into()),
                        toughness: Expr::Constant((*toughness).into()),
                    }],
                    S::AdjustPTX(power, toughness, x) => {
                        let x = self.number(x)?;
                        vec![Change::Pump {
                            power: self.modifier(power, &x),
                            toughness: self.modifier(toughness, &x),
                        }]
                    }
                    S::AdjustPTForEach(power, toughness, n) => {
                        let n = self.number(n)?;
                        vec![Change::Pump {
                            power: Expr::multiply(Expr::Constant((*power).into()), n.clone()),
                            toughness: Expr::multiply(Expr::Constant((*toughness).into()), n),
                        }]
                    }
                    S::SetPT(source::PT::PT(p, t)) => vec![Change::SetBasePowerToughness {
                        power: Expr::Constant((*p).into()),
                        toughness: Expr::Constant((*t).into()),
                    }],
                    S::SetPT(source::PT::PTX(power, toughness, x)) => {
                        let x = self.number(x)?;
                        let component = |v: &source::PTXValue| match v {
                            source::PTXValue::Integer(n) => Expr::Constant((*n).into()),
                            source::PTXValue::X => x.clone(),
                        };
                        vec![Change::SetBasePowerToughness {
                            power: component(power),
                            toughness: component(toughness),
                        }]
                    }
                    S::SetPT(source::PT::ManualPT(p, t)) => vec![Change::SetBasePowerToughness {
                        power: self.number(p)?,
                        toughness: self.number(t)?,
                    }],
                    S::SetPowerAndToughnessBoth(n) => {
                        let n = self.number(n)?;
                        vec![Change::SetBasePowerToughness {
                            power: n.clone(),
                            toughness: n,
                        }]
                    }
                    S::SetPower(n) => vec![Change::SetPower(self.number(n)?)],
                    S::SetToughness(n) => vec![Change::SetToughness(self.number(n)?)],
                    S::SetController(p) => vec![Change::SetController(self.player(p)?)],
                    S::SetCreatureType(ty) => vec![Change::CreatureTypes { mode:ChangeMode::Set,types:CreatureTypeSet::Listed(vec![ty.clone()]) }],
                    S::SetCreatureTypes(types) => vec![Change::CreatureTypes { mode:ChangeMode::Set,types:CreatureTypeSet::Listed(types.clone()) }],
                    S::AddCreatureType(ty) => vec![Change::CreatureTypes { mode:ChangeMode::Add,types:CreatureTypeSet::Listed(vec![ty.clone()]) }],
                    S::AddCardtype(ty) => vec![Change::AddCardType(ty.clone())],
                    S::AddAbility(abilities) => abilities
                        .iter()
                        .map(|r| {
                            if let Ok(k) = self.keyword(r) {
                                Ok(Change::GrantKeyword(k))
                            } else {
                                if matches!(r, source::Rule::CDA_Color(_) | source::Rule::CDA_ColorButNotColorIdentity(_) | source::Rule::CDA_Power(_) | source::Rule::CDA_Toughness(_) | source::Rule::CDA_PowerToughness(..) | source::Rule::CDA_Types(_)) {
                                    return Err(invalid("granted characteristic text needs ordinary static-ability lowering"));
                                }
                                self.fresh()
                                    .ability(r)
                                    .map(|a| Change::GrantAbility(Box::new(a)))
                            }
                        })
                        .collect::<Result<_>>()?,
                    S::SetColor(c) | S::AddColor(c) => vec![Change::Colors { mode:if matches!(value,S::SetColor(_)) {ChangeMode::Set} else {ChangeMode::Add},colors:self.fixed_colors(c)? }],
                    S::HasAllCreatureTypes => vec![Change::CreatureTypes {mode:ChangeMode::Set,types:CreatureTypeSet::All}],
                    S::RemoveAllCreatureTypes => vec![Change::CreatureTypes {mode:ChangeMode::Set,types:CreatureTypeSet::Listed(vec![])}],
                    S::LosesAllAbilities => vec![Change::RemoveAllAbilities],
                    _ => return Err(unsupported(value)),
                })
            })();
            result.extend(changes.map_err(|e| e.at(format!("changes[{index}]")))?);
        }
        Ok(result)
    }

    fn static_changes(&self, values: &[source::StaticLayerEffect]) -> Result<Vec<Change>> {
        // A typed bridge shares the semantic conversion for live definitions
        // and resolving effects. No untyped or legacy value reaches the IR.
        use source::{LayerEffect as T, StaticLayerEffect as S};
        let mut changes = Vec::new();
        for (i, value) in values.iter().enumerate() {
            let change = match value {
                S::AdjustPT(p, t) => T::AdjustPT(*p, *t),
                S::AdjustPTX(p, t, x) => T::AdjustPTX(p.clone(), t.clone(), x.clone()),
                S::AdjustPTForEach(p, t, n) => T::AdjustPTForEach(*p, *t, n.clone()),
                S::SetPT(pt) => T::SetPT(pt.clone()),
                S::SetPowerAndToughnessBoth(n) => T::SetPowerAndToughnessBoth(n.clone()),
                S::SetPower(n) => T::SetPower(n.clone()),
                S::SetToughness(n) => T::SetToughness(n.clone()),
                S::SetController(p) => T::SetController(p.clone()),
                S::AddCreatureType(t) => T::AddCreatureType(t.clone()),
                S::SetCreatureType(t) => T::SetCreatureType(t.clone()),
                S::SetCreatureTypes(t) => T::SetCreatureTypes(t.clone()),
                S::AddCardtype(t) => T::AddCardtype(t.clone()),
                S::AddAbility(a) => T::AddAbility(a.clone()),
                S::SetColor(c) => T::SetColor(c.clone()),
                S::AddColor(c) => T::AddColor(c.clone()),
                S::HasAllCreatureTypes => T::HasAllCreatureTypes,
                S::RemoveAllCreatureTypes => T::RemoveAllCreatureTypes,
                S::LosesAllAbilities => T::LosesAllAbilities,
                _ => return Err(unsupported(value).at(format!("changes[{i}]"))),
            };
            changes.extend(
                self.changes(&[change])
                    .map_err(|e| e.at(format!("static[{i}]")))?,
            );
        }
        Ok(changes)
    }

    fn counter_placement(
        &self,
        value: &source::PutCountersAction,
        actor: &PlayerSet,
    ) -> Result<CounterPlacement> {
        use source::PutCountersAction as S;
        let (recipients, counter, amount, choose) = match value {
            S::ACounterOfTypeOnPermanent(kind, object) => (
                Recipients::Objects(ObjectSet::One(self.object(object)?)),
                kind,
                Expr::Constant(1),
                None,
            ),
            S::ACounterOfTypeOnAPermanent(kind, objects) => (
                Recipients::Objects(self.objects(objects)?),
                kind,
                Expr::Constant(1),
                Some(Count::AsManyAsCan(Expr::Constant(1))),
            ),
            S::ACounterOfTypeOnEachPermanent(kind, objects) => (
                Recipients::Objects(self.objects(objects)?),
                kind,
                Expr::Constant(1),
                None,
            ),
            S::ACounterOfTypeOnPlayer(kind, player) => (
                Recipients::Players(PlayerSet::One(self.player(player)?)),
                kind,
                Expr::Constant(1),
                None,
            ),
            S::ACounterOfTypeOnEachPlayer(kind, players) => (
                Recipients::Players(self.players(players)?),
                kind,
                Expr::Constant(1),
                None,
            ),
            S::NumberCountersOfTypeOnPermanent(n, kind, object) => (
                Recipients::Objects(ObjectSet::One(self.object(object)?)),
                kind,
                self.number(n)?,
                None,
            ),
            S::NumberCountersOfTypeOnAPermanent(n, kind, objects) => (
                Recipients::Objects(self.objects(objects)?),
                kind,
                self.number(n)?,
                Some(Count::AsManyAsCan(Expr::Constant(1))),
            ),
            S::NumberCountersOfTypeOnEachPermanent(n, kind, objects) => (
                Recipients::Objects(self.objects(objects)?),
                kind,
                self.number(n)?,
                None,
            ),
            S::NumberCountersOfTypeOnPlayer(n, kind, player) => (
                Recipients::Players(PlayerSet::One(self.player(player)?)),
                kind,
                self.number(n)?,
                None,
            ),
            S::NumberCountersOfTypeOnEachPlayer(n, kind, players) => (
                Recipients::Players(self.players(players)?),
                kind,
                self.number(n)?,
                None,
            ),
            S::ACounterOfTypeOnUptoNumberPermanents(kind, n, objects) => (
                Recipients::Objects(self.objects(objects)?),
                kind,
                Expr::Constant(1),
                Some(Count::UpTo(self.number(n)?)),
            ),
            _ => return Err(unsupported(value)),
        };
        let selection = choose
            .map(|count| match actor {
                PlayerSet::One(player) => Ok(SelectionChoice {
                    player: player.clone(),
                    count,
                }),
                _ => Err(invalid("counter selection requires one explicit decider")),
            })
            .transpose()?;
        Ok(CounterPlacement {
            recipients,
            kind: crate::counter::lower(counter),
            amount,
            selection,
        })
    }

    fn recipients(&self, value: &source::DamageRecipient) -> Result<Recipients> {
        use source::DamageRecipient as S;
        Ok(match value {
            S::Permanent(p) => Recipients::Objects(ObjectSet::One(self.object(p)?)),
            S::Player(p) => Recipients::Players(PlayerSet::One(self.player(p)?)),
            S::EachPermanent(p) => Recipients::Objects(self.objects(p)?),
            S::EachPlayer(p) => Recipients::Players(self.players(p)?),
            S::Ref_AnyTarget
            | S::Ref_AnyTarget1
            | S::Ref_AnyTargets
            | S::Ref_TargetPlayerOrPermanent => {
                Recipients::Targets(self.target(TargetKind::DamageRecipient, 0, 0)?)
            }
            S::MultipleRecipients(items) => Recipients::Union(
                items
                    .iter()
                    .map(|r| self.recipients(r))
                    .collect::<Result<_>>()?,
            ),
            _ => return Err(unsupported(value)),
        })
    }

    fn action(&mut self, value: &source::Action, actor: &PlayerSet) -> Result<Instruction> {
        if let Some(recipe) =
            self.prepared_program(crate::pipeline::programs::ProgramRef::Action(value))
        {
            return self.program_instruction(&recipe, actor);
        }
        if let Some(surface) = crate::pipeline::rewrite(value) {
            let expanded = if self.prepared.actions.is_empty() {
                surface.expand()
            } else {
                self.prepared
                    .actions
                    .get(&crate::pipeline::surface_key(&surface))
                    .cloned()
                    .unwrap_or_else(|| surface.expand())
            };
            return self.bind_pipeline_recipe(expanded, actor);
        }
        use source::Action as S;
        let operation = match value {
            S::CreateFutureTrigger(event, actions) | S::CreateFutureTriggerI(event, _, actions) => {
                let (event, start, expires) = self.future_turn(event)?;
                let mut child = self.clone();
                // Deferred work has its own search episode and result scope.
                // Existing reference bindings remain captured and read-only.
                child.open_searches.clear();
                let source::Actions::ActionList(actions) = actions.as_ref() else {
                    return Err(invalid(
                        "delayed announcements need their own target/mode scope",
                    ));
                };
                let intervening_if = if let S::CreateFutureTriggerI(_, condition, _) = value {
                    Some(child.condition(condition)?)
                } else {
                    None
                };
                let body = child.action_list(actions, actor)?;
                Operation::CreateDelayedTrigger {
                    event,
                    start,
                    expires,
                    intervening_if,
                    body,
                }
            }
            S::LookAtPlayersHand(p) | S::LookAtCardsOfTypeInPlayersHand(_, p) => {
                let PlayerSet::One(viewer) = actor else {
                    return Err(invalid("inspection requires one viewer"));
                };
                Operation::LookAt {
                    player: viewer.clone(),
                    objects: self.private_cards(
                        EventZone::Hand,
                        PlayerSet::One(self.player(p)?),
                        if let S::LookAtCardsOfTypeInPlayersHand(f, _) = value {
                            self.card_filter(f)?
                        } else {
                            ObjectFilter::Any
                        },
                    ),
                }
            }
            S::LookAtFaceDownPermanent(p) => {
                let PlayerSet::One(viewer) = actor else {
                    return Err(invalid("inspection requires one viewer"));
                };
                Operation::LookAt {
                    player: viewer.clone(),
                    objects: ObjectSet::One(self.object(p)?),
                }
            }
            S::LookAtFaceDownExiledCards(cards) => {
                let PlayerSet::One(viewer) = actor else {
                    return Err(invalid("inspection requires one viewer"));
                };
                Operation::LookAt {
                    player: viewer.clone(),
                    objects: self.exile_cards(cards)?,
                }
            }
            S::LookAtTopOfLibrary => {
                let PlayerSet::One(viewer) = actor else {
                    return Err(invalid("inspection requires one viewer"));
                };
                Operation::LookAt {
                    player: viewer.clone(),
                    objects: self.library_slice(
                        viewer.clone(),
                        Expr::Constant(1),
                        SequenceEnd::Top,
                    ),
                }
            }
            S::LookAtTheTopNumberCardsOfLibrary(n, actions)
            | S::LookAtTheTopNumberCardsOfPlayersLibrary(_, n, actions) => {
                let PlayerSet::One(player) = actor else {
                    return Err(invalid("inspection requires one viewer"));
                };
                let library = if let S::LookAtTheTopNumberCardsOfPlayersLibrary(p, ..) = value {
                    self.player(p)?
                } else {
                    player.clone()
                };
                return self.inspect_library(player.clone(), library, self.number(n)?, actions);
            }
            S::LookAtTheTopNumberCardsOfLibraryNEW(_) | S::LookAtTheTopCardOfPlayersLibrary(_) => {
                let PlayerSet::One(player) = actor else {
                    return Err(invalid("inspection requires one viewer"));
                };
                let (library, amount) = match value {
                    S::LookAtTheTopCardOfPlayersLibrary(p) => (self.player(p)?, Expr::Constant(1)),
                    S::LookAtTheTopNumberCardsOfLibraryNEW(n) => (player.clone(), self.number(n)?),
                    _ => unreachable!(),
                };
                Operation::LookAt {
                    player: player.clone(),
                    objects: self.library_slice(library, amount, SequenceEnd::Top),
                }
            }
            S::SearchLibrary(actions) => {
                let PlayerSet::One(player) = actor else {
                    return Err(invalid("search requires one searcher"));
                };
                return self.search_library(player.clone(), player.clone(), None, actions);
            }
            S::SearchPlayersLibrary(library, actions) => {
                let PlayerSet::One(player) = actor else {
                    return Err(invalid("search requires one searcher"));
                };
                return self.search_library(player.clone(), self.player(library)?, None, actions);
            }
            S::SearchTheTopNumberCardsOfLibrary(n, actions) => {
                let PlayerSet::One(player) = actor else {
                    return Err(invalid("search requires one searcher"));
                };
                return self.search_library(
                    player.clone(),
                    player.clone(),
                    Some(self.number(n)?),
                    actions,
                );
            }
            S::Shuffle => Operation::Shuffle {
                players: actor.clone(),
                excluding: None,
            },
            S::Exile(items, flags) => return self.exile(items, flags, actor),
            S::CreateGraveyardCardEffectUntil(card, effects, duration) => {
                Operation::CreateContinuousEffect {
                    source: ObjectRef::Source,
                    affected: EffectSubjects::Objects(ObjectSet::InZone {
                        objects: Box::new(ObjectSet::One(self.grave_card(card)?)),
                        zone: EventZone::Graveyard,
                    }),
                    changes: self.graveyard_changes(effects)?,
                    duration: self.duration(duration)?,
                    evaluation: EffectEvaluation::Snapshot,
                }
            }
            S::CreateEachCardInPlayersGraveyardEffectUntil(cards, player, effects, duration) => {
                Operation::CreateContinuousEffect {
                    source: ObjectRef::Source,
                    affected: EffectSubjects::Objects(self.private_cards(
                        EventZone::Graveyard,
                        PlayerSet::One(self.player(player)?),
                        self.card_filter(cards)?,
                    )),
                    changes: self.graveyard_changes(effects)?,
                    duration: self.duration(duration)?,
                    evaluation: EffectEvaluation::Snapshot,
                }
            }
            S::CounterSpell(s)
            | S::CounterSpellIntoExile(s)
            | S::CounterSpellIntoHand(s)
            | S::CounterSpellIntoTopOfLibrary(s)
            | S::CounterSpellIntoBottomOfLibrary(s) => Operation::Counter {
                objects: ObjectSet::One(self.spell(s)?),
                destination: match value {
                    S::CounterSpellIntoExile(_) => Destination::Exile,
                    S::CounterSpellIntoHand(_) => Destination::OwnersHand,
                    S::CounterSpellIntoTopOfLibrary(_) => Destination::OwnersLibraryTop,
                    S::CounterSpellIntoBottomOfLibrary(_) => Destination::OwnersLibraryBottom,
                    _ => Destination::OwnersGraveyard,
                },
            },
            S::RemoveCounters(removals) => Operation::RemoveCounters {
                removals: removals
                    .iter()
                    .enumerate()
                    .map(|(i, r)| {
                        self.remove_counter_action(r, actor)
                            .map_err(|e| e.at(format!("removals[{i}]")))
                    })
                    .collect::<Result<_>>()?,
            },
            S::AttachPermanentToPermanent(attachment, host) => Operation::Attach {
                attachments: ObjectSet::One(self.object(attachment)?),
                host: self.object(host)?,
            },
            S::CreateTokens(tokens, flags) => Operation::CreateTokens {
                players: actor.clone(),
                tokens: tokens
                    .iter()
                    .enumerate()
                    .map(|(i, token)| {
                        self.token(token, 0)
                            .map_err(|e| e.at(format!("tokens[{i}]")))
                    })
                    .collect::<Result<_>>()?,
                entry: self.token_entry(flags)?,
            },
            S::PutCounters(placements) => Operation::PutCounters {
                placements: placements
                    .iter()
                    .enumerate()
                    .map(|(i, p)| {
                        self.counter_placement(p, actor)
                            .map_err(|e| e.at(format!("placements[{i}]")))
                    })
                    .collect::<Result<_>>()?,
            },
            S::AddMana(mana) | S::AddManaWithModifiers(mana, _) => Operation::AddMana {
                players: actor.clone(),
                mana: self.mana_production(mana)?,
                restriction: match value {
                    S::AddManaWithModifiers(_, modifier) => Some(self.mana_use(modifier)?),
                    _ => None,
                },
            },
            S::AddManaRepeated(mana, n)
            | S::AddManaRepeatedWithModifiers(mana, n, _)
            | S::AddCombinationMana(mana, n)
            | S::AddCombinationManaWithModifers(mana, n, _) => Operation::AddMana {
                players: actor.clone(),
                mana: ManaProduction::Repeat {
                    amount: self.number(n)?,
                    production: Box::new(self.mana_production(mana)?),
                    choice: if matches!(
                        value,
                        S::AddCombinationMana(..) | S::AddCombinationManaWithModifers(..)
                    ) {
                        ManaChoiceTiming::EachUnit
                    } else {
                        ManaChoiceTiming::Once
                    },
                },
                restriction: match value {
                    S::AddManaRepeatedWithModifiers(_, _, modifier)
                    | S::AddCombinationManaWithModifers(_, _, modifier) => {
                        Some(self.mana_use(modifier)?)
                    }
                    _ => None,
                },
            },
            S::AddManaRepeatedTwiceWithModifiers(a, n, b, m, modifier) => Operation::AddMana {
                players: actor.clone(),
                mana: ManaProduction::All(vec![
                    ManaProduction::Repeat {
                        amount: self.number(n)?,
                        production: Box::new(self.mana_production(a)?),
                        choice: ManaChoiceTiming::Once,
                    },
                    ManaProduction::Repeat {
                        amount: self.number(m)?,
                        production: Box::new(self.mana_production(b)?),
                        choice: ManaChoiceTiming::Once,
                    },
                ]),
                restriction: Some(self.mana_use(modifier)?),
            },
            S::PermanentDealsDamage(p, n, r) => Operation::DealDamage {
                source: self.object(p)?,
                recipients: self.recipients(r)?,
                amount: self.number(n)?,
                preventable: true,
            },
            S::SpellDealsDamage(spell, n, r) | S::SpellDealsDamageCantBePrevented(spell, n, r) => {
                if !matches!(spell.as_ref(), source::Spell::ThisSpell) {
                    return Err(unsupported(spell));
                }
                Operation::DealDamage {
                    source: ObjectRef::Source,
                    recipients: self.recipients(r)?,
                    amount: self.number(n)?,
                    preventable: matches!(value, S::SpellDealsDamage(_, _, _)),
                }
            }
            S::DiscardedCardDealsDamage(c, n, r) | S::HaveDiscardedCardDealDamage(c, n, r) => {
                Operation::DealDamage {
                    source: self.hand_card(c)?,
                    recipients: self.recipients(r)?,
                    amount: self.number(n)?,
                    preventable: true,
                }
            }
            S::DealDamage(dealer, n, r) => {
                let source = match dealer.as_ref() {
                    source::DamageDealer::Permanent(p) => self.object(p)?,
                    source::DamageDealer::Spell(source::Spell::ThisSpell) => ObjectRef::Source,
                    _ => return Err(unsupported(dealer)),
                };
                Operation::DealDamage {
                    source,
                    recipients: self.recipients(r)?,
                    amount: self.number(n)?,
                    preventable: true,
                }
            }
            S::CreatePlayerEffect(player, rules) | S::CreatePlayerEffectUntil(player, rules, _) => {
                Operation::CreateContinuousEffect {
                    source: ObjectRef::Source,
                    affected: EffectSubjects::Players(PlayerSet::One(self.player(player)?)),
                    changes: self.player_changes(rules)?,
                    duration: match value {
                        S::CreatePlayerEffectUntil(_, _, duration) => self.duration(duration)?,
                        _ => Duration::Indefinite,
                    },
                    evaluation: EffectEvaluation::LiveObjects,
                }
            }
            S::CreateEachPlayerEffectUntil(players, rules, duration) => {
                Operation::CreateContinuousEffect {
                    source: ObjectRef::Source,
                    affected: EffectSubjects::Players(self.players(players)?),
                    changes: self.player_changes(rules)?,
                    duration: self.duration(duration)?,
                    evaluation: EffectEvaluation::LiveObjects,
                }
            }
            _ => return Err(unsupported(value)),
        };
        Ok(self.effect(operation))
    }

    fn action_list(&mut self, values: &[source::Action], actor: &PlayerSet) -> Result<Program> {
        Ok(Program {
            instructions: values
                .iter()
                .enumerate()
                .map(|(i, a)| {
                    self.action(a, actor)
                        .map_err(|e| e.at(format!("actions[{i}]")))
                })
                .collect::<Result<_>>()?,
        })
    }

    fn targets(&mut self, values: &[source::Target]) -> Result<()> {
        let mut proposal = self.proposal();
        proposal.proposal_targets(values)?;
        self.targets = proposal.targets;
        Ok(())
    }
    fn proposal_targets(&mut self, values: &[source::Target]) -> Result<()> {
        use source::Target as S;
        for (i, value) in values.iter().enumerate() {
            let mut target = TargetSpec {
                slot: Slot(
                    self.targets
                        .len()
                        .try_into()
                        .map_err(|_| invalid("too many target slots"))?,
                ),
                kind: TargetKind::Object(ObjectDomain::permanent()),
                count: Count::Exactly(Expr::Constant(1)),
                filter: ObjectFilter::Any,
                players: None,
            };
            match value {
                S::TargetSpell(s)
                | S::UptoOneTargetSpell(s)
                | S::UptoNumberTargetSpells(_, s)
                | S::AnyNumberOfTargetSpells(s) => {
                    target.kind = TargetKind::Object(ObjectDomain::spell());
                    target.filter = self.spell_filter(s)?;
                    target.count = match value {
                        S::UptoOneTargetSpell(_) => Count::UpTo(Expr::Constant(1)),
                        S::UptoNumberTargetSpells(n, _) => Count::UpTo(self.number(n)?),
                        S::AnyNumberOfTargetSpells(_) => Count::AnyNumber,
                        _ => Count::Exactly(Expr::Constant(1)),
                    };
                }
                S::TargetGraveyardCard(c)
                | S::UptoOneTargetGraveyardCard(c)
                | S::NumberTargetGraveyardCards(_, c)
                | S::UptoNumberTargetGraveyardCards(_, c)
                | S::AnyNumberOfTargetGraveyardCards(c) => {
                    target.kind = TargetKind::Object(ObjectDomain::card(EventZone::Graveyard));
                    target.filter = self.grave_filter(c)?;
                    target.count = match value {
                        S::UptoOneTargetGraveyardCard(_) => Count::UpTo(Expr::Constant(1)),
                        S::NumberTargetGraveyardCards(n, _) => Count::Exactly(self.number(n)?),
                        S::UptoNumberTargetGraveyardCards(n, _) => Count::UpTo(self.number(n)?),
                        S::AnyNumberOfTargetGraveyardCards(_) => Count::AnyNumber,
                        _ => Count::Exactly(Expr::Constant(1)),
                    };
                }
                S::TargetExiledCard(c) | S::UptoOneTargetExiledCard(c) => {
                    target.kind = TargetKind::Object(ObjectDomain::card(EventZone::Exile));
                    target.filter = self.exile_filter(c)?;
                    if matches!(value, S::UptoOneTargetExiledCard(_)) {
                        target.count = Count::UpTo(Expr::Constant(1));
                    }
                }
                S::TargetPermanent(p) => target.filter = self.filter(p)?,
                S::UptoOneTargetPermanent(p) => {
                    target.filter = self.filter(p)?;
                    target.count = Count::UpTo(Expr::Constant(1));
                }
                S::NumberTargetPermanents(n, p) => {
                    target.filter = self.filter(p)?;
                    target.count = Count::Exactly(self.number(n)?);
                }
                S::UptoNumberTargetPermanents(n, p) => {
                    target.filter = self.filter(p)?;
                    target.count = Count::UpTo(self.number(n)?);
                }
                S::AnyNumberOfTargetPermanents(p) => {
                    target.filter = self.filter(p)?;
                    target.count = Count::AnyNumber;
                }
                S::TargetPlayer(p) | S::UptoOneTargetPlayer(p) => {
                    target.kind = TargetKind::Player;
                    target.players = Some(self.players(p)?);
                    if matches!(value, S::UptoOneTargetPlayer(_)) {
                        target.count = Count::UpTo(Expr::Constant(1));
                    }
                }
                S::AnyTarget => target.kind = TargetKind::DamageRecipient,
                S::TargetPlayerOrPermanent(players, objects) => {
                    target.kind = TargetKind::DamageRecipient;
                    target.filter = self.filter(objects)?;
                    target.players = Some(self.players(players)?);
                }
                S::AnyNumberOfAnyTargets => {
                    target.kind = TargetKind::DamageRecipient;
                    target.count = Count::AnyNumber;
                }
                _ => return Err(unsupported(value).at(format!("targets[{i}]"))),
            }
            self.targets.push(target);
        }
        Ok(())
    }

    fn actions(&mut self, value: &source::Actions) -> Result<Program> {
        if let Some(recipe) =
            self.prepared_program(crate::pipeline::programs::ProgramRef::Actions(value))
        {
            return self
                .bind_program_recipe(&recipe, &PlayerSet::One(PlayerRef::AbilityController));
        }
        Err(unsupported(value))
    }

    fn cost_for(&self, value: &source::Cost, player: &PlayerRef) -> Result<Cost> {
        let mut context = self.clone();
        context.payer = Some(player.clone());
        context.cost(value)
    }

    fn cost(&self, value: &source::Cost) -> Result<Cost> {
        if let Some(crate::pipeline::operands::OperandRecipe::Cost(recipe)) =
            self.operand_recipe(crate::pipeline::operands::OperandRef::Cost(value))
        {
            return self.bind_cost_recipe(&recipe);
        }
        Err(unsupported(value))
    }

    fn limits(
        &self,
        value: &source::ActivateModifier,
        limits: &mut ActivationLimits,
    ) -> Result<()> {
        match value {
            source::ActivateModifier::And(items) => {
                for item in items {
                    self.limits(item, limits)?;
                }
            }
            source::ActivateModifier::ActivateOnlyOnce => limits.once = true,
            source::ActivateModifier::ActivateOnlyOnceEachTurn => limits.once_each_turn = true,
            source::ActivateModifier::ActivateOnlyAsASorcery => limits.sorcery = true,
            source::ActivateModifier::ActivateOnlyIf(condition)
            | source::ActivateModifier::CantActivateIf(condition) => {
                let mut condition = self.condition(condition)?;
                if matches!(value, source::ActivateModifier::CantActivateIf(_)) {
                    condition = Condition::Not(Box::new(condition));
                }
                limits.condition = Some(match limits.condition.take() {
                    Some(previous) => Condition::All(vec![previous, condition]),
                    None => condition,
                });
            }
            source::ActivateModifier::ActivateOnlyDuringTheirTurn => {
                let condition = Condition::PlayerIsActive(self.player(&source::Player::You)?);
                limits.condition = Some(match limits.condition.take() {
                    Some(previous) => Condition::All(vec![previous, condition]),
                    None => condition,
                });
            }
            _ => return Err(unsupported(value)),
        }
        Ok(())
    }

    fn ability(&mut self, value: &source::Rule) -> Result<Ability> {
        if !self.prepared.mechanics.is_empty() {
            if let Some(recipe) = self
                .prepared
                .mechanics
                .get(&crate::pipeline::mechanics::source_key(value))
            {
                return self.bind_mechanic(recipe);
            }
        }
        if let Some(recipe) = crate::pipeline::mechanics::rewrite(value) {
            return self.bind_mechanic(&recipe);
        }
        use source::Rule as S;
        match value {
            S::Flashback(cost) => Ok(self.flashback(self.cost(cost)?)),
            S::FlashbackX(mana) => Ok(self.flashback(Cost {
                parts: vec![CostPart::Mana(
                    mana.iter().map(mana_symbol_x).collect::<Result<_>>()?,
                )],
            })),
            S::CDA_Color(_)
            | S::CDA_ColorButNotColorIdentity(_)
            | S::CDA_Power(_)
            | S::CDA_Toughness(_)
            | S::CDA_PowerToughness(..)
            | S::CDA_Types(_) => self.characteristic_definition(value),
            S::FromHand(_)
            | S::FromGraveyard(_)
            | S::FromGraveyardIf(..)
            | S::FromGraveyardOrBattlefield(_)
            | S::FromStack(_)
            | S::FromStackIf(..)
            | S::FromExile(_)
            | S::FromExileIf(..)
            | S::FromExileOrBattlefield(_)
            | S::FromAnyZone(_)
            | S::FromCommandZone(_)
            | S::FromCommandZoneOrBattlefield(_) => self.scoped_ability(value),
            S::Cycling(cost) => self.cycling(self.cost(cost)?),
            S::CyclingX(mana) => self.cycling(Cost {
                parts: vec![CostPart::Mana(
                    mana.iter().map(mana_symbol_x).collect::<Result<_>>()?,
                )],
            }),
            S::TypeCycling(filter, cost) => {
                self.typecycling(self.card_filter(filter)?, self.cost(cost)?)
            }
            S::ThisSpellEffect(effects) => with_source_scope(
                vec![EventZone::Stack],
                None,
                self.stack_effect(ObjectFilter::SameObject(ObjectRef::Source), effects)?,
            ),
            S::StackSpellsEffect(spells, effects) => {
                self.stack_effect(self.spell_filter(spells)?, effects)
            }
            S::CastEffect(rule) => Ok(Ability::Casting(self.casting_rule(rule)?)),
            S::NoMoreThanNumberCreaturesCanAttack(n) | S::NoMoreThanNumberCreaturesCanBlock(n) => {
                Ok(Ability::Static {
                    kind: StaticKind::Ordinary,
                    affected: EffectSubjects::Objects(ObjectSet::Battlefield(
                        ObjectFilter::CardType(source::CardType::Creature),
                    )),
                    changes: vec![Change::Rule(RuleChange::CombatGroup {
                        role: if matches!(value, S::NoMoreThanNumberCreaturesCanAttack(_)) {
                            CombatRole::Attacker
                        } else {
                            CombatRole::Blocker
                        },
                        participants: ObjectFilter::Any,
                        others_only: false,
                        comparison: Comparison::LessOrEqual,
                        amount: self.number(n)?,
                    })],
                })
            }
            S::ActivatedAbilityEffect(filter, source::ActivatedAbilityEffect::CantBeActivated) => {
                let (sources, filter) = self.permanent_activations(filter)?;
                Ok(Ability::Static {
                    kind: StaticKind::Ordinary,
                    affected: EffectSubjects::Objects(ObjectSet::Battlefield(sources.ok_or_else(
                        || {
                            invalid(
                                "activation restriction includes sources outside the battlefield",
                            )
                        },
                    )?)),
                    changes: vec![Change::Rule(RuleChange::Prohibit(ObjectAction::Activate(
                        filter,
                    )))],
                })
            }
            S::AsPermanentEnters(p, actions)
                if matches!(p.as_ref(), source::Permanent::ThisPermanent) =>
            {
                self.entry_replacement(
                    ReplacementActive::SourceEntering,
                    ObjectFilter::SameObject(ObjectRef::Source),
                    actions,
                )
            }
            S::ReplaceWouldEnter(event, actions) => {
                let objects = match event {
                    source::ReplacableEventWouldEnter::APermanentWouldEnterTheBattlefield(p) => self.filter(p)?,
                    source::ReplacableEventWouldEnter::PermanentWouldEnterTheBattlefield(p) => ObjectFilter::SameObject(self.object(p)?),
                    source::ReplacableEventWouldEnter::APermanentWouldEnterTheBattlefieldUnderAPlayersControl(p,players) => ObjectFilter::All(vec![self.filter(p)?,ObjectFilter::ControlledBy(self.players(players)?)]),
                    _=>return Err(unsupported(event)),
                };
                let active = if matches!(objects, ObjectFilter::SameObject(ObjectRef::Source)) {
                    ReplacementActive::SourceEntering
                } else {
                    ReplacementActive::Battlefield
                };
                self.entry_replacement(active, objects, actions)
            }
            S::EnchantPermanent(hosts) => Ok(Ability::Enchant {
                hosts: EnchantHosts::Permanents(self.filter(hosts)?),
            }),
            S::EnchantPlayer(players) => Ok(Ability::Enchant {
                hosts: EnchantHosts::Players(self.players(players)?),
            }),
            S::Equip(quality, cost) => {
                let filter = self.filter(quality)?;
                fn planeswalker(f: &ObjectFilter) -> bool {
                    match f {
                        ObjectFilter::CardType(source::CardType::Planeswalker) => true,
                        ObjectFilter::All(v) | ObjectFilter::Either(v) => {
                            v.iter().any(planeswalker)
                        }
                        ObjectFilter::Not(f) => planeswalker(f),
                        _ => false,
                    }
                }
                if planeswalker(&filter) {
                    return Err(invalid(
                        "equip planeswalker requires its separate as-though-creature semantics",
                    ));
                }
                let slot = Slot(0);
                Ok(Ability::Activated {
                    origin: ActivationOrigin::Equip,
                    cost: self.cost(cost)?,
                    limits: ActivationLimits {
                        sorcery: true,
                        ..ActivationLimits::unrestricted()
                    },
                    announcement: Announcement {
                        targets: vec![TargetSpec {
                            slot: slot.clone(),
                            kind: TargetKind::Object(ObjectDomain::permanent()),
                            count: Count::Exactly(Expr::Constant(1)),
                            filter: ObjectFilter::All(vec![
                                filter,
                                ObjectFilter::ControlledBy(PlayerSet::One(
                                    PlayerRef::AbilityController,
                                )),
                            ]),
                            players: None,
                        }],
                        modes: None,
                    },
                    body: Program {
                        instructions: vec![self.effect(Operation::Attach {
                            attachments: ObjectSet::One(ObjectRef::Source),
                            host: ObjectRef::Target { slot, index: 0 },
                        })],
                    },
                })
            }
            S::PlayerEffect(player, rules) => Ok(Ability::Static {
                kind: StaticKind::Ordinary,
                affected: EffectSubjects::Players(PlayerSet::One(self.player(player)?)),
                changes: self.player_changes(rules)?,
            }),
            S::EachPlayerEffect(players, rules) => Ok(Ability::Static {
                kind: StaticKind::Ordinary,
                affected: EffectSubjects::Players(self.players(players)?),
                changes: self.player_changes(rules)?,
            }),
            S::PermanentRuleEffect(object, rules) => Ok(Ability::Static {
                kind: StaticKind::Ordinary,
                affected: EffectSubjects::Objects(ObjectSet::One(self.object(object)?)),
                changes: self.rule_changes(rules)?,
            }),
            S::EachPermanentRuleEffect(objects, rules) => Ok(Ability::Static {
                kind: StaticKind::Ordinary,
                affected: EffectSubjects::Objects(self.objects(objects)?),
                changes: self.rule_changes(rules)?,
            }),
            S::PermanentLayerEffect(object, changes) => Ok(Ability::Static {
                kind: StaticKind::Ordinary,
                affected: EffectSubjects::Objects(ObjectSet::One(self.object(object)?)),
                changes: self.static_changes(changes)?,
            }),
            S::EachPermanentLayerEffect(objects, changes) => Ok(Ability::Static {
                kind: StaticKind::Ordinary,
                affected: EffectSubjects::Objects(self.objects(objects)?),
                changes: self.static_changes(changes)?,
            }),
            S::If(condition, then) | S::Unless(condition, then) | S::IfElse(condition, then, _) => {
                let mut condition = self.condition(condition)?;
                if matches!(value, S::Unless(_, _)) {
                    condition = Condition::Not(Box::new(condition));
                }
                let abilities = |rules: &[source::Rule]| {
                    rules
                        .iter()
                        .enumerate()
                        .map(|(i, rule)| {
                            self.fresh()
                                .ability(rule)
                                .map_err(|e| e.at(format!("abilities[{i}]")))
                        })
                        .collect::<Result<Vec<_>>>()
                };
                Ok(Ability::Conditional {
                    condition,
                    then: abilities(then)?,
                    otherwise: match value {
                        S::IfElse(_, _, otherwise) => abilities(otherwise)?,
                        _ => vec![],
                    },
                })
            }
            S::SpellActions(actions) => {
                let body = self.actions(actions)?;
                Ok(Ability::Spell {
                    announcement: Announcement {
                        targets: self.targets.clone(),
                        modes: self.modes.clone(),
                    },
                    body,
                })
            }
            S::Activated(cost, actions) | S::ActivatedWithModifiers(cost, actions, _) => {
                let cost = self.cost(cost)?;
                self.install_paid_history(&cost, PaymentRef::Activation);
                let mut limits = ActivationLimits::unrestricted();
                if cost.parts.iter().any(|p| matches!(p, CostPart::Loyalty(_))) {
                    limits.sorcery = true;
                }
                if let S::ActivatedWithModifiers(_, _, modifier) = value {
                    self.proposal().limits(modifier, &mut limits)?;
                }
                let body = self.actions(actions)?;
                Ok(Ability::Activated {
                    origin: ActivationOrigin::Ordinary,
                    cost,
                    limits,
                    announcement: Announcement {
                        targets: self.targets.clone(),
                        modes: self.modes.clone(),
                    },
                    body,
                })
            }
            S::TriggerA(trigger, actions)
            | S::TriggerI(trigger, _, actions)
            | S::TriggerOnce(trigger, actions)
            | S::TriggerOnceEachTurn(trigger, actions)
            | S::TriggerTwiceEachTurn(trigger, actions)
            | S::TriggerOnceEachTurnI(trigger, _, actions)
            | S::TriggerIOnce(trigger, _, actions)
            | S::TriggerIOnceEachTurn(trigger, _, actions) => {
                let event = self.trigger(trigger)?;
                self.event = crate::semantics::event_bindings(&event);
                if matches!(trigger, source::Trigger::WhenAPermanentEntersTheBattlefield(p) | source::Trigger::WhenAPermanentEntersTheBattlefieldFromExile(p) if matches!(p.as_ref(), source::Permanents::SinglePermanent(p) if matches!(p.as_ref(), source::Permanent::ThisPermanent)))
                {
                    self.cast_x = Some(ObjectRef::EventSubject);
                }
                let recipient_alias = legacy_damage_recipient_alias(trigger, actions);
                self.ambiguous_damage_permanent =
                    legacy_damage_ambiguity(&event) && !recipient_alias;
                self.damage_permanent_recipient = recipient_alias
                    || legacy_damage_permanent_role(&event) == Some(LegacyPermanentRef::Recipient);
                self.state_subject = match trigger {
                    source::Trigger::WhenAPermanentHasNumberCountersOfType(objects, _, _) => {
                        match objects.as_ref() {
                            source::Permanents::SinglePermanent(object) => {
                                Some(self.object(object)?)
                            }
                            _ => None,
                        }
                    }
                    _ => None,
                };
                let intervening_if = match value {
                    S::TriggerI(_, condition, _)
                    | S::TriggerOnceEachTurnI(_, condition, _)
                    | S::TriggerIOnce(_, condition, _)
                    | S::TriggerIOnceEachTurn(_, condition, _) => Some(self.condition(condition)?),
                    _ => None,
                };
                let limits = TriggerLimits {
                    per_turn: match value {
                        S::TriggerOnceEachTurn(_, _)
                        | S::TriggerOnceEachTurnI(_, _, _)
                        | S::TriggerIOnceEachTurn(_, _, _) => Some(1),
                        S::TriggerTwiceEachTurn(_, _) => Some(2),
                        _ => None,
                    },
                    total: match value {
                        S::TriggerOnce(_, _) | S::TriggerIOnce(_, _, _) => Some(1),
                        _ => None,
                    },
                };
                let body = self.actions(actions)?;
                Ok(Ability::Triggered {
                    event,
                    intervening_if,
                    limits,
                    announcement: Announcement {
                        targets: self.targets.clone(),
                        modes: self.modes.clone(),
                    },
                    body,
                })
            }
            _ => self.keyword(value).map(Ability::Keyword),
        }
    }
}

fn keyword(value: &source::Rule) -> Result<Keyword> {
    use source::Rule as S;
    Ok(match value {
        S::Flying => Keyword::Flying,
        S::Trample => Keyword::Trample,
        S::Lifelink => Keyword::Lifelink,
        S::Deathtouch => Keyword::Deathtouch,
        S::Vigilance => Keyword::Vigilance,
        S::Haste => Keyword::Haste,
        S::Reach => Keyword::Reach,
        S::FirstStrike => Keyword::FirstStrike,
        S::DoubleStrike => Keyword::DoubleStrike,
        S::Indestructible => Keyword::Indestructible,
        S::Hexproof => Keyword::Hexproof,
        S::Shroud => Keyword::Shroud,
        S::Menace => Keyword::Menace,
        S::Defender => Keyword::Defender,
        S::Flash => Keyword::Flash,
        S::Fear => Keyword::Fear,
        S::Intimidate => Keyword::Intimidate,
        S::Infect => Keyword::Infect,
        S::Wither => Keyword::Wither,
        S::Prowess => Keyword::Prowess,
        S::Flanking => Keyword::Flanking,
        S::Horsemanship => Keyword::Horsemanship,
        S::Shadow => Keyword::Shadow,
        S::Skulk => Keyword::Skulk,
        _ => return Err(unsupported(value)),
    })
}

pub fn card(value: &source::OracleCard) -> std::result::Result<Card, Vec<Diagnostic>> {
    crate::pipeline::lower(value)
}
pub(crate) fn bind_card(
    value: &source::OracleCard,
    prepared: crate::pipeline::PreparedRecipes,
) -> std::result::Result<Card, Vec<Diagnostic>> {
    let source::OracleCard::Card {
        name,
        typeline,
        mana_cost,
        color_indicator,
        rules,
        card_pt,
        loyalty,
        defense,
    } = value
    else {
        return Err(vec![unsupported(value)]);
    };
    let metadata = (|| -> Result<Characteristics> {
        let color_indicator = color_indicator
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(|c| match c {
                source::ColorIndicatorColor::White => ManaColor::White,
                source::ColorIndicatorColor::Blue => ManaColor::Blue,
                source::ColorIndicatorColor::Black => ManaColor::Black,
                source::ColorIndicatorColor::Red => ManaColor::Red,
                source::ColorIndicatorColor::Green => ManaColor::Green,
            })
            .collect();
        Ok(Characteristics {
            supertypes: typeline.supertypes.clone(),
            card_types: typeline.cardtypes.clone(),
            subtypes: typeline.subtypes.clone(),
            mana_cost: mana_cost
                .as_ref()
                .map(|m| m.iter().map(mana_symbol_x).collect::<Result<_>>())
                .transpose()?,
            color_indicator,
            power_toughness: card_pt.as_ref().map(|p| (p.power, p.toughness)),
            loyalty: loyalty
                .as_ref()
                .map(|l| match l {
                    source::LoyaltyNumber::Integer(n) => Ok(Expr::Constant((*n).into())),
                    _ => Err(unsupported(l)),
                })
                .transpose()?,
            defense: *defense,
        })
    })();
    let mut errors = Vec::new();
    let mut abilities = Vec::new();
    let base = Context {
        prepared,
        ..Context::default()
    };
    let casting_histories: Vec<_> = rules
        .as_deref()
        .unwrap_or_default()
        .iter()
        .enumerate()
        .filter_map(|(i, rule)| {
            let source::Rule::CastEffect(effect) = rule else {
                return None;
            };
            let native = base.fresh().casting_rule(effect).ok()?;
            let ability = Ability::Casting(native);
            crate::cost_history::casting_cost(&ability)
                .cloned()
                .map(|cost| (i as u32, cost))
        })
        .collect();
    for (i, rule) in rules.as_deref().unwrap_or_default().iter().enumerate() {
        let mut context = base.fresh();
        if matches!(rule, source::Rule::SpellActions(_)) {
            // Multiple declarations of the same role are ambiguous, even when
            // they reside in different additional-cost abilities.
            let mut combined = std::collections::BTreeMap::new();
            for (ability, cost) in &casting_histories {
                context.install_paid_history(cost, PaymentRef::Casting { ability: *ability });
                for (action, _) in crate::cost_history::components(cost) {
                    if combined.insert(action, ()).is_some() {
                        context.paid.insert(action, None);
                    }
                }
            }
        }
        match context.ability(rule) {
            Ok(ability) => abilities.push(ability),
            Err(error) => errors.push(error.at(format!("Rules[{i}]"))),
        }
    }
    let characteristics = match metadata {
        Ok(m) => Some(m),
        Err(e) => {
            errors.push(e.at("characteristics"));
            None
        }
    };
    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(Card {
        schema_version: SCHEMA_VERSION,
        name: name.clone(),
        characteristics: characteristics.unwrap(),
        abilities,
    })
}

// Mana symbols are generated as exhaustive matches over the legacy vocabulary.
// This is inert printed data, shared by costs and characteristics.
include!("lower_mana.rs");
include!("lower_tokens.rs");
include!("lower_mana_production.rs");

include!("lower_events.rs");

include!("lower_queries.rs");

include!("lower_counters.rs");
include!("lower_replacement.rs");

include!("lower_rules.rs");

include!("lower_card_filters.rs");
include!("lower_zones.rs");
include!("lower_casting.rs");
include!("lower_cast_history.rs");
include!("lower_damage.rs");
include!("lower_scopes.rs");
include!("lower_library.rs");

include!("lower_players.rs");

include!("lower_keywords.rs");

include!("lower_inspection.rs");

include!("lower_characteristics.rs");
include!("lower_pipeline.rs");
include!("lower_groups.rs");
include!("lower_mechanics.rs");
include!("lower_operand_pipeline.rs");
include!("lower_program_pipeline.rs");
include!("lower_selector_pipeline.rs");
include!("lower_number_pipeline.rs");

include!("lower_paid.rs");
