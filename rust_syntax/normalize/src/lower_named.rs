// `Named` forms are built from the inspected source tree, so every enum variant
// has one with no per-variant code. Operands lower with an empty Context: a
// reference that needs the enclosing ability's bindings fails and stays a Form.
fn named_value(value: &impl Serialize) -> Named {
    let inspected = crate::inspect::value(value).expect("source AST serialization");
    named_node(&inspected)
}

fn named_rule(rule: &source::Rule) -> Named {
    named_value(rule)
}

/// The source rule for an ability index of a Card face.
pub(crate) fn rule_at(card: &source::OracleCard, index: usize) -> Named {
    let source::OracleCard::Card { rules, .. } = card else {
        unreachable!("faces are Card values")
    };
    named_rule(&rules.as_deref().unwrap_or_default()[index])
}

/// A card field with no reduced form.
fn named_field(name: &str, value: &impl Serialize) -> Named {
    let inspected = crate::inspect::value(value).expect("source AST serialization");
    Named {
        source_type: "Characteristics".into(),
        form: name.into(),
        operands: vec![operand(&inspected)],
    }
}

fn named_node(value: &serde_json::Value) -> Named {
    let args = &value["args"];
    Named {
        source_type: value["enum"].as_str().unwrap_or("value").into(),
        form: value["variant"].as_str().unwrap_or("value").into(),
        operands: match args {
            serde_json::Value::Null => vec![],
            serde_json::Value::Array(items) => items.iter().map(operand).collect(),
            other => vec![operand(other)],
        },
    }
}

include!(concat!(env!("OUT_DIR"), "/vocabularies.rs"));

pub use crate::complete::reads_scope;

/// The word in a "this way" row's name, and the words of the instruction it
/// reads. A row whose word no instruction here has reads a cost or an event.
const THIS_WAY: &[(&[&str], &[&str])] = &[
    (&["Energy"], &["Energy"]),
    (&["Evidence"], &["Evidence"]),
    // "The exiled pile chosen this way" reads the choice of a pile, not the
    // last move of exiled cards (Abstract Performance).
    (
        &["PileChosen", "PileNotChosen"],
        &["ChoosePile", "ChooseAnExiledPile", "ChooseAPermanentPile", "ChooseAGraveyardPile", "SeparatePiles", "IntoTwoPiles", "IntoNumberPiles"],
    ),
    (&["Exploited"], &["Exploit"]),
    (&["CountersOfType"], &["Counters"]),
    (&["Discard"], &["Discard"]),
    (&["Drawn"], &["Draw"]),
    (&["Exile"], &["Exile"]),
    (&["Milled"], &["Mill"]),
    (&["Surveil"], &["Surveil"]),
    (&["Died"], &["Destroy", "Sacrifice", "Damage"]),
    (&["Destroy"], &["Destroy"]),
    (&["Sacrifice"], &["Sacrifice"]),
    (&["Countered"], &["Counter"]),
    (&["Reveal"], &["Reveal", "LookAt"]),
    (&["Tapped"], &["Tap"]),
    (&["Token"], &["Token"]),
    (&["Damage"], &["Damage"]),
    (&["Paid"], &["Pay"]),
    (&["Chosen", "Chose"], &["Choose", "Choice", "StartingWith"]),
    (&["Cast"], &["Cast"]),
    (&["Copied"], &["Copy"]),
    (&["Unattached", "Unequipped"], &["Attach"]),
    (&["Control"], &["Control"]),
    (&["Goaded"], &["Goad"]),
    (&["Phased"], &["Phase"]),
    (&["Cloaked"], &["Cloak"]),
    (&["Manifested"], &["Manifest"]),
    (&["Shuffled"], &["Shuffle"]),
    (&["Seeked"], &["Seek"]),
    (&["Discovered"], &["Discover"]),
    (&["Guessed"], &["Guess"]),
    (&["Subgame"], &["Subgame"]),
    (&["Votes"], &["Vote"]),
    (&["Lost"], &["Lose"]),
    (&["Returned", "PutInto", "PutOn", "Left"], &["Move", "Return"]),
    (&["RemovedFromCombat"], &["FromCombat"]),
];

/// A row reporting what an earlier action did, which it must name.
fn reads_last(name: &str) -> bool {
    (name.ends_with("ThisWay") || name.starts_with("Action::") && name.contains("ThisWay"))
        && !name.contains("::Reflexive_")
        && !name.contains("_When")
}

fn vocabulary(ty: &str, value: &serde_json::Value) -> Option<Operand> {
    if value["variant"].as_str().is_some_and(reads_scope) {
        return None;
    }
    let plain = VOCABULARIES.contains(&ty)
        || crate::primitive_spec::registry()
            .vocabularies
            .iter()
            .any(|v| v == ty);
    (plain && value["args"].is_null()).then(|| Operand::Vocabulary {
        family: ty.into(),
        form: value["variant"].as_str().unwrap_or_default().into(),
    })
}

/// Convert any inspected value. Total: everything lacking a typed lowering is a Form.
pub fn operand(value: &serde_json::Value) -> Operand {
    use serde_json::Value as V;
    match value {
        V::Null => Operand::Absent,
        V::Bool(b) => Operand::Bool(*b),
        V::Number(n) => n
            .as_i64()
            .map_or_else(|| Operand::Text(n.to_string()), Operand::Integer),
        V::String(s) => Operand::Text(s.clone()),
        V::Array(items) => Operand::List(items.iter().map(operand).collect()),
        V::Object(fields) => match fields.get("enum").and_then(V::as_str) {
            Some(ty) => vocabulary(ty, value)
                .or_else(|| standalone_operand(ty, value))
                .unwrap_or_else(|| Operand::Form(Box::new(named_node(value)))),
            None => Operand::Record(
                fields
                    .iter()
                    .map(|(name, value)| Field {
                        name: name.clone(),
                        value: operand(value),
                    })
                    .collect(),
            ),
        },
    }
}

fn standalone_operand(ty: &str, value: &serde_json::Value) -> Option<Operand> {
    let result = Context::default().typed_operand(ty, value)?;
    crate::validate::operand(&result).ok()?;
    Some(result)
}

impl Context {
    /// Whether this card may use `name`'s row: a row listing cards was reviewed
    /// for those cards alone.
    fn reviewed(&self, name: &str) -> bool {
        crate::primitive_spec::lookup(name).is_some_and(|row| {
            row.cards.is_empty() || row.cards.iter().any(|card| **card == *self.card)
        })
    }

