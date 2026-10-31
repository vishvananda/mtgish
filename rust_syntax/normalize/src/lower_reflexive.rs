/// Why a reflexive trigger's event is the base action having happened. Forms that
/// test what happened carry an operand and stay out of this list.
const REFLEXIVE_EVENTS: [&str; 13] = [
    "YouDo",
    "TheyDo",
    "ItConnivesThisWay",
    "ACardIsDiscardedThisWay",
    "ACardIsExiledThisWay",
    "ACardIsDrawnThisWay",
    "APermanentEntersThisWay",
    "ThatPermanentEnters",
    "OneOrMoreAreRemovedThisWay",
    "OneOrMoreCardsAreMilledThisWay",
    "YouDiscardOneOrMoreCardsThisWay",
    "YouPutAnyNumberOfPermanentsOnTheBattlefieldThisWay",
    "YouSearchYourLibraryThisWay",
];

enum Base {
    Action(source::Action),
    Cost(source::Cost),
}

fn base_form(name: &str, args: &serde_json::Value) -> Option<Base> {
    use crate::pipeline::source_tree::decode;
    let form = |ty| serde_json::json!({ "enum": ty, "variant": name, "args": args });
    let mut action = form("Action");
    retag(&mut action, &|n: &str| (n == "CardsInHand").then_some("Cards"));
    decode(&action)
        .map(Base::Action)
        .or_else(|_| decode(&form("Cost")).map(Base::Cost))
        .ok()
}

/// The base with these operands, or, where they are a wrapper enum such as
/// `Sacrificable`, the verb joined to each wrapper's variant: `SacrificeAPermanent`.
fn base_forms(name: &str, args: &serde_json::Value) -> Option<Vec<Base>> {
    if let Some(base) = base_form(name, args) {
        return Some(vec![base]);
    }
    let wrappers = match args {
        serde_json::Value::Array(items) => items.as_slice(),
        one => std::slice::from_ref(one),
    };
    wrappers
        .iter()
        .map(|w| base_form(&format!("{name}{}", w["variant"].as_str()?), &w["args"]))
        .collect()
}

fn last_result(instruction: &Instruction) -> Option<ResultId> {
    match instruction {
        Instruction::Effect { result, .. } => Some(result.clone()),
        Instruction::Group(program) => program.instructions.last().and_then(last_result),
        _ => None,
    }
}

impl Context {
    /// `Reflexive_<Base>_When<Event>` is the action or cost `Base` with the same
    /// operands, then a reflexive trigger. The variant name says all of that, so
    /// no form needs its own code. `None` means this is not such a form.
    fn reflexive_action(
        &mut self,
        value: &source::Action,
        actor: &PlayerSet,
    ) -> Option<Result<Instruction>> {
        use crate::pipeline::source_tree::decode;
        let inspected = crate::inspect::value(value).ok()?;
        let variant = inspected["variant"].as_str()?.strip_prefix("Reflexive_")?;
        let (base, event) = variant.split_once("_When")?;
        let (event, conditional) = match event.strip_suffix("_If") {
            Some(event) => (event, true),
            None => (event, false),
        };
        let filtered = matches!(
            event,
            "ACardOfTypeIsDiscardedThisWay" | "YouDiscardOneOrMoreCardsOfTypeThisWay"
        );
        if !REFLEXIVE_EVENTS.contains(&event) && !filtered {
            return None;
        }
        let mut args = match &inspected["args"] {
            serde_json::Value::Array(args) => args.clone(),
            arg => vec![arg.clone()],
        };
        let actions: source::Actions = decode(&args.pop()?).ok()?;
        let condition: Option<source::Condition> = match (conditional, filtered) {
            (true, _) => Some(decode(&args.pop()?).ok()?),
            (_, true) => {
                let mut cards = args.pop()?;
                retag(&mut cards, &|n: &str| {
                    matches!(n, "CardsInHand" | "CardsInLibrary").then_some("Cards")
                });
                Some(source::Condition::ACardWasDiscardedThisWay(decode(&cards).ok()?))
            }
            _ => None,
        };
        let args = match args.len() {
            0 => serde_json::Value::Null,
            1 => args.pop()?,
            _ => serde_json::Value::Array(args),
        };
        let bases = base_forms(base, &args)?;
        let before = self.clone();
        let lowered = self.reflexive_body(&bases, condition.as_ref(), &actions, actor);
        if lowered.is_err() {
            *self = before;
        }
        Some(lowered)
    }

    fn reflexive_body(
        &mut self,
        bases: &[Base],
        condition: Option<&source::Condition>,
        actions: &source::Actions,
        actor: &PlayerSet,
    ) -> Result<Instruction> {
        let mut instructions = Vec::new();
        for base in bases {
            instructions.push(match base {
                Base::Action(action) => self.action(action, actor)?,
                Base::Cost(cost) => {
                    let PlayerSet::One(payer) = actor else {
                        return Err(invalid("payment requires one explicit payer"));
                    };
                    let mut context = self.clone();
                    context.payer = Some(payer.clone());
                    let cost = context.cost(cost)?;
                    self.effect(Operation::PayCost {
                        cost,
                        player: payer.clone(),
                        optional: false,
                    })
                }
            });
        }
        let condition = condition.map(|c| self.condition(c)).transpose()?;
        self.reflexive_after(instructions, condition, actions)
    }

    /// The base already lowered, then its reflexive trigger.
    fn reflexive_after(
        &mut self,
        mut instructions: Vec<Instruction>,
        condition: Option<Condition>,
        actions: &source::Actions,
    ) -> Result<Instruction> {
        let performed = instructions
            .last()
            .and_then(last_result)
            .ok_or_else(|| invalid("a reflexive trigger needs one base action"))?;
        instructions.push(self.reflexive_on(performed, condition, actions)?);
        Ok(Instruction::Group(Program { instructions }))
    }

    /// The reflexive trigger of an already performed base action.
    fn reflexive_on(
        &mut self,
        performed: ResultId,
        condition: Option<Condition>,
        actions: &source::Actions,
    ) -> Result<Instruction> {
        let mut child = self.clone();
        child.open_searches.clear();
        child.modes = None;
        let known = child.targets.len();
        let body = child.actions(actions)?;
        self.result = child.result;
        Ok(self.effect(Operation::ReflexiveTrigger {
            performed,
            condition,
            announcement: Announcement {
                targets: child.targets.split_off(known),
                modes: child.modes,
            },
            body,
        }))
    }
}
