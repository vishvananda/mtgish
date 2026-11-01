//! Keyword abilities the rules define: each is what `cantrip_mechanics` says a
//! card with it lists. One the engine has no form of stays a named gap.
use super::*;

const M: &str = "cantrip_mechanics";

/// The ability a mechanic needs at index 0 of `delayed`, where the engine looks.
pub(super) fn first_delayed(mechanic: Mechanic) -> Option<String> {
    let path = match mechanic {
        Mechanic::Unearth => "unearth::EXILE",
        Mechanic::Dash => "dash::RETURN",
        Mechanic::Warp => "warp::EXILE",
        Mechanic::Myriad => "myriad::EXILE",
        Mechanic::Mobilize => "mobilize::SACRIFICE",
        _ => return None,
    };
    Some(format!("{M}::{path}"))
}

/// A mechanic that asks `cast_way` for the first way, so it must be the only one.
fn only_way(mechanic: Mechanic) -> bool {
    matches!(mechanic, Mechanic::Evoke | Mechanic::Dash | Mechanic::Warp)
}

/// Escape's cost: mana, and exiling this many other cards from your graveyard.
fn escape_cost(cost: Option<&Cost>) -> Result<(String, i64)> {
    let unlike = || gap("Escape cost other than exiling other graveyard cards");
    let [CostPart::Mana(symbols), CostPart::Move {
        selections,
        destination: Destination::Exile,
        ..
    }] = cost.map_or(&[][..], |c| &c.parts)
    else {
        return Err(unlike());
    };
    let [selection] = selections.as_slice() else {
        return Err(unlike());
    };
    let Some(Count::Exactly(Expr::Constant(exiled))) = &selection.choose else {
        return Err(unlike());
    };
    let shape = format!("{:?}", selection.objects);
    let others = shape.contains("Graveyard")
        && shape.contains("Not(SameObject(Source))")
        && shape.contains("AbilityController");
    if !others || !(0..=255).contains(exiled) {
        return Err(unlike());
    }
    Ok((mana(symbols)?, *exiled))
}

/// The cost as mana alone, for the engine's forms that take no other cost.
fn mana_alone(cost: Option<&Cost>, mechanic: Mechanic) -> Result<String> {
    match cost.map(|c| c.parts.as_slice()) {
        Some([CostPart::Mana(symbols)]) => mana(symbols),
        _ => Err(gap(format!("{mechanic:?} cost other than mana"))),
    }
}

/// The printed symbols of a mana-only cost, for a form that parses them.
fn symbols_text(cost: Option<&Cost>, mechanic: Mechanic) -> Result<String> {
    let letter =
        |c: &ManaColor| -> Result<&str> { Ok(["W", "U", "B", "R", "G", "C"][color_index(c)?]) };
    let other = || gap(format!("{mechanic:?} cost other than mana"));
    let Some([CostPart::Mana(symbols)]) = cost.map(|c| c.parts.as_slice()) else {
        return Err(other());
    };
    let mut text = String::new();
    for symbol in symbols {
        match symbol {
            ManaSymbol::Generic(n) => write!(text, "{{{n}}}").unwrap(),
            ManaSymbol::Color(c) => write!(text, "{{{}}}", letter(c)?).unwrap(),
            ManaSymbol::Hybrid(a, b) => write!(text, "{{{}/{}}}", letter(a)?, letter(b)?).unwrap(),
            _ => return Err(other()),
        }
    }
    Ok(text)
}

/// The `N` of a keyword, written into a const generic.
fn number(amount: Option<&Expr>, mechanic: Mechanic) -> Result<u8> {
    amount
        .and_then(costs::constant_u8)
        .ok_or_else(|| gap(format!("{mechanic:?} without a fixed number")))
}