    fn typed_operand(&self, ty: &str, value: &serde_json::Value) -> Option<Operand> {
        use crate::pipeline::source_tree::decode;
        let scoped = value["variant"].as_str().is_some_and(reads_scope);
        Some(match ty {
            "Permanent" => Operand::Object(self.object(&decode(value).ok()?).ok()?),
            "Permanents" => Operand::Objects(self.objects(&decode(value).ok()?).ok()?),
            "CardsInExile" => {
                let cards = decode(value).ok()?;
                Operand::Objects(match self.exile_cards(&cards) {
                    Ok(cards) => cards,
                    // A compound description of exiled cards, as "exiled this way that ...".
                    Err(_) => ObjectSet::Zone {
                        scope: ZoneScope::Shared(EventZone::Exile),
                        kind: ObjectKind::Card,
                        filter: self.exile_filter(&cards).ok()?,
                    },
                })
            }
            "DamageRecipient" => Operand::Recipients(self.recipients(&decode(value).ok()?).ok()?),
            "SingleDamageRecipient" => {
                Operand::Recipients(match self.single_recipient(&decode(value).ok()?).ok()? {
                    DamageRecipients::Objects(ObjectFilter::SameObject(object)) => {
                        Recipients::Objects(ObjectSet::One(object))
                    }
                    DamageRecipients::Players(players) => Recipients::Players(players),
                    _ => return None,
                })
            }
            "DamageRecipientsList" => {
                use source::DamageRecipientsList as List;
                let list: List = decode(value).ok()?;
                Operand::Recipients(match list {
                    List::APermanent(objects) => {
                        Recipients::Objects(self.objects(&objects).ok()?)
                    }
                    List::APlayer(players) => Recipients::Players(self.players(&players).ok()?),
                    List::APlayerOrAPermanent(players, objects) => Recipients::Union(vec![
                        Recipients::Players(self.players(&players).ok()?),
                        Recipients::Objects(self.objects(&objects).ok()?),
                    ]),
                })
            }
            "Player" => Operand::Player(self.player(&decode(value).ok()?).ok()?),
            "Players" => Operand::Players(self.players(&decode(value).ok()?).ok()?),
            "GameNumber" => Operand::Number(self.number(&decode(value).ok()?).ok()?),
            "Condition" => Operand::Condition(self.condition(&decode(value).ok()?).ok()?),
            "Expiration" => Operand::Duration(self.duration(&decode(value).ok()?).ok()?),
            "Cost" => Operand::Cost(self.cost(&decode(value).ok()?).ok()?),
            // A rule named whole, as "this creature loses" one.
            "CheckHasable" if value["variant"] == "PreventDamage" => {
                let rule: source::Rule = as_family(
                    &decode::<source::CheckHasable>(value).ok()?,
                    "Rule",
                )?;
                Operand::Ability(Box::new(self.fresh().ability(&rule).ok()?))
            }
            "Spells" => Operand::Filter(self.spell_filter(&decode(value).ok()?).ok()?),
            "DamageSources" => Operand::Filter(self.damage_sources(&decode(value).ok()?).ok()?),
            "Cards" => Operand::Filter(self.card_filter(&decode(value).ok()?).ok()?),
            "AbilitySources" => Operand::Filter(self.mana_sources(&decode(value).ok()?).ok()?),
            "CardsInLibrary" => Operand::Filter(self.library_filter(&decode(value).ok()?).ok()?),
            "Comparison" => {
                let (comparison, expression) = self.comparison(&decode(value).ok()?).ok()?;
                Operand::Comparison(comparison, expression)
            }
            "Rule" => {
                let rule: source::Rule = decode(value).ok()?;
                Operand::Ability(Box::new(self.fresh().ability(&rule).ok()?))
            }
            // A perpetual change reads the object it changes as "it".
            "PerpetualEffect" if !self.affected => {
                let mut context = self.clone();
                context.affected = true;
                context.bound_operand(value)?
            }
            "Spell" => Operand::Object(self.spell(&decode(value).ok()?).ok()?),
            "CardInHand" => Operand::Object(self.hand_card(&decode(value).ok()?).ok()?),
            "SpellOrPermanent" => {
                Operand::Object(self.spell_or_permanent(&decode(value).ok()?).ok()?)
            }
            "Trigger" => Operand::Event(Box::new(self.trigger(&decode(value).ok()?).ok()?)),
            "Ability" => match decode(value).ok()? {
                source::Ability::Ref_TargetAbility => {
                    Operand::Object(self.object_target(ObjectDomain::ability(), 0).ok()?)
                }
                source::Ability::Trigger_ThatAbility
                | source::Ability::Trigger_ThatActivatedAbility
                | source::Ability::Trigger_ThatTriggeredAbility
                    if self.event.stack_object =>
                {
                    Operand::Object(ObjectRef::EventStackObject)
                }
                _ => return None,
            },
            "ActivatedAbility" if self.event.stack_object => {
                Operand::Object(ObjectRef::EventStackObject)
            }
            "SpellOrAbility" => Operand::Object(self.stack_object(&decode(value).ok()?).ok()?),
            "SingleCard" => Operand::Object(self.single_card(&decode(value).ok()?).ok()?),
            "CardInLibrary" => match decode(value).ok()? {
                source::CardInLibrary::TheCardConjureIntoLibraryThisWay => {
                    Operand::Object(self.conjured_object().ok()?)
                }
                source::CardInLibrary::TheLibraryCardFoundThisWay => {
                    Operand::Objects(self.found.clone()?)
                }
                _ => return None,
            },
            "CardsInHand" if scoped => Operand::Filter(self.hand_filter(&decode(value).ok()?).ok()?),
            "CardsInGraveyards" if scoped => {
                Operand::Objects(self.grave_cards(&decode(value).ok()?).ok()?)
            }
            "CardsInGraveyards" => Operand::Filter(self.grave_filter(&decode(value).ok()?).ok()?),
            "Targets" => Operand::Recipients(Recipients::Targets(
                self.target(TargetKind::DamageRecipient, 0, 0).ok()?,
            )),
            "SpellsAndAbilities" if scoped => match self.target_on_stack().ok()? {
                ObjectRef::Target { slot, .. } => Operand::Objects(ObjectSet::Targets(slot)),
                _ => return None,
            },
            "ExchangeOwnershipCard" => match decode(value).ok()? {
                source::ExchangeOwnershipCard::Ref_TargetPermanent => {
                    Operand::Object(self.object(&source::Permanent::Ref_TargetPermanent).ok()?)
                }
                source::ExchangeOwnershipCard::TheCardRevealedFromHandThisWay => Operand::Object(
                    self.hand_card(&source::CardInHand::TheCardRevealedFromHandThisWay)
                        .ok()?,
                ),
                source::ExchangeOwnershipCard::TheFirstCardExiledThisWay => {
                    Operand::Object(self.result_object(self.exiled.as_ref(), 0).ok()?)
                }
                source::ExchangeOwnershipCard::TheSecondCardExiledThisWay => {
                    Operand::Object(self.result_object(self.exiled.as_ref(), 1).ok()?)
                }
                source::ExchangeOwnershipCard::ThisPermanent => return None,
            },
            "PlayerOrPermanent" => Operand::Recipients(Recipients::Targets(
                self.target(TargetKind::DamageRecipient, 0, 0).ok()?,
            )),
            "PermanentOrExiledCard" => Operand::Object(
                self.object_target(
                    ObjectDomain {
                        kind: ObjectKind::Any,
                        zones: vec![EventZone::Battlefield, EventZone::Exile],
                    },
                    0,
                )
                .ok()?,
            ),
            "AnteCard" => Operand::Object(
                self.object_target(ObjectDomain::card(EventZone::Ante), 0)
                    .ok()?,
            ),
            "CardInGraveyards" => Operand::Object(self.grave_card(&decode(value).ok()?).ok()?),
            "CardInExile" => Operand::Object(self.exiled_card(&decode(value).ok()?).ok()?),
            "AttackAssignment" if value["variant"] == "ThePlayerOrPlaneswalkerChosenThisWay" => {
                Operand::Object(self.result_object(self.row_choice.as_ref(), 0).ok()?)
            }
            "Commander" if value["variant"] == "TheCommanderChosenThisWay" => {
                Operand::Object(self.result_object(self.row_choice.as_ref(), 0).ok()?)
            }
            "Actions" => {
                let actions: source::Actions = decode(value).ok()?;
                let mut child = self.nested_program();
                let known = child.targets.len();
                child.outer_targets = known;
                let body = child.actions(&actions).ok()?;
                // A body announcing its own targets is an ability, not a nested step.
                if child.targets.len() == known && child.modes.is_none() {
                    Operand::Program(body)
                } else {
                    Operand::Ability(Box::new(Ability::Spell {
                        announcement: Announcement {
                            targets: child.targets.split_off(known),
                            modes: child.modes,
                        },
                        body,
                    }))
                }
            }
            "Action" => {
                let action: source::Action = decode(value).ok()?;
                let mut child = self.nested_program();
                let actor = self.acting.clone().unwrap_or(PlayerRef::AbilityController);
                let instruction = child.action(&action, &PlayerSet::One(actor)).ok()?;
                Operand::Program(Program {
                    instructions: vec![instruction],
                })
            }
            _ => return None,
        })
    }

