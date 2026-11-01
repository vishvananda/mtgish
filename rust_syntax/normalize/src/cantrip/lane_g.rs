//! Keywords the engine has by another road, and the final form of those it does
//! not: each planned entry here is the call the card makes once Cantrip has it.
use super::*;

const M: &str = "cantrip_mechanics";
/// The change an object keeps through every zone for the rest of the game.
const PERPETUAL: &str = "cantrip_mechanics::perpetual::Perpetual";

/// "Commander" as a test: Cantrip plays no game that has one.
pub(super) fn commander(id: &str) -> String {
    planned::planned_value(
        "engine: no commander",
        "false",
        &format!("q.is_commander({id})"),
    )
}

/// An `EffectKind` Cantrip does not have yet.
pub(super) fn planned_kind(feature: &str, code: String) -> String {
    planned::planned_value(feature, "EffectKind::Pump(0, 0)", &code)
}

/// How long mana lasts, where that is all its `ManaUse` says, and what else it says.
pub(super) fn mana_lasting(value: Option<&ManaUse>) -> (Option<&'static str>, Option<&ManaUse>) {
    match value {
        Some(ManaUse::Persists(Duration::EndOfTurn)) => (Some("until_end_of_turn"), None),
        Some(ManaUse::Persists(Duration::EndOfCombat)) => (Some("until_end_of_combat"), None),
        other => (None, other),
    }
}

/// The engine's words for the spells a filter names, where it has them.
fn spell_words(filter: &ObjectFilter, chose_as_entered: bool) -> Option<String> {
    use crate::mtg_types::CardType as T;
    fn subtype(f: &ObjectFilter) -> Option<String> {
        match f {
            ObjectFilter::Subtype(s) => {
                let word = name(s);
                crate::cantrip_vocab::SUBTYPES
                    .contains(&word.as_str())
                    .then(|| format!("Subtype::{word}"))
            }
            _ => None,
        }
    }
    fn card_types(f: &ObjectFilter) -> Option<Vec<String>> {
        match f {
            ObjectFilter::CardType(t) => Some(vec![format!("CardType::{}", card_type(t).ok()?)]),
            ObjectFilter::Either(v) => {
                let mut all = Vec::new();
                for f in v {
                    all.extend(card_types(f)?);
                }
                Some(all)
            }
            _ => None,
        }
    }
    if let Some(types) = card_types(filter) {
        return Some(format!(
            "ManaRestriction::Spells(TypeSet::of(&[{}]))",
            types.join(", ")
        ));
    }
    if let ObjectFilter::Either(v) = filter {
        let subtypes = v.iter().map(subtype).collect::<Option<Vec<_>>>()?;
        return Some(format!(
            "ManaRestriction::Subtypes(SubtypeSpells::of_any(&[{}]))",
            subtypes.join(", ")
        ));
    }
    if let ObjectFilter::All(v) = filter {
        let creature = |f: &ObjectFilter| matches!(f, ObjectFilter::CardType(T::Creature));
        if let ([a, b], true) = (v.as_slice(), v.iter().any(creature)) {
            let other = if creature(a) { b } else { a };
            if matches!(other, ObjectFilter::Chosen(ValueKind::CreatureType)) {
                return chose_as_entered
                    .then(|| "ManaRestriction::Subtypes(SubtypeSpells::CHOSEN_TYPE_CREATURE)".into());
            }
            let word = subtype(other)?;
            return Some(format!(
                "ManaRestriction::Subtypes(SubtypeSpells {{ creature: true, ..SubtypeSpells::of({word}) }})"
            ));
        }
    }
    if matches!(filter, ObjectFilter::Chosen(ValueKind::CreatureType)) && chose_as_entered {
        return Some(
            "ManaRestriction::Subtypes(SubtypeSpells { creature: false, ..SubtypeSpells::CHOSEN_TYPE_CREATURE })"
                .into(),
        );
    }
    None
}

/// The printed price of a keyword: its mana where that is all it is.
fn priced(c: &mut Compiler, cost: Option<&Cost>, mechanic: Mechanic, label: &str) -> Result<String> {
    match cost.map(|c| c.parts.as_slice()) {
        Some([CostPart::Mana(symbols)]) => mana(symbols),
        _ => c.mechanic_cost(cost, mechanic, label),
    }
}