impl Compiler {
    pub(super) fn mechanic(
        &mut self,
        mechanic: Mechanic,
        cost: Option<&Cost>,
        amount: Option<&Expr>,
        filter: Option<&ObjectFilter>,
        label: &str,
        parts: &mut Parts,
    ) -> Result<()> {
        use Mechanic as K;
        let ability = |parts: &mut Parts, def: String| parts.abilities.push(def);
        let constant = |parts: &mut Parts, path: &str| parts.abilities.push(format!("{M}::{path}"));
        // "Way" mechanics are all one shape, but for the module that holds them.
        let way_module = match mechanic {
            K::Spectacle => Some("spectacle"),
            K::Surge => Some("surge"),
            K::Freerunning => Some("freerunning"),
            K::Harmonize => Some("harmonize"),
            K::Sneak => Some("sneak"),
            K::WebSlinging => Some("web_slinging"),
            K::Mayhem => Some("mayhem"),
            K::Dash => Some("dash"),
            K::Warp => Some("warp"),
            _ => None,
        };
        if let Some(module) = way_module {
            let cost = mana_alone(cost, mechanic)?;
            let text = quoted(&self.said_word(format!("{mechanic:?}")));
            parts.table("ways", format!("{M}::{module}::way({cost}, {text})"));
        }
        let face_down = match mechanic {
            K::Morph => Some("morph"),
            K::Megamorph => Some("megamorph"),
            K::Disguise => Some("disguise"),
            _ => None,
        };
        if let Some(module) = face_down {
            let cost = self.mechanic_cost(cost, mechanic, label)?;
            let text = quoted(&self.said_word(format!("{mechanic:?}")));
            parts.table("ways", format!("{M}::{module}::way({cost}, {text})"));
        }
        if only_way(mechanic) {
            self.only_way = true;
        }
        match mechanic {
            K::Spectacle
            | K::Surge
            | K::Freerunning
            | K::Harmonize
            | K::Sneak
            | K::WebSlinging
            | K::Mayhem
            | K::Morph
            | K::Megamorph
            | K::Disguise => {}
            K::Dash => {
                parts.field("end_step_riders", format!("{M}::dash::RIDERS"));
                parts.statics.push(format!("{M}::dash::HASTE"));
            }
            K::Warp => parts.field("end_step_riders", format!("{M}::warp::RIDERS")),
            K::Evoke => {
                let cost = self.mechanic_cost(cost, mechanic, label)?;
                let text = quoted(&self.said_word("Evoke".into()));
                parts.table("ways", format!("{M}::evoke::way({cost}, {text})"));
                ability(parts, format!("{M}::evoke::sacrifice_trigger(\"Evoke\")"));
            }
            K::Emerge => {
                let [CostPart::Mana(symbols)] = cost.map_or(&[][..], |c| &c.parts) else {
                    return Err(gap("Emerge cost other than mana"));
                };
                let creature = "cantrip_mechanics::targets::filters::creature";
                parts.table("ways", format!("way(Cost {{ mana: {}, permanents: Some(PermanentCost {{ count: 1, filter: {creature}, action: PermanentAction::Sacrifice, tapped: false }}), emerge: true, ..Cost::NONE }}, ZoneKind::Hand, \"Emerge\")", mana(symbols)?));
            }
            K::Banding | K::SplitSecond | K::Decayed | K::Retrace | K::Phasing => {
                parts.keywords.push(format!("Keyword::{mechanic:?}"));
            }
            K::Convoke => {
                parts.pay_with.insert("convoke");
            }
            K::Delve => {
                parts.pay_with.insert("delve");
            }
            K::Improvise => {
                parts.pay_with.insert("improvise");
            }
            K::Assist => {
                parts.pay_with.insert("assist");
            }
            K::Exalted => constant(parts, "triggers::EXALTED"),
            K::Cascade => constant(parts, "triggers::CASCADE"),
            K::Evolve => constant(parts, "evolve::EVOLVE"),
            K::Extort => constant(parts, "extort::EXTORT"),
            K::Mentor => constant(parts, "mentor::MENTOR"),
            K::BattleCry => constant(parts, "battle_cry::TRIGGER"),
            K::Melee => constant(parts, "melee::TRIGGER"),
            K::Training => constant(parts, "training::TRIGGER"),
            K::Ingest => constant(parts, "ingest::TRIGGER"),
            K::Provoke => constant(parts, "provoke::TRIGGER"),
            K::Dethrone => constant(parts, "dethrone::TRIGGER"),
            K::Increment => constant(parts, "increment::TRIGGER"),
            K::Enlist => constant(parts, "enlist::ENLIST"),
            K::Myriad => constant(parts, "myriad::TRIGGER"),
            K::Storm => ability(parts, format!("{M}::storm::trigger(\"Storm\")")),
            K::Persist => ability(parts, format!("{M}::persist::trigger(\"Persist\")")),
            K::Undying => ability(parts, format!("{M}::undying::trigger(\"Undying\")")),
            K::ForMirrodin => {
                constant(parts, "for_mirrodin::TRIGGER");
                self.extras.push(format!("{M}::for_mirrodin::REBEL"));
            }
            K::JobSelect => {
                constant(parts, "job_select::JOB_SELECT");
                self.extras.push(format!("{M}::job_select::HERO"));
            }
            K::LivingWeapon => {
                ability(parts, format!("triggered({M}::triggers::THIS_ENTERS, {M}::attach::living_weapon, \"Living weapon\")"));
                self.extras.push(format!("{M}::tokens::GERM"));
            }
            K::Unearth => ability(
                parts,
                format!(
                    "{M}::unearth::ability({}, \"Unearth\")",
                    mana_alone(cost, mechanic)?
                ),
            ),
            K::Embalm | K::Eternalize | K::Scavenge | K::Plot | K::Ninjutsu => {
                let (module, function) = match mechanic {
                    K::Embalm => ("embalm", "ability"),
                    K::Eternalize => ("eternalize", "ability"),
                    K::Scavenge => ("scavenge", "ability"),
                    K::Plot => ("plot", "plot"),
                    _ => ("ninjutsu", "ninjutsu"),
                };
                let text = quoted(&self.said_word(format!("{mechanic:?}")));
                let cost = mana_alone(cost, mechanic)?;
                ability(parts, format!("{M}::{module}::{function}({cost}, {text})"));
            }
            K::Transmute => ability(
                parts,
                format!(
                    "{M}::transmute::transmute({}, \"Transmute\")",
                    mana_alone(cost, mechanic)?
                ),
            ),
            K::Transfigure => ability(
                parts,
                format!(
                    "{M}::transfigure::transfigure({}, \"Transfigure\")",
                    mana_alone(cost, mechanic)?
                ),
            ),
            K::Outlast => {
                let symbols = symbols_text(cost, mechanic)?;
                ability(
                    parts,
                    format!("{M}::outlast::ability({symbols:?}, \"Outlast\")"),
                );
            }
            K::Madness => parts.field("madness", format!("Some({})", mana_alone(cost, mechanic)?)),
            K::Miracle => parts.field("miracle", format!("Some({})", mana_alone(cost, mechanic)?)),
            K::Foretell => {
                parts.field("foretell", format!("Some({})", mana_alone(cost, mechanic)?));
                constant(parts, "foretell::ABILITY");
            }
            K::Suspend => {
                let mana = mana_alone(cost, mechanic)?;
                let count = number(amount, mechanic)?;
                parts.field(
                    "suspend",
                    format!("Some(SuspendDef {{ count: {count}, cost: {mana} }})"),
                );
                ability(parts, format!("{M}::suspend::ability({mana}, \"Suspend\")"));
            }
            K::Echo => {
                let cost = self.mechanic_cost(cost, mechanic, label)?;
                ability(parts, format!("{M}::echo!({cost}, \"Echo\")"));
            }
            K::CumulativeUpkeep => {
                let payment = match cost.map_or(&[][..], |c| &c.parts) {
                    [CostPart::Mana(symbols)] => format!("Mana({})", mana(symbols)?),
                    [CostPart::Life(n)] => format!(
                        "Life({})",
                        costs::constant_u8(n).ok_or_else(|| gap("cumulative life not fixed"))?
                    ),
                    _ => return Err(gap("cumulative upkeep paid other than in mana or life")),
                };
                ability(parts, format!("{M}::cumulative_upkeep!({M}::cumulative_upkeep::Payment::{payment}, \"Cumulative upkeep\")"));
            }
            K::Kicker | K::Multikicker | K::Replicate | K::Buyback | K::Squad | K::Offspring => {
                let mana = mana_alone(cost, mechanic)?;
                let field = match mechanic {
                    K::Kicker | K::Buyback => format!("kicker({mana})"),
                    K::Offspring => format!("Some({M}::offspring::kicker({mana}))"),
                    _ => format!("multikicker({mana})"),
                };
                if parts.fields.contains_key("kicker") {
                    return Err(gap("two costs a spell kicks with"));
                }
                parts.field("kicker", field);
                match mechanic {
                    K::Replicate => {
                        ability(parts, format!("{M}::replicate::trigger(\"Replicate\")"))
                    }
                    K::Offspring => {
                        ability(parts, format!("{M}::offspring::trigger(\"Offspring\")"))
                    }
                    K::Squad => constant(parts, "triggers::SQUAD"),
                    K::Buyback => parts
                        .replacements
                        .push(format!("{M}::replacements::BUYBACK")),
                    _ => {}
                }
            }
            K::Bargain => {
                parts.field(
                    "optional_cost",
                    format!("Some({M}::bargain::OPTIONAL_COST)"),
                );
                parts.field("labels", format!("{M}::bargain::LABELS"));
            }
            K::Casualty => {
                let n = number(amount, mechanic)?;
                parts.field(
                    "optional_cost",
                    format!("Some({M}::casualty::optional::<{n}>())"),
                );
                parts.field(
                    "labels",
                    format!("&[\"Cast without casualty\", \"Casualty {n}\"]"),
                );
                ability(parts, format!("{M}::casualty::trigger(\"Casualty\")"));
            }
            K::Rebound => parts.field("rebound", "true".into()),
            K::Ascend => parts.field("ascend", "true".into()),
            K::Toxic => {
                let n = number(amount, mechanic)?;
                if parts.fields.contains_key("toxic") {
                    return Err(gap("two toxic abilities"));
                }
                parts.field("toxic", n.to_string());
            }
            K::Annihilator => {
                let n = number(amount, mechanic)?;
                ability(
                    parts,
                    format!("{M}::triggers::annihilator::<{n}>(\"Annihilator\")"),
                );
            }
            K::Firebending => {
                let n = number(amount, mechanic)?;
                ability(
                    parts,
                    format!("{M}::firebending::trigger::<{n}>(\"Firebending\")"),
                );
            }
            K::Mobilize => {
                let n = number(amount, mechanic)?;
                ability(
                    parts,
                    format!("{M}::mobilize::trigger::<{n}>(\"Mobilize\")"),
                );
                self.extras.push(format!("{M}::mobilize::WARRIOR"));
            }
            K::Ripple => {
                let n = number(amount, mechanic)?;
                ability(parts, format!("{M}::ripple::trigger::<{n}>(\"Ripple\")"));
            }
            K::Dredge => {
                let n = number(amount, mechanic)?;
                parts
                    .replacements
                    .push(format!("{M}::dredge::dredge::<{n}>()"));
            }
            K::Saddle => {
                let n = number(amount, mechanic)?;
                ability(parts, format!("{M}::saddle::ability({n}, \"Saddle\")"));
            }
            K::Fading => {
                let n = number(amount, mechanic)?;
                parts.enters.push(format!("{M}::fading::enters::<{n}>"));
                ability(parts, format!("{M}::fading::upkeep(\"Fading\")"));
            }
            K::Impending => {
                let n = number(amount, mechanic)?;
                let cost = mana_alone(cost, mechanic)?;
                let text = quoted(&self.said_word("Impending".into()));
                parts.table("ways", format!("{M}::impending::way({cost}, {text})"));
                parts.enters.push(format!("{M}::impending::enters::<{n}>"));
                parts
                    .statics
                    .push(format!("{M}::impending::NOT_A_CREATURE"));
                constant(parts, "impending::REMOVE_A_TIME_COUNTER");
            }
            K::Modular => {
                let n = number(amount, mechanic)?;
                let function = format!("{label}_modular");
                writeln!(self.functions, "fn {function}(_: &mut dyn Game, entering: &mut Entering) -> Result<(), Suspend> {{ let kind = CounterKind::PlusOne as usize; entering.counters[kind] = entering.counters[kind].saturating_add({n}); Ok(()) }}").unwrap();
                parts.enters.push(function);
                constant(parts, "modular::TRIGGER");
            }
            K::Sunburst => parts.enters.push(format!("{M}::sunburst::enters")),
            K::Unleash => {
                parts.enters.push(format!("{M}::unleash::enters"));
                parts.statics.push(format!("{M}::unleash::CANT_BLOCK"));
            }
            K::Ravenous => {
                parts.enters.push(format!("{M}::ravenous::enters"));
                ability(parts, format!("{M}::ravenous::draw_trigger(\"Ravenous\")"));
            }
            K::Riot => {
                let function = format!("{label}_riot");
                writeln!(self.functions, "fn {function}(g: &mut dyn Game, entering: &mut Entering) -> Result<(), Suspend> {{ {M}::riot::enters(g, entering, &CARD) }}").unwrap();
                parts.enters.push(function);
                parts.field("labels", format!("{M}::riot::LABELS"));
                parts.statics.push(format!("{M}::riot::HASTE_STATIC"));
            }
            K::Reconfigure => {
                let cost = mana_alone(cost, mechanic)?;
                parts.field("reconfigure", "true".into());
                ability(
                    parts,
                    format!("{M}::reconfigure::attach({cost}, \"Reconfigure\")"),
                );
                ability(
                    parts,
                    format!("{M}::reconfigure::unattach({cost}, \"Reconfigure\")"),
                );
                parts
                    .statics
                    .push(format!("{M}::reconfigure::NOT_A_CREATURE_WHILE_ATTACHED"));
            }
            K::SpliceOnto => {
                let arcane =
                    matches!(filter, Some(ObjectFilter::Subtype(s)) if name(s) == "Arcane");
                if !arcane {
                    return Err(gap("engine: splice onto other than Arcane"));
                }
                parts.field("splice", format!("Some({})", mana_alone(cost, mechanic)?));
            }
            K::Affinity => {
                let filter = filter.ok_or_else(|| gap("affinity for nothing"))?;
                let saved = (self.reader, self.event_at);
                (self.reader, self.event_at) = (Reader::Query, EventAt::Nowhere);
                let test = self.filter(filter, &Candidate::of("id"));
                (self.reader, self.event_at) = saved;
                let test = test?;
                let function = format!("{label}_affinity");
                writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, source: ObjectId, player: PlayerId, costed: Costed, targets: &[Target]) -> CostDelta {{ if costed != Costed::Spell(source) {{ return CostDelta::NONE; }} let asker = &Asker::new(source, player); let count = q.permanents_of(player).into_iter().filter(|&id| {test}).count(); CostDelta::less(count.min(255) as u8) }}").unwrap();
                parts.table("own_cost_changes", function);
            }
            K::Bestow => {
                let cost = mana_alone(cost, mechanic)?;
                self.bestow = Some(cost);
            }
            K::Escape => {
                let (mana, exiled) = escape_cost(cost)?;
                let text = quoted(&self.said_word("Escape".into()));
                parts.table("ways", format!("{M}::escape::way({mana}, {exiled}, {text})"));
            }
            other => self.mechanic_beyond(other, cost, amount, filter, label, parts)?,
        }
        Ok(())
    }
    pub(super) fn mechanic_cost(
        &mut self,
        cost: Option<&Cost>,
        mechanic: Mechanic,
        label: &str,
    ) -> Result<String> {
        let cost = cost.ok_or_else(|| gap(format!("{mechanic:?} without a cost")))?;
        self.cost(cost, label)
    }
}