    /// A nested body closes over this scope; its results stay local to it.
    fn nested_program(&self) -> Self {
        let mut child = self.clone();
        child.result += 1;
        child
    }

    /// Lower an operand in this scope, or fail. A primitive may not hold a `Form`.
    fn bound_operand(&self, value: &serde_json::Value) -> Option<Operand> {
        use serde_json::Value as V;
        Some(match value {
            V::Null => Operand::Absent,
            V::Bool(b) => Operand::Bool(*b),
            V::Number(n) => Operand::Integer(n.as_i64()?),
            V::String(s) => Operand::Text(s.clone()),
            V::Array(items) => match items
                .iter()
                .map(|v| self.bound_operand(v))
                .collect::<Option<Vec<_>>>()
            {
                Some(items) => Operand::List(items),
                // Later actions of a list read what earlier ones did.
                None if items.first().is_some_and(|v| v["enum"] == "Action") => {
                    use crate::pipeline::source_tree::decode;
                    let list = items
                        .iter()
                        .map(|v| decode(v).ok())
                        .collect::<Option<Vec<source::Action>>>()?;
                    let mut child = self.nested_program();
                    Operand::Program(child.actions(&source::Actions::ActionList(list)).ok()?)
                }
                // What a spell does as it resolves, so later steps read what it moved.
                None if items.first().is_some_and(|v| v["enum"] == "ResolveAction") => {
                    use crate::pipeline::source_tree::decode;
                    let controller = PlayerRef::AbilityController;
                    let mut child = self.nested_program();
                    let mut instructions = vec![];
                    for item in items {
                        let step: source::ResolveAction = decode(item).ok()?;
                        instructions.push(match as_family::<source::Action>(&step, "Action") {
                            Some(action) => child
                                .action(&action, &PlayerSet::One(controller.clone()))
                                .ok()?,
                            None => child
                                .primitive_step(
                                    "ResolveAction",
                                    &step,
                                    &controller,
                                    ObjectSet::One(ObjectRef::Affected),
                                )
                                .ok()?,
                        });
                    }
                    Operand::Program(Program { instructions })
                }
                None => return None,
            },
            V::Object(fields) => match fields.get("enum").and_then(V::as_str) {
                Some(ty) => vocabulary(ty, value)
                    .or_else(|| self.typed_operand(ty, value))
                    .or_else(|| self.row_target(value))
                    .or_else(|| self.term(ty, value))
                    .or_else(|| self.scoped_row(ty, value))?,
                None => Operand::Record(
                    fields
                        .iter()
                        .map(|(name, value)| {
                            Some(Field {
                                name: name.clone(),
                                value: self.bound_operand(value)?,
                            })
                        })
                        .collect::<Option<_>>()?,
                ),
            },
        })
    }

    /// `Ref_TargetX` reads the slot the reviewed row `Target::TargetX` announced.
    fn row_target(&self, value: &serde_json::Value) -> Option<Operand> {
        let form = value["variant"].as_str()?.strip_prefix("Ref_")?;
        let name = format!("Target::{form}");
        let spec = self.targets.iter().find(
            |t| matches!(&t.filter, ObjectFilter::Primitive { name: row, .. } if *row == name),
        )?;
        Some(Operand::Objects(ObjectSet::Targets(spec.slot.clone())))
    }

    /// A reference this scope cannot resolve further, kept by its reviewed row.
    /// A "this way" row still points at the instruction it reads.
    fn scoped_row(&self, ty: &str, value: &serde_json::Value) -> Option<Operand> {
        if !value["variant"].as_str().is_some_and(reads_scope) {
            return None;
        }
        let (name, operands) = self.primitive_inspected(ty, value)?;
        Some(match ty {
            "Permanent" | "Spell" | "SpellOrAbility" | "CardInExile" | "CardInHand"
            | "CardInGraveyards" => Operand::Object(crate::semantics::reduce_object(name, operands)),
            "Permanents" | "CardsInExile" | "CardsInHand" | "CardsInGraveyards"
            | "CardsOutsideTheGame" => {
                Operand::Objects(crate::semantics::reduce_objects(name, operands))
            }
            "Player" => Operand::Player(crate::semantics::reduce_player_ref(name, operands)),
            "GameNumber" => Operand::Number(Expr::Primitive { name, operands }),
            "Condition" => Operand::Condition(Condition::Primitive { name, operands }),
            "Expiration" => Operand::Duration(
                crate::semantics::reduce_duration(&name, &operands)
                    .unwrap_or(Duration::Primitive { name, operands }),
            ),
            "Cards" | "CardsInOracle" | "CardsInLibrary" | "CreatureTypeVariable"
            | "CheckHasable" => Operand::Filter(crate::semantics::reduce_filter(name, operands)),
            _ => return None,
        })
    }