impl Compiler {
    /// An event Cantrip does not show to abilities. `in_history` says its record
    /// is already there, so a question of what happened can ask it now.
    pub(super) fn plan_event(
        &self,
        matched: &mut triggers::Matched,
        feature: &str,
        in_history: bool,
    ) -> Result<()> {
        if self.history.is_some() {
            return match in_history {
                true => Ok(()),
                false => Err(gap(feature)),
            };
        }
        matched.planned.push(feature.into());
        Ok(())
    }
    /// The trigger of an event that can't fire yet: its matcher is written out as
    /// it will be, and the file compiles with one that never matches.
    pub(super) fn planned_trigger(
        &mut self,
        features: &[String],
        label: &str,
        matcher: &str,
        mask: &str,
    ) -> String {
        let feature = features
            .iter()
            .find(|f| !planned::landed(f))
            .expect("a requirement still open");
        let id = planned::requirement(feature);
        writeln!(self.holes, "planned_item!({id:?} => matcher: {matcher});").unwrap();
        writeln!(self.functions, "#[allow(unused_variables)] fn {label}(q: &dyn Query, asker: &Asker, event: &Event) -> Option<Fired> {{ None }}").unwrap();
        let mask = planned::planned_value(feature, "EventMask(0)", &format!("EventMask({mask})"));
        format!("when_asked({mask}, {label})")
    }
    /// A filter asked of a spell or source where mana is being spent.
    fn spending_test(&self, filter: &ObjectFilter, id: &str) -> Result<String> {
        let mut asked = self.clone();
        (asked.reader, asked.event_at) = (Reader::Query, EventAt::Nowhere);
        asked.filter(filter, &Candidate::of(id))
    }
    /// A `ManaUse` as a `ManaRestriction`: the engine's own words, and otherwise
    /// the test its mana will carry.
    pub(super) fn mana_use(&self, value: &ManaUse) -> Result<String> {
        const FEATURE: &str = "engine: mana spent by what it may pay for";
        let planned =
            |code: String| planned::planned_value(FEATURE, "ManaRestriction::ABILITIES", &code);
        if let Ok(code) = mana_restriction(value) {
            return Ok(code);
        }
        let many = |c: &Self, uses: &[ManaUse]| -> Result<String> {
            let codes = uses
                .iter()
                .map(|u| c.mana_use(u))
                .collect::<Result<Vec<_>>>()?;
            Ok(codes.join(", "))
        };
        Ok(match value {
            ManaUse::Either(uses) => format!("ManaRestriction::AnyOf(&[{}])", many(self, uses)?),
            ManaUse::All(uses) => {
                planned(format!("ManaRestriction::AllOf(&[{}])", many(self, uses)?))
            }
            ManaUse::Not(inner) => {
                planned(format!("ManaRestriction::Not(&{})", self.mana_use(inner)?))
            }
            ManaUse::CastSpell(filter) => match spell_words(
                filter,
                self.enters_choosing.contains(&ValueKind::CreatureType),
            ) {
                Some(code) => code,
                None => planned(format!(
                    "ManaRestriction::SpellsMatching(|q, asker, spell| {})",
                    self.spending_test(filter, "spell")?
                )),
            },
            ManaUse::ActivateAbility {
                sources,
                permanents_only,
            } => planned(format!(
                "ManaRestriction::AbilitiesMatching(|q, asker, source| {}, AbilitiesOf::{})",
                self.spending_test(sources, "source")?,
                if *permanents_only {
                    "Permanents"
                } else {
                    "Sources"
                }
            )),
            ManaUse::Purpose(purpose) => planned(format!(
                "ManaRestriction::Purpose(ManaPurpose::{})",
                form(purpose)
            )),
            ManaUse::Persists(duration) => planned(format!(
                "ManaRestriction::Lasts(Duration::{})",
                form(duration)
            )),
            ManaUse::SpentOn { spells, .. }
            | ManaUse::SpentGives { spells, .. }
            | ManaUse::WhenSpent { spells, .. } => planned(format!(
                "ManaRestriction::Rider({}, |q, asker, spell| {})",
                quoted(&ron::to_string(value).unwrap_or_default()),
                self.spending_test(spells, "spell")?
            )),
            ManaUse::Primitive { name, .. } => {
                return Err(gap(format!("primitive mana use {name}")))
            }
        })
    }
    /// A token definition a planned keyword will list in `EXTRA`.
    fn plan_extra(&mut self, feature: &str, path: String) {
        if planned::landed(feature) {
            self.extras.push(path);
            return;
        }
        let id = planned::requirement(feature);
        writeln!(self.holes, "planned_item!({id:?} => extras: {path});").unwrap();
    }
    /// Changes an object keeps for the rest of the game (Alchemy's "perpetually").
    pub(super) fn perpetual_effect(
        &mut self,
        objects: &ObjectSet,
        changes: &[Change],
        out: &mut String,
    ) -> Result<()> {
        const FEATURE: &str = "engine: no perpetual effects";
        let objects = match self.objects(objects) {
            Ok(code) => self.fetch(code),
            // The objects come from an instruction with a hole of its own.
            Err(failed) if failed.feature == FOLLOWS_GAP => return Err(gap(FEATURE)),
            Err(failed) => return Err(failed),
        };
        let id = self.fresh();
        let mut code = format!("let objects_{id}: Vec<ObjectId> = {objects};\n");
        let mut kinds = Vec::new();
        for change in changes {
            for kind in self.perpetual_kinds(change)? {
                let local = format!("kind_{id}_{}", kinds.len());
                writeln!(code, "let {local} = {};", self.fetch(kind)).unwrap();
                kinds.push(local);
            }
        }
        let applies: String = kinds
            .iter()
            .map(|k| format!("{M}::perpetual::apply(g, asker.source, object, {k}); "))
            .collect();
        write!(code, "for &object in &objects_{id} {{ {applies}}}").unwrap();
        self.plan_code(FEATURE, &code, out);
        Ok(())
    }
    fn perpetual_kinds(&mut self, change: &Change) -> Result<Vec<String>> {
        match change {
            Change::GrantAbility(ability) => Ok(vec![self.perpetual_grant(ability)?]),
            Change::Incorporate(symbols) => Ok(vec![format!(
                "{PERPETUAL}::Incorporate({})",
                mana(symbols)?
            )]),
            other => Ok(self
                .effect_kinds(other)?
                .into_iter()
                .map(|kind| format!("{PERPETUAL}::Effect({kind})"))
                .collect()),
        }
    }
    /// An ability an object keeps: one of `delayed` where it is one ability, and
    /// otherwise all that its rules add to a card, as a definition of their own.
    fn perpetual_grant(&mut self, ability: &Ability) -> Result<String> {
        if matches!(
            ability,
            Ability::Casting(CastingRule::SpendAsThoughAnyType)
        ) {
            return Ok(format!("{PERPETUAL}::SpendAsThoughAnyType"));
        }
        let id = self.fresh();
        let grown = self.grown_parts(ability, &format!("perpetual_{id}"))?;
        if let [def] = grown.abilities.as_slice() {
            if grown.only_abilities() {
                self.delayed.push(def.clone());
                let index = self.delayed.len() - 1;
                return Ok(format!(
                    "{PERPETUAL}::Effect(EffectKind::AddAbility(AbilityRef::delayed({}, {index})))",
                    read::SOURCE_CARD
                ));
            }
        }
        let enters = self.enters(&grown.enters);
        let name = format!("PERPETUAL_{id}");
        writeln!(self.functions, "static {name}: CardDef = {};", grown.card(&enters)).unwrap();
        Ok(format!("{PERPETUAL}::Rules(&{name})"))
    }
    /// A keyword that isn't one of the many `mechanic` lists by the same shape.
    pub(super) fn mechanic_beyond(
        &mut self,
        mechanic: Mechanic,
        cost: Option<&Cost>,
        amount: Option<&Expr>,
        filter: Option<&ObjectFilter>,
        label: &str,
        parts: &mut Parts,
    ) -> Result<()> {
        use Mechanic as K;
        let feature = format!("engine: no {mechanic:?}");
        let fixed = || -> Result<u8> {
            amount
                .and_then(costs::constant_u8)
                .ok_or_else(|| gap(feature.clone()))
        };
        // What Cantrip has: the card lists it as the mechanic says.
        match mechanic {
            K::StartYourEngines => {
                parts.field("start_your_engines", "true".into());
                return Ok(());
            }
            K::Storied => {
                parts.field("storied", "true".into());
                return Ok(());
            }
            K::Aftermath => {
                parts.field("aftermath", "true".into());
                return Ok(());
            }
            K::Fuse => {
                parts.field("fuse", "true".into());
                return Ok(());
            }
            K::Daybound | K::Nightbound => {
                parts.keywords.push(format!("Keyword::{mechanic:?}"));
                return Ok(());
            }
            K::Station => {
                parts
                    .abilities
                    .push(format!("{M}::station::ability(\"Station\")"));
                return Ok(());
            }
            K::Hideaway => {
                let n = amount
                    .and_then(costs::constant_u8)
                    .ok_or_else(|| gap("Hideaway without a fixed number"))?;
                let function = format!("{label}_hideaway");
                writeln!(self.functions, "fn {function}(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {{ {M}::hideaway(g, item, {n}) }}").unwrap();
                parts.abilities.push(format!(
                    "triggered({M}::triggers::THIS_ENTERS, {function}, {})",
                    quoted(&format!("Hideaway {n}"))
                ));
                return Ok(());
            }
            K::BeamMeUp => {
                let Some([CostPart::Mana(symbols)]) = cost.map(|c| c.parts.as_slice()) else {
                    return Err(gap("BeamMeUp cost other than mana"));
                };
                let way = format!("{M}::beam_me_up::way({}, \"Beam me up\")", mana(symbols)?);
                parts.table("ways", way);
                return Ok(());
            }
            _ => {}
        }
        // The rest is the keyword's own module, once Cantrip has it.
        let word = name(&mechanic);
        let module = |word: &str| {
            let mut snake = String::new();
            for (i, c) in word.chars().enumerate() {
                if c.is_ascii_uppercase() && i > 0 {
                    snake.push('_');
                }
                snake.push(c.to_ascii_lowercase());
            }
            snake
        };
        let path = module(&word);
        let trigger = |n: u8| format!("{M}::{path}::trigger::<{n}>({word:?})");
        let plain = format!("{M}::{path}::TRIGGER");
        let mut entries: Vec<(&'static str, String)> = Vec::new();
        match mechanic {
            K::Bushido
            | K::Soulshift
            | K::Rampage
            | K::Afflict
            | K::Renown
            | K::Poisonous
            | K::Frenzy
            | K::Fabricate
            | K::Afterlife => entries.push(("abilities", trigger(fixed()?))),
            K::Exploit
            | K::DoubleTeam
            | K::Soulbond
            | K::Conspire
            | K::Haunt
            | K::Gravestorm
            | K::Demonstrate => entries.push(("abilities", plain)),
            K::Absorb => entries.push((
                "preventions",
                format!("{M}::absorb::prevention::<{}>()", fixed()?),
            )),
            K::Devour | K::Tribute | K::Amplify | K::Bloodthirst | K::VanishingEnters => {
                let (path, n) = (path.trim_end_matches("_enters"), fixed()?);
                entries.push(("enters", format!("{M}::{path}::enters::<{n}>")));
            }
            K::BloodthirstX => {
                entries.push(("enters", format!("{M}::bloodthirst::enters_x")));
            }
            K::Vanishing => {
                entries.push(("abilities", format!("{M}::vanishing::upkeep(\"Vanishing\")")));
                entries.push(("abilities", format!("{M}::vanishing::LAST_COUNTER")));
            }
            K::Graft => {
                entries.push(("enters", format!("{M}::graft::enters::<{}>", fixed()?)));
                entries.push(("abilities", format!("{M}::graft::TRIGGER")));
            }
            K::Blitz | K::Prowl | K::Disturb | K::Mutate | K::MoreThanMeetsTheEye => {
                let price = priced(self, cost, mechanic, label)?;
                entries.push(("ways", format!("{M}::{path}::way({price}, {word:?})")));
            }
            K::Encore | K::Specialize | K::Recover | K::Fortify | K::SpecializeFromGraveyard => {
                let price = priced(self, cost, mechanic, label)?;
                entries.push(("abilities", format!("{M}::{path}::ability({price}, {word:?})")));
            }
            K::Reinforce => {
                let price = priced(self, cost, mechanic, label)?;
                let n = fixed()?;
                entries.push((
                    "abilities",
                    format!("{M}::reinforce::ability::<{n}>({price}, {word:?})"),
                ));
            }
            K::Champion => {
                let filter = filter.ok_or_else(|| gap(feature.clone()))?;
                let test = self.cost_filter(filter, &format!("{label}_champion"))?;
                entries.push(("abilities", format!("{M}::champion::enters({test})")));
                entries.push(("abilities", format!("{M}::champion::LEAVES")));
            }
            K::BandsWithOthers => {
                let filter = filter.ok_or_else(|| gap(feature.clone()))?;
                let test = self.cost_filter(filter, &format!("{label}_bands"))?;
                entries.push(("statics", format!("{M}::banding::with_others({test})")));
            }
            K::UmbraArmor => {
                entries.push(("replacements", format!("{M}::umbra_armor::REPLACEMENT")));
            }
            K::LivingMetal => {
                entries.push(("statics", format!("{M}::living_metal::ON_YOUR_TURN")));
            }
            K::StartingIntensity => {
                entries.push(("enters", format!("{M}::intensity::starting::<{}>", fixed()?)));
            }
            K::JumpStart => {
                entries.push(("ways", format!("{M}::jump_start::way(\"Jump-start\")")));
            }
            K::ReadAhead => {
                entries.push(("enters", format!("{M}::read_ahead::enters")));
            }
            K::Undaunted => {
                entries.push(("own_cost_changes", format!("{M}::undaunted::cost")));
            }
            K::Compleated => {
                entries.push(("enters", format!("{M}::compleated::enters")));
            }
            K::HiddenAgenda | K::DoubleAgenda => {
                entries.push(("opening_hand", format!("{M}::{path}::OPENING_HAND")));
            }
            other => return Err(gap(format!("engine: no {other:?}"))),
        }
        for (list, code) in entries {
            self.plan_entry(&feature, list, &code, parts);
        }
        match mechanic {
            K::Afterlife => self.plan_extra(&feature, format!("{M}::afterlife::SPIRIT")),
            K::Fabricate => self.plan_extra(&feature, format!("{M}::fabricate::SERVO")),
            _ => {}
        }
        Ok(())
    }
}