    /// A descriptive enum the registry lists, with every operand bound.
    fn term(&self, ty: &str, value: &serde_json::Value) -> Option<Operand> {
        let listed = crate::primitive_spec::registry()
            .terms
            .iter()
            .any(|t| t == ty);
        let reviewed = value["variant"]
            .as_str()
            .is_some_and(|form| crate::primitive_spec::lookup(&format!("{ty}::{form}")).is_some());
        if !listed && !reviewed || value["variant"].as_str().is_some_and(reads_scope) {
            return None;
        }
        let operands = match &value["args"] {
            serde_json::Value::Null => vec![],
            // One list of steps, read as a program rather than step by step.
            serde_json::Value::Array(items)
                if items.first().is_some_and(|v| v["enum"] == "ResolveAction") =>
            {
                vec![self.bound_operand(&value["args"])?]
            }
            serde_json::Value::Array(items) => items
                .iter()
                .map(|v| self.bound_operand(v))
                .collect::<Option<_>>()?,
            other => vec![self.bound_operand(other)?],
        };
        Some(Operand::Term {
            family: ty.into(),
            form: value["variant"].as_str()?.into(),
            operands,
        })
    }

    /// The row name and bound operands of `value`, if it has a reviewed primitive.
    fn primitive(&self, family: &str, value: &impl Serialize) -> Option<(String, Vec<Operand>)> {
        self.primitive_inspected(family, &crate::inspect::value(value).ok()?)
    }

    fn primitive_inspected(
        &self,
        family: &str,
        inspected: &serde_json::Value,
    ) -> Option<(String, Vec<Operand>)> {
        let name = format!("{family}::{}", inspected["variant"].as_str()?);
        let row = crate::primitive_spec::lookup(&name)?;
        if !row.cards.is_empty() && !row.cards.iter().any(|card| **card == *self.card) {
            return None;
        }
        // What the row binds, or the trigger it holds, names things for its operands.
        let mut scoped = self.clone();
        // These adapters can reduce ObjectSet::Zone to a bare ObjectFilter.
        // Preserve IsPermanent's old qualification until their event contracts
        // carry the permanent domain independently. This deliberately withholds
        // the typeless correction here instead of widening unrelated zones.
        scoped.retain_permanent_type_guard |= family == "Trigger"
            || family == "Condition"
            || family == "GameNumber" && name.ends_with("ThisTurn")
            || family.starts_with("ReplacableEvent")
            || family.starts_with("FutureReplacableEvent");
        if !row.binds.is_empty() {
            scoped.event = crate::semantics::with_row(&self.event, &name);
        }
        let operands = match &inspected["args"] {
            serde_json::Value::Null => vec![],
            serde_json::Value::Array(items) => {
                let held = items.iter().find(|v| v["enum"] == "Trigger");
                let held = held.and_then(|v| crate::pipeline::source_tree::decode(v).ok());
                if let Some(event) = held.and_then(|trigger| self.trigger(&trigger).ok()) {
                    scoped.event = crate::semantics::event_bindings(&event);
                }
                items
                    .iter()
                    .map(|v| scoped.bound_operand(v))
                    .collect::<Option<_>>()?
            }
            other => vec![scoped.bound_operand(other)?],
        };
        let mut operands = operands;
        if let Some(result) = self.this_way_result(&name, &operands) {
            operands.push(Operand::Result(result));
        }
        operands.extend(self.row_referent(&name));
        Some((name, operands))
    }

    /// What a row that says "those" or "each" means here, where only this scope knows.
    fn row_referent(&self, name: &str) -> Option<Operand> {
        match name.split("::").nth(1)? {
            "SharesACardtypeWithEachableExiledPermanent" => {
                let each = self.iterated_object.clone()?;
                Some(Operand::Object(ObjectRef::Bound(each)))
            }
            "Trigger_ThoseCreatures" => self.group_effect.clone().map(Operand::Result),
            _ => None,
        }
    }

    /// Record what a reviewed row says this operation did.
    fn note_yield(&mut self, operation: &Operation, result: &ResultId) {
        // Each player drew their own, so a later read is of that player's share.
        if matches!(operation, Operation::DrawUpTo { players, .. } if !matches!(players, PlayerSet::One(_)))
        {
            self.looped.insert(result.0);
        }
        let yields = crate::semantics::yields(operation);
        match yields {
            Some("observed") => self.revealed = Some(ObjectSet::Observed(result.clone())),
            Some("exiled") => {
                self.exiled = Some(result.clone());
                self.exiled_permanents = false;
                self.did.insert(Done::Exiled, result.clone());
            }
            Some("entered") => self.entered = Some(result.clone()),
            Some("hand") => self.to_hand = Some(result.clone()),
            Some("created") => self.created_tokens = Some(result.clone()),
            Some("amassed") => self.amassed = Some(result.clone()),
            Some("paid") => {
                self.did.insert(Done::Paid, result.clone());
            }
            Some("graveyard") => {
                self.did.insert(Done::Graveyard, result.clone());
            }
            Some("drew") => {
                self.did.insert(Done::Drew, result.clone());
            }
            Some("milled") => {
                self.did.insert(Done::Milled, result.clone());
                self.did.insert(Done::Graveyard, result.clone());
            }
            Some("destroyed") => {
                self.did.insert(Done::Destroyed, result.clone());
                self.did.insert(Done::Graveyard, result.clone());
            }
            Some("took") => self.took = Some(result.clone()),
            Some("discarded") => {
                self.did.insert(Done::Discarded, result.clone());
                self.did.insert(Done::Graveyard, result.clone());
            }
            Some("counters") => {
                self.did.insert(Done::Counters, result.clone());
            }
            Some("effect") => {
                self.did.insert(Done::Effect, result.clone());
            }
            Some("life_lost") => {
                self.did.insert(Done::LifeLost, result.clone());
            }
            Some("extra_turn") => self.extra_turn = Some(result.clone()),
            Some("damage") => {
                self.did.insert(Done::Damage, result.clone());
            }
            Some("seeked") => self.seeked.push(result.clone()),
            Some("chosen") => self.row_choice = Some(result.clone()),
            _ => {}
        }
    }

    /// A step over cards a reveal or search found, named by its reviewed row.
    fn primitive_step(
        &mut self,
        family: &str,
        value: &impl Serialize,
        actor: &PlayerRef,
        objects: ObjectSet,
    ) -> Result<Instruction> {
        let (name, mut operands) = self
            .primitive(family, value)
            .ok_or_else(|| unsupported(value))?;
        if !crate::primitive_spec::lookup(&name).is_some_and(|row| row.keeps) {
            self.forget_results();
        }
        operands.insert(0, Operand::Objects(objects));
        let step = self.effect(crate::semantics::reduce_inspection(
            PlayerSet::One(actor.clone()),
            name,
            operands,
        ));
        if let Instruction::Effect { result, operation } = &step {
            self.note_yield(operation, result);
        }
        Ok(step)
    }

    /// A perpetual effect is an ordinary layer change that outlives its zone.
    /// Whichever objects it names, the row has already bound them.
    pub(super) fn perpetual(
        &self,
        value: &source::Action,
        subject: Option<&Operand>,
    ) -> Option<Operation> {
        use source::Action as S;
        let effects = match value {
            S::CreatePerpetualTriggeredCardEffect(effects)
            | S::CreatePerpetualCardsInHandEffect(_, effects)
            | S::CreatePerpetualPermanentEffect(_, effects)
            | S::CreatePerpetualCardInHandEffect(_, effects)
            | S::CreatePerpetualCardInLibraryEffect(_, effects)
            | S::CreatePerpetualEachPermanentEffect(_, effects)
            | S::CreatePerpetualExiledCardEffect(_, effects)
            | S::CreatePerpetualEachExiledCardEffect(_, effects)
            | S::CreatePerpetualGraveyardCardEffect(_, effects)
            | S::CreatePerpetualEachGraveyardCardEffect(_, effects)
            | S::CreatePerpetualSpellEffect(_, effects)
            | S::CreatePerpetualCardsInPlayersHandEffect(_, _, effects)
            | S::CreatePerpetualCardsInEachPlayersHandEffect(_, _, effects)
            | S::CreatePerpetualCardsInPlayersLibraryEffect(_, _, effects)
            | S::CreatePerpetualCardsInPlayersGraveyardEffect(_, _, effects) => effects,
            _ => return None,
        };
        let hand = |filter| {
            let you = PlayerSet::One(self.player(&source::Player::You).ok()?);
            Some(self.private_cards(EventZone::Hand, you, filter))
        };
        let exiled_followup = match value {
            S::CreatePerpetualExiledCardEffect(card, _) => self.exiled_followup_group(card),
            _ => None,
        };
        let objects = match (value, subject) {
            _ if exiled_followup.is_some() => self.still_exiled(exiled_followup?),
            (S::CreatePerpetualTriggeredCardEffect(_), _) if self.event.moved => {
                ObjectSet::One(ObjectRef::EventDestination)
            }
            (S::CreatePerpetualCardInHandEffect(card, _), _) => {
                ObjectSet::One(self.hand_card(card).ok()?)
            }
            (S::CreatePerpetualCardsInHandEffect(cards, _), _) => {
                hand(self.hand_filter(cards).ok()?)?
            }
            (S::CreatePerpetualCardsInPlayersHandEffect(cards, player, _), _) => self.private_cards(
                EventZone::Hand,
                PlayerSet::One(self.player(player).ok()?),
                self.hand_filter(cards).ok()?,
            ),
            (S::CreatePerpetualCardsInEachPlayersHandEffect(cards, players, _), _) => self
                .private_cards(
                    EventZone::Hand,
                    self.players(players).ok()?,
                    self.hand_filter(cards).ok()?,
                ),
            (S::CreatePerpetualCardsInPlayersLibraryEffect(cards, player, _), _) => self
                .private_cards(
                    EventZone::Library,
                    PlayerSet::One(self.player(player).ok()?),
                    self.library_filter(cards).ok()?,
                ),
            (S::CreatePerpetualCardsInPlayersGraveyardEffect(cards, player, _), _) => self
                .private_cards(
                    EventZone::Graveyard,
                    PlayerSet::One(self.player(player).ok()?),
                    self.grave_filter(cards).ok()?,
                ),
            (
                S::CreatePerpetualCardInLibraryEffect(
                    source::CardInLibrary::ARandomCardOfTypeFromPlayersLibrary(cards, player),
                    _,
                ),
                _,
            ) => ObjectSet::DraftRandom {
                objects: Box::new(self.private_cards(
                    EventZone::Library,
                    PlayerSet::One(self.player(player).ok()?),
                    self.card_filter(cards).ok()?,
                )),
                count: Expr::Constant(1),
            },
            (_, Some(Operand::Object(object))) => ObjectSet::One(object.clone()),
            (_, Some(Operand::Objects(objects))) => objects.clone(),
            _ => return None,
        };
        let mut changes = vec![];
        for effect in effects {
            match effect {
                source::PerpetualEffect::Incorporate(mana) => changes.push(Change::Incorporate(
                    mana.iter().map(mana_symbol).collect::<Result<_>>().ok()?,
                )),
                effect => {
                    let layer: source::LayerEffect = as_family(effect, "LayerEffect")?;
                    changes.extend(self.changes(&[layer]).ok()?);
                }
            }
        }
        Some(Operation::CreatePerpetualEffect { objects, changes })
    }

    /// A change to a spell, or the row when any effect has no generic form.
    fn spell_change(&self, value: &source::Action) -> Option<Operation> {
        use source::Action as S;
        use source::FutureSpell as F;
        let (subject, effects) = match value {
            S::CreateSpellEffect(spell, effects) => {
                (SpellSubject::One(self.spell(spell).ok()?), effects)
            }
            S::CreateFutureSpellEffect(next, effects) => {
                let (spells, player, this_turn, from_hand) = match next {
                    F::TheNextSpellPlayerCasts(spells, player) => (spells, player, false, false),
                    F::TheNextSpellPlayerCastsThisTurn(spells, player) => {
                        (spells, player, true, false)
                    }
                    F::TheNextSpellPlayerCastsFromTheirHandThisTurn(spells, player) => {
                        (spells, player, true, true)
                    }
                };
                let subject = SpellSubject::Next {
                    spells: self.spell_filter(spells).ok()?,
                    player: self.player(player).ok()?,
                    this_turn,
                    from_hand,
                };
                (subject, effects)
            }
            _ => return None,
        };
        let mut changes = vec![];
        for effect in effects {
            changes.extend(self.spell_effect(effect)?);
        }
        Some(Operation::ChangeSpell { subject, changes })
    }

    fn spell_effect(&self, value: &source::SpellEffect) -> Option<Vec<Change>> {
        use source::SpellEffect as S;
        let reduced = |symbols: &[source::CostReductionSymbol], factor| {
            Some(Change::SpellCost(SpellCostModifier::Reduce {
                reductions: self.mana_reduction(symbols, factor).ok()?,
                spill_into_generic: true,
            }))
        };
        let change = match value {
            S::EntersWithLayerEffectUntil(effects, until) => Change::Continuing {
                changes: self.changes(effects).ok()?,
                until: self.duration(until).ok()?,
            },
            S::EntersWithLayerEffect(effects) => Change::Continuing {
                changes: self.changes(effects).ok()?,
                until: Duration::Indefinite,
            },
            S::EntersWithACounter(kind) => Change::EntersWithCounters {
                kind: crate::counter::lower(kind),
                amount: Expr::Constant(1),
            },
            S::EntersWithNumberCounters(amount, kind) => Change::EntersWithCounters {
                kind: crate::counter::lower(kind),
                amount: self.number(amount).ok()?,
            },
            S::DecreaseManaCost(symbols) => reduced(symbols, Expr::Constant(1))?,
            S::DecreaseManaCostForEach(symbols, each) => {
                reduced(symbols, self.number(each).ok()?)?
            }
            S::IncreaseManaCost(mana) => Change::SpellCost(SpellCostModifier::Increase(Cost {
                parts: vec![CostPart::Mana(
                    mana.iter().map(mana_symbol).collect::<Result<_>>().ok()?,
                )],
            })),
            S::CantBeCountered => Change::CantBeCountered,
            _ => {
                if let Some(layer) = as_family::<source::LayerEffect>(value, "LayerEffect") {
                    return self.layer_changes(&[layer]).ok();
                }
                let rule: source::Rule = as_family(value, "Rule")?;
                Change::GrantAbility(Box::new(self.fresh().ability(&rule).ok()?))
            }
        };
        Some(vec![change])
    }

    /// Its result is unknown, so earlier antecedents are forgotten as for a named action.
    fn primitive_action(
        &mut self,
        value: &source::Action,
        actor: &PlayerSet,
    ) -> Option<Instruction> {
        // A spell's own changes are typed, so their rows bind nothing
        if let Some(operation) = self.spell_change(value) {
            return Some(self.effect(operation));
        }
        let (name, operands) = self.primitive("Action", value)?;
        if !crate::primitive_spec::lookup(&name)?.keeps {
            self.forget_results();
        }
        let result = ResultId(self.result);
        self.result += 1;
        self.reported.push((result.clone(), name.clone()));
        if name.starts_with("Action::Roll") {
            self.rolled = Some(result.clone());
        }
        if name == "Action::FlipCoins" {
            self.flipped = Some(result.clone());
        }
        if name.starts_with("Action::Conjure") || name.starts_with("Action::Draft") {
            self.conjured = Some(result.clone());
        }
        let mut operands = operands;
        if name.starts_with("Action::Copy") && name.contains("Card") {
            self.copied = Some(result.clone());
        } else if name.contains("CopiedCard") {
            if let Some(copies) = &self.copied {
                operands.push(Operand::Objects(ObjectSet::Copies(copies.clone())));
            }
        }
        let operation = self
            .perpetual(value, operands.first())
            .unwrap_or_else(|| crate::semantics::reduce_action(actor.clone(), name, operands));
        self.note_yield(&operation, &result);
        if let Operation::ChooseObjects { player, .. } = &operation {
            self.picks.push((Some(player.clone()), result.clone()));
        }
        if crate::semantics::singular_operation(&operation, |_| false) {
            self.singular.insert(result.0);
        }
        self.note_done(&operation, &result);
        if let Operation::Keyword {action,..}=&operation {
            match action {
                KeywordAction::PhaseOut {object}=>self.phased=Some(object.clone()),
                KeywordAction::Transform {object}=>self.acted_on=Some(object.clone()),
                KeywordAction::Bolster {..}=>self.acted_on=Some(ObjectRef::Result {result:result.clone(),phase:ObjectPhase::After,index:0}),
                _=>{}
            }
        }
        Some(Instruction::Effect { result, operation })
    }

    /// Rules whose cost is printed mana, a list no operand carries as one cost.
    fn printed_cost_rule(&self, value: &source::Rule) -> Option<Ability> {
        let cost = |symbols: Vec<ManaSymbol>| Cost {
            parts: vec![CostPart::Mana(symbols)],
        };
        let plain = |symbols: &source::ManaCost| -> Option<Vec<ManaSymbol>> {
            symbols.iter().map(|s| mana_symbol(s).ok()).collect()
        };
        let with_x = |symbols: &source::ManaCostX| -> Option<Vec<ManaSymbol>> {
            symbols.iter().map(|s| mana_symbol_x(s).ok()).collect()
        };
        let mechanic = |mechanic, cost| Ability::Mechanic {
            mechanic,
            cost: Some(cost),
            amount: None,
            filter: None,
        };
        Some(match value {
            source::Rule::Prowl(symbols) => mechanic(Mechanic::Prowl, cost(plain(symbols)?)),
            source::Rule::Surge(symbols) => mechanic(Mechanic::Surge, cost(plain(symbols)?)),
            source::Rule::SurgeX(symbols) => mechanic(Mechanic::SurgeX, cost(with_x(symbols)?)),
            source::Rule::Prototype { mana_cost, card_pt } => Ability::Prototype {
                mana: with_x(mana_cost)?,
                power: card_pt.power,
                toughness: card_pt.toughness,
            },
            _ => return None,
        })
    }

    fn primitive_ability(&self, value: &source::Rule) -> Option<Ability> {
        if let Some(ability) = self.printed_cost_rule(value) {
            return Some(ability);
        }
        // One ability with an effect for each of its events: each reads its own.
        if let source::Rule::TriggerModalA(options) = value {
            let abilities = options
                .iter()
                .map(|source::TriggerAndActions(trigger, actions)| {
                    let rule = source::Rule::TriggerA(trigger.clone(), actions.clone());
                    Some(Operand::Ability(Box::new(self.fresh().ability(&rule).ok()?)))
                })
                .collect::<Option<_>>()?;
            return Some(crate::semantics::reduce_ability(
                "Rule::TriggerModalA".into(),
                vec![Operand::List(abilities)],
            ));
        }
        let (name, operands) = match value {
            source::Rule::DeckConstruction(deck) => self.primitive("DeckConstruction", deck)?,
            _ => self.primitive("Rule", value)?,
        };
        Some(crate::semantics::reduce_ability(name, operands))
    }

    fn primitive_cost(&self, value: &source::Cost) -> Option<Cost> {
        let (name, operands) = self.primitive("Cost", value)?;
        let payer = self.payer.clone().unwrap_or(PlayerRef::AbilityController);
        if let Some(parts) = crate::semantics::reduce_card_cost(&name, &operands, &payer) {
            return Some(Cost { parts });
        }
        let part = match (name.as_str(), operands.as_slice()) {
            ("Cost::PayEnergy", [Operand::Number(amount)]) => CostPart::Energy(amount.clone()),
            _ => match self.typed_cost(value) {
                Some(part) => part,
                None => crate::semantics::reduce_cost(name, operands),
            },
        };
        Some(Cost { parts: vec![part] })
    }

    fn activation_rule(&self, value: &source::ActivateModifier) -> Option<ActivationRule> {
        use source::ActivateModifier as S;
        let reduce = |symbols: &[source::CostReductionSymbol], factor| {
            Some(SpellCostModifier::Reduce {
                reductions: self.mana_reduction(symbols, factor).ok()?,
                spill_into_generic: true,
            })
        };
        Some(match value {
            S::ReduceCostForEach(symbols, each) => ActivationRule::Cost {
                modifier: reduce(symbols, self.number(each).ok()?)?,
                condition: None,
            },
            S::ReduceCostIf(condition, symbols) => {
                let mut context = self.clone();
                context.activation_limit = true;
                ActivationRule::Cost {
                    modifier: reduce(symbols, Expr::Constant(1))?,
                    condition: Some(context.condition(condition).ok()?),
                }
            }
            S::IncreaseManaCostForEach(mana, each) => ActivationRule::Cost {
                modifier: SpellCostModifier::Increase(Cost {
                    parts: vec![CostPart::Repeated {
                        part: Box::new(CostPart::Mana(
                            mana.iter().map(mana_symbol).collect::<Result<_>>().ok()?,
                        )),
                        times: self.number(each).ok()?,
                    }],
                }),
                condition: None,
            },
            S::ActivateNoMoreThanNumberTimesEachTurn(times) => {
                ActivationRule::MaxPerTurn(self.number(times).ok()?)
            }
            S::XCantBeZero => ActivationRule::XNotZero,
            S::XCantBeGreaterThan(limit) => ActivationRule::XAtMost(self.number(limit).ok()?),
            S::XIs(value) => ActivationRule::XIs(self.number(value).ok()?),
            S::CantBeCopied => ActivationRule::CantBeCopied,
            S::OnlyPlayerMayActivate(player) => {
                ActivationRule::OnlyBy(PlayerSet::One(self.player(player).ok()?))
            }
            S::OnlyOtherPlayersMayActivate(players) => {
                ActivationRule::OnlyBy(self.players(players).ok()?)
            }
            S::ActivateOnlyAsAnInstant => ActivationRule::AnyTimeOfInstant,
            _ => return None,
        })
    }

    fn typed_cost(&self, value: &source::Cost) -> Option<CostPart> {
        use source::Cost as S;
        let plain = |mana: &source::ManaCost| -> Option<Vec<ManaSymbol>> {
            mana.iter().map(mana_symbol).collect::<Result<_>>().ok()
        };
        let with_x = |mana: &source::ManaCostX| -> Option<Vec<ManaSymbol>> {
            mana.iter().map(mana_symbol_x).collect::<Result<_>>().ok()
        };
        Some(match value {
            S::PayManaAnyX(mana) => CostPart::ManaWithX {
                mana: with_x(mana)?,
                x: None,
            },
            S::PayManaX(mana, x) => CostPart::ManaWithX {
                mana: with_x(mana)?,
                x: Some(self.number(x).ok()?),
            },
            S::Teamwork(power) => CostPart::Teamwork(*power),
            S::Waterbend(mana) => CostPart::Waterbend(plain(mana)?),
            S::WaterbendX(mana) => CostPart::Waterbend(with_x(mana)?),
            S::LoyaltyMinusX => CostPart::Loyalty(Expr::Multiply(
                Box::new(Expr::Constant(-1)),
                Box::new(Expr::AnnouncedX),
            )),
            S::PutCounters(placements) => CostPart::PutCounters(
                placements
                    .iter()
                    .map(|placement| self.paid_counters(placement))
                    .collect::<Option<_>>()?,
            ),
            _ => return None,
        })
    }

    /// Counters placed as a cost: exact objects only, since a cost's choice
    /// of object stays with the row that names it.
    fn paid_counters(&self, value: &source::PutCountersCost) -> Option<CounterPlacement> {
        use source::PutCountersCost as S;
        let (recipients, kind, amount) = match value {
            S::ACounterOfTypeOnPermanent(kind, object) => (
                Recipients::Objects(ObjectSet::One(self.object(object).ok()?)),
                kind,
                Expr::Constant(1),
            ),
            S::NumberCountersOfTypeOnPermanent(amount, kind, object) => (
                Recipients::Objects(ObjectSet::One(self.object(object).ok()?)),
                kind,
                self.number(amount).ok()?,
            ),
            S::NumberCountersOfTypeOnPlayer(amount, kind, player) => (
                Recipients::Players(PlayerSet::One(self.player(player).ok()?)),
                kind,
                self.number(amount).ok()?,
            ),
            S::ACounterOfTypeOnCardInExile(kind, card) => (
                Recipients::Objects(ObjectSet::One(self.exiled_card(card).ok()?)),
                kind,
                Expr::Constant(1),
            ),
            S::ACounterOfTypeOnAPermanent(kind, objects) => {
                return Some(CounterPlacement {
                    recipients: Recipients::Objects(self.objects(objects).ok()?),
                    kind: crate::counter::lower(kind),
                    amount: Expr::Constant(1),
                    selection: Some(SelectionChoice {
                        player: self.payer.clone().unwrap_or(PlayerRef::AbilityController),
                        count: Count::Exactly(Expr::Constant(1)),
                    }),
                })
            }
        };
        Some(CounterPlacement {
            recipients,
            kind: crate::counter::lower(kind),
            amount,
            selection: None,
        })
    }

    fn primitive_condition(&self, value: &source::Condition) -> Option<Condition> {
        let (name, operands) = self.primitive("Condition", value)?;
        Some(crate::semantics::reduce_condition(name, operands))
    }

    fn primitive_filter(&self, value: &source::Permanents) -> Option<ObjectFilter> {
        self.primitive_predicate("Permanents", value)
    }

    fn primitive_predicate(&self, family: &str, value: &impl Serialize) -> Option<ObjectFilter> {
        let (name, operands) = self.primitive(family, value)?;
        Some(crate::semantics::reduce_filter(name, operands))
    }

    fn primitive_object(&self, value: &source::Permanent) -> Option<ObjectRef> {
        let (name, operands) = self.primitive("Permanent", value)?;
        Some(match (name.as_str(), operands.as_slice()) {
            ("Permanent::ThisGuest", []) => ObjectRef::Granter,
            ("Permanent::SingleTargetPermanentOfSpell", [Operand::Object(spell)]) => {
                ObjectRef::SoleTargetOf(Box::new(spell.clone()))
            }
            ("Permanent::TheCreaturePairedWithPermanent", [Operand::Object(object)]) => {
                ObjectRef::PairedWith(Box::new(object.clone()))
            }
            _ => crate::semantics::reduce_object(name, operands),
        })
    }

    fn primitive_player(&self, value: &source::Player) -> Option<PlayerRef> {
        let (name, operands) = self.primitive("Player", value)?;
        Some(match (name.as_str(), operands.as_slice()) {
            ("Player::ClashOpponent", []) => PlayerRef::ClashOpponent,
            // The targets share one graveyard, so its owner owns the first.
            ("Player::SingleGraveyardOwner", []) => match self.targets.first() {
                Some(spec) => PlayerRef::OwnerOf(Box::new(ObjectRef::Target {
                    slot: spec.slot.clone(),
                    index: 0,
                })),
                None => crate::semantics::reduce_player_ref(name, operands),
            },
            (
                "Player::LoseLife_ThatPlayer"
                | "Player::PlayerAction_ThatPlayer"
                | "Player::SpellDealsDamage_ThatPlayer"
                | "Player::DealsDamage_ThatPlayer"
                | "Player::Condition_ThatPlayer",
                [],
            ) => PlayerRef::Acting,
            ("Player::TheGiftedPlayer", []) => PlayerRef::GiftedPlayer,
            ("Player::PlayerOrControllerOfPermanent", [Operand::Recipients(recipient)]) => {
                PlayerRef::PlayerOrControllerOf(Box::new(recipient.clone()))
            }
            (name_, operands_) => {
                let form = name_.split("::").nth(1).unwrap_or(name_);
                match crate::semantics::reduce_player(form, operands_) {
                    Some(player) => player,
                    None => crate::semantics::reduce_player_ref(name, operands),
                }
            }
        })
    }

    fn primitive_number(&self, value: &source::GameNumber) -> Option<Expr> {
        let (name, mut operands) = self.primitive("GameNumber", value)?;
        // Which payments went unmade is read off the loop that offered them.
        if name == "GameNumber::TheNumberOfRepeatedCostsNotPaidByPlayer" {
            let offered = self
                .reported
                .iter()
                .rev()
                .find(|(_, row)| row == "Action::EachPlayerRepeatedMayCost");
            if let Some((result, _)) = offered {
                operands.push(Operand::Result(result.clone()));
            }
        }
        if name == "GameNumber::ManaValueOfTheCardDiscardedThisWay" {
            use crate::number_spec::PaymentAction;
            if let Ok(object) = self.paid_object(PaymentAction::Discard) {
                return Some(Expr::ObjectProperty {
                    object,
                    property: Property::ManaValue,
                    information: Information::CurrentOrLastKnown,
                });
            }
        }
        // A row may read an amount only where the event carries one.
        match crate::semantics::reduce_number(name.clone(), operands.clone()) {
            Expr::EventAmount if !self.event.amount => Some(Expr::Primitive { name, operands }),
            reduced => Some(reduced),
        }
    }

    /// A scalar-only source position (for example one characteristic type).
    fn primitive_change(&self, value: &source::LayerEffect) -> Option<Change> {
        let changes = self.primitive_changes(value)?;
        match changes.as_slice() { [one] => Some(one.clone()), _ => None }
    }

    fn primitive_changes(&self, value: &source::LayerEffect) -> Option<Vec<Change>> {
        if let source::LayerEffect::IsACopyOfThatCard(effects) = value {
            return self.primitive_changes(&source::LayerEffect::IsACopyOfLibraryCard(
                source::CardInLibrary::TheLibraryCardFoundThisWay,
                effects.clone(),
            ));
        }
        let (name, operands) = self.primitive("LayerEffect", value)?;
        Some(crate::semantics::reduce_changes(name, operands))
    }

    /// The tested player comes first, then the form's own operands.
    fn primitive_player_check(
        &self,
        player: &PlayerRef,
        value: &source::Players,
    ) -> Option<Condition> {
        if let source::Players::HasACardInGraveyard(cards) = value {
            if let Ok(ObjectSet::Zone { kind, filter, .. }) = self.grave_cards(cards) {
                return Some(Condition::Compare {
                    left: Expr::CountObjects(Box::new(ObjectSet::Zone {
                        scope: ZoneScope::Players {
                            zone: EventZone::Graveyard,
                            players: PlayerSet::One(player.clone()),
                        },
                        kind,
                        filter,
                    })),
                    comparison: Comparison::GreaterOrEqual,
                    right: Expr::Constant(1),
                });
            }
        }
        let (name, mut operands) = self.primitive("Players", value)?;
        operands.insert(0, Operand::Player(player.clone()));
        Some(crate::semantics::reduce_condition(name, operands))
    }

    fn primitive_trigger(&self, value: &source::Trigger) -> Option<EventSpec> {
        let (name, operands) = self.primitive("Trigger", value)?;
        Some(crate::semantics::reduce_event(name, operands))
    }

    /// The nearest instruction doing what a "this way" row reports.
    fn this_way_result(&self, name: &str, operands: &[Operand]) -> Option<ResultId> {
        if !reads_last(name) {
            return None;
        }
        let form = name.split("::").nth(1).unwrap_or(name);
        // A later step that takes a card back out of exile also says "Exile".
        if let ("TheOtherPermanentsExiledThisWay", Some(exiled)) = (form, &self.exiled) {
            return Some(exiled.clone());
        }
        let words = THIS_WAY
            .iter()
            .find(|(row, _)| row.iter().any(|word| form.contains(word)))
            .map_or(&[][..], |(_, words)| *words);
        let mut candidates = self
            .reported
            .iter()
            .rev()
            .filter(|(_, text)| !text.contains(form))
            .filter(|(_, text)| words.is_empty() || words.iter().any(|word| text.contains(word)));
        // What a player put onto the battlefield is that player's move there,
        // not a later move of the rest elsewhere (Guild Feud).
        if form.contains("Battlefield") {
            let onto: Vec<_> = candidates
                .clone()
                .filter(|(_, text)| text.contains("destination: Battlefield("))
                .collect();
            let by = operands.iter().find_map(|operand| match operand {
                Operand::Player(player) => Some(format!("destination: Battlefield({player:?})")),
                _ => None,
            });
            let theirs = by.and_then(|by| onto.iter().find(|(_, text)| text.contains(&by)));
            if let Some((result, _)) = theirs.or(onto.first()) {
                return Some(result.clone());
            }
        }
        candidates.next().map(|(result, _)| result.clone())
    }

    fn named_action(&mut self, value: &source::Action, actor: &PlayerSet) -> Instruction {
        self.forget_results();
        let result = ResultId(self.result);
        self.result += 1;
        self.reported.push((result.clone(), format!("{:?}", named_value(value))));
        Instruction::Effect {
            result,
            operation: Operation::Named {
                actor: actor.clone(),
                form: named_value(value),
            },
        }
    }

    /// The named action may have produced what a later "that" means, so anything
    /// bound before it could be the wrong antecedent. Later references must fail.
    fn forget_results(&mut self) {
        self.coin_players = None;
        self.forgotten = true;
        self.created_tokens = None;
        self.attachment_host = None;
        self.amassed = None;
        self.seeked.clear();
        self.conjured = None;
        self.acted_on = None;
        self.row_choice = None;
        self.performer = None;
        self.tapped = None;
        self.chosen_pile = None;
        self.found = None;
        self.took = None;
        self.repeated = None;
        self.offered = None;
        self.did.clear();
        self.payment = None;
        self.exiled = None;
        self.exiled_one_input = None;
        self.exiled_permanents = false;
        self.entered = None;
        self.entered_from_one = None;
        self.inspection_hand = None;
        self.paid.values_mut().for_each(|binding| *binding = None);
        self.acted.clear();
    }
}

#[cfg(test)]
mod named_tests {
    use super::*;

    #[test]
    fn a_named_action_keeps_its_actor_and_clears_antecedents() {
        let mut context = Context {
            created_tokens: Some(ResultId(0)),
            exiled: Some(ResultId(0)),
            entered: Some(ResultId(0)),
            payment: Some(ResultId(0)),
            result: 3,
            ..Context::default()
        };
        let actor = PlayerSet::OpponentsOf(PlayerRef::AbilityController);
        let instruction = context.named_action(&source::Action::Recruit, &actor);
        let Instruction::Effect { result, operation } = instruction else {
            panic!("named actions are effects")
        };
        let Operation::Named { actor: kept, form } = operation else {
            panic!("expected a named operation")
        };
        assert_eq!(result.0, 3);
        assert_eq!(context.result, 4);
        assert_eq!(form.form, "Recruit");
        assert_eq!(
            serde_json::to_value(kept).unwrap(),
            serde_json::to_value(actor).unwrap()
        );
        assert!(context.created_tokens.is_none());
        assert!(context.exiled.is_none());
        assert!(context.entered.is_none());
        assert!(context.payment.is_none());
    }
}
