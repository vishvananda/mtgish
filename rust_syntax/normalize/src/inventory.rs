//! Count every semantic family, rather than presenting operation count alone
//! as the size of the normalized language. Vocabulary values are counted too.
use crate::ir::*;
use serde::Serialize;
use std::collections::BTreeMap;
pub type Forms = BTreeMap<String, BTreeMap<String, usize>>;
pub struct Inventory<'a>(pub &'a mut Forms);
impl Inventory<'_> {
    fn form(&mut self, family: &str, value: &impl Serialize) {
        let value = serde_json::to_value(value).expect("serializable semantic form");
        let variant = value
            .as_str()
            .or_else(|| value.get("kind").and_then(|v| v.as_str()))
            .or_else(|| {
                value
                    .as_object()
                    .and_then(|v| v.keys().next().map(String::as_str))
            })
            .expect("enum form");
        *self
            .0
            .entry(family.into())
            .or_default()
            .entry(variant.into())
            .or_default() += 1;
    }
    pub fn card(&mut self, card: &Card) {
        for ty in &card.characteristics.card_types {
            self.form("CardType", ty);
        }
        for ty in &card.characteristics.supertypes {
            self.form("SuperType", ty);
        }
        for ty in &card.characteristics.subtypes {
            self.form("Subtype", ty);
        }
        if let Some(mana) = &card.characteristics.mana_cost {
            for symbol in mana {
                self.form("ManaSymbol", symbol);
            }
        }
        if let Some(n) = &card.characteristics.loyalty {
            self.expr(n);
        }
        for a in &card.abilities {
            self.ability(a);
        }
    }
    fn primitive(&mut self, name: &str) {
        *self
            .0
            .entry("Primitive".into())
            .or_default()
            .entry(name.into())
            .or_default() += 1;
    }
    fn ability(&mut self, a: &Ability) {
        self.form("Ability", a);
        match a {
            Ability::Named(_) => {}
            Ability::Primitive { name, .. } => self.primitive(name),
            Ability::CounterStages {
                counter,
                level_up,
                stages,
            } => {
                self.form("CounterKind", counter);
                if let Some(cost) = level_up {
                    self.cost(cost);
                }
                for ability in stages.iter().flat_map(|stage| &stage.abilities) {
                    self.ability(ability);
                }
            }
            Ability::Backup { abilities, .. } => {
                abilities.iter().for_each(|a| self.ability(a));
            }
            Ability::Mystery {
                condition,
                abilities,
            } => {
                self.condition(condition);
                abilities.iter().for_each(|a| self.ability(a));
            }
            Ability::DraftRule(_)
            | Ability::Companion(_)
            | Ability::CommanderDeckRule(_)
            | Ability::Craft { .. } => {}
            Ability::Offering(filter) => self.filter(filter),
            Ability::FlashIf(condition) => self.condition(condition),
            Ability::ProtectionExcept { keeps, .. } => self.objects(keeps),
            Ability::DeckRule(rule) => self.form("DeckRule", rule),
            Ability::Mechanic {
                mechanic,
                cost,
                amount,
                filter,
            } => {
                self.form("Mechanic", mechanic);
                if let Some(cost) = cost {
                    self.cost(cost);
                }
                if let Some(amount) = amount {
                    self.expr(amount);
                }
                if let Some(filter) = filter {
                    self.filter(filter);
                }
            }
            Ability::CostBranch {
                cost, paid, unpaid, ..
            } => {
                self.cost(cost);
                self.ability(paid);
                self.ability(unpaid);
            }
            Ability::CostOptions { options, .. } => {
                for (cost, ability) in options {
                    self.cost(cost);
                    self.ability(ability);
                }
            }
            Ability::Saga { chapters } => {
                for chapter in chapters {
                    self.ability(chapter);
                }
            }
            Ability::Gift {
                gift,
                promised,
                unpromised,
            } => {
                for program in gift {
                    self.program(program);
                }
                for outcome in promised.iter().chain(unpromised) {
                    self.ability(outcome);
                }
            }
            Ability::Spree { modes } => {
                for mode in modes {
                    self.cost(&mode.cost);
                    self.ability(&mode.ability);
                }
            }
            Ability::Prototype { .. } => {}
            Ability::Visit { body, prize } => {
                self.ability(body);
                if let Some(prize) = prize {
                    self.ability(prize);
                }
            }
            Ability::DungeonRoom { body, .. } => self.ability(body),
            Ability::TriggerRule {
                cause,
                abilities,
                rule,
            } => {
                self.form("TriggerRule", rule);
                if let Some(event) = cause {
                    self.event(event);
                }
                if let Some(filter) = abilities {
                    self.filter(filter);
                }
            }
            Ability::Class { levels } => {
                for level in levels {
                    self.cost(&level.cost);
                    level.abilities.iter().for_each(|a| self.ability(a));
                }
            }
            Ability::Scoped {
                zones,
                condition,
                ability,
            } => {
                for zone in zones {
                    self.form("EventZone", zone);
                }
                if let Some(c) = condition {
                    self.condition(c);
                }
                self.ability(ability);
            }
            Ability::Casting(rule) => self.casting_rule(rule),
            Ability::Replacement {
                active,
                event,
                body,
            } => {
                self.form("ReplacementActive", active);
                self.replacement_event(event);
                self.program(body);
            }
            Ability::Enchant { hosts } => {
                self.form("EnchantHosts", hosts);
                match hosts {
                    EnchantHosts::Permanents(f) => self.filter(f),
                    EnchantHosts::Players(p) => self.players(p),
                    EnchantHosts::GraveyardCards { filter, owners } => {
                        self.filter(filter);
                        self.players(owners);
                    }
                }
            }
            Ability::DamageCantBePrevented { sources, .. } => {
                if let Some(sources) = sources {
                    self.objects(sources);
                }
            }
            Ability::CantEnter { cards, from } => {
                self.filter(cards);
                if let Some(zone) = from {
                    self.zone_scope(zone);
                }
            }
            Ability::AttackCap { defender, amount } => {
                self.recipients(defender);
                self.expr(amount);
            }
            Ability::AsEvent { object, event, .. } => {
                self.object(object);
                self.form("AsEvent", event);
            }
            Ability::BeforeGame { when, .. } => self.form("BeforeGameMoment", when),
            Ability::Modified { ability, .. } | Ability::KickedHas { ability, .. } => {
                self.ability(ability)
            }
            Ability::Tiered { modes } => {
                for mode in modes {
                    self.cost(&mode.cost);
                    self.ability(&mode.ability);
                }
            }
            Ability::PayToIgnore { cost, .. } => self.cost(cost),
            Ability::CountsAsNamed { .. } => {}
            Ability::Unique(rule) => self.form("UniqueAbility", rule),
            Ability::WhileOnTopOfLibrary(abilities) | Ability::Triggers(abilities) => {
                for ability in abilities {
                    self.ability(ability);
                }
            }
            Ability::StartingHandSize(size) => self.expr(size),
            Ability::VillainousChoiceTwice(players) => self.players(players),
            Ability::ExilesSearchedCards { searching, player } => {
                self.players(searching);
                self.player(player);
            }
            Ability::Keyword(k) => self.keyword(k),
            Ability::Spell { announcement, body } | Ability::OpeningHand { announcement, body } => {
                self.announcement(announcement);
                self.program(body);
            }
            Ability::Activated {
                origin,
                cost,
                limits,
                announcement,
                body,
            } => {
                self.form("ActivationOrigin", origin);
                if let ActivationOrigin::Cycling {
                    search: Some(filter),
                } = origin
                {
                    self.filter(filter);
                }
                self.form(
                    "ActivationKind",
                    &crate::semantics::activation_kind(cost, announcement, body),
                );
                self.cost(cost);
                if let Some(condition) = &limits.condition {
                    self.condition(condition);
                }
                limits.modifiers.iter().for_each(|m| self.change(m));
                self.announcement(announcement);
                self.program(body);
            }
            Ability::Triggered {
                event,
                intervening_if,
                limits: _,
                announcement,
                body,
            } => {
                self.event(event);
                if let Some(c) = intervening_if {
                    self.condition(c);
                }
                self.announcement(announcement);
                self.program(body);
            }
            Ability::Static {
                kind,
                affected,
                changes,
            } => {
                self.form("StaticKind", kind);
                if let StaticKind::CharacteristicDefining {
                    keyword: Some(keyword),
                    ..
                } = kind
                {
                    self.keyword(keyword);
                }
                self.subjects(affected);
                for c in changes {
                    self.change(c);
                }
            }
            Ability::Conditional {
                condition,
                then,
                otherwise,
            } => {
                self.condition(condition);
                for a in then.iter().chain(otherwise) {
                    self.ability(a);
                }
            }
        }
    }
    fn announcement(&mut self, a: &Announcement) {
        for target in &a.targets {
            if let Some(chooser) = &target.chooser {
                self.player(chooser);
            }
            self.form("TargetKind", &target.kind);
            if let TargetKind::Object(domain) = &target.kind {
                self.form("ObjectKind", &domain.kind);
                for zone in &domain.zones {
                    self.form("EventZone", zone);
                }
            }
            self.count(&target.count);
            self.filter(&target.filter);
            for constraint in &target.constraints {
                self.selection_constraint(constraint);
            }
            if let Some(p) = &target.players {
                self.players(p);
            }
        }
        if let Some(m) = &a.modes {
            self.expr(&m.minimum);
            self.expr(&m.maximum);
            if let Some(cost) = &m.surcharge {
                self.cost(cost);
            }
            for mode in &m.modes {
                self.announcement(&mode.announcement);
                self.program(&mode.body);
            }
        }
    }
    fn new_targets(&mut self, targets: &NewTargets) {
        self.form("NewTargets", targets);
        match targets {
            NewTargets::To(objects) | NewTargets::Each(objects) => self.objects(objects),
            NewTargets::ToPlayers(players) | NewTargets::EachPlayer(players) => {
                self.players(players)
            }
            NewTargets::Random(_)
            | NewTargets::Same
            | NewTargets::May
            | NewTargets::Must
            | NewTargets::EachLegal => {}
        }
    }
    fn program(&mut self, p: &Program) {
        for i in &p.instructions {
            self.form("Instruction", i);
            match i {
                Instruction::Effect {
                    result: _,
                    operation,
                } => self.operation(operation),
                Instruction::Group(p) => self.program(p),
                Instruction::If {
                    condition,
                    then,
                    otherwise,
                } => {
                    self.condition(condition);
                    self.program(then);
                    self.program(otherwise);
                }
                Instruction::Optional { decider, body } => {
                    self.player(decider);
                    self.program(body);
                }
                Instruction::Repeatable(body) => self.program(body),
                Instruction::ForEachObject { objects, body, .. } => {
                    self.objects(objects);
                    self.program(body);
                }
                Instruction::Times { amount, body } => {
                    self.expr(amount);
                    self.program(body);
                }
                Instruction::ForEachPlayer {
                    players,
                    starting_with,
                    body,
                    ..
                } => {
                    if let Some(player) = starting_with {
                        self.player(player);
                    }
                    self.players(players);
                    self.program(body);
                }
                Instruction::ChooseOne { decider, options } => {
                    self.player(decider);
                    options.iter().for_each(|option| self.program(option));
                }
            }
        }
    }
    fn operation(&mut self, op: &Operation) {
        self.form("Operation", op);
        match op {
            Operation::Meld { objects, .. } | Operation::Piles { objects, .. } => {
                self.objects(objects)
            }
            Operation::PutStickers {
                objects,
                ticket_cost,
                ..
            } => {
                self.objects(objects);
                ticket_cost.iter().for_each(|(_, n)| self.expr(n));
            }
            Operation::ChoosePile { actor, .. }
            | Operation::Guess { actor, .. }
            | Operation::Seek { actor, .. }
            | Operation::PickFromHand { actor, .. } => self.players(actor),
            Operation::Note(_) => {}
            Operation::EachPlayer { players, action } => {
                self.players(players);
                self.operation(action);
            }
            Operation::EachObject { objects, action } => {
                self.objects(objects);
                self.operation(action);
            }
            Operation::LimitX { value, .. } => self.expr(value),
            Operation::RestartGame { keep, then } => {
                self.objects(keep);
                self.operation(then);
            }
            Operation::EachNumber { from, to, action } => {
                self.expr(from);
                self.expr(to);
                self.operation(action);
            }
            Operation::CounterWith { .. }
            | Operation::ReduceSpeed { .. }
            | Operation::PutCountersCapped { .. } => {}
            Operation::Populate { actor, .. }
            | Operation::ChooseDefender { actor, .. }
            | Operation::MillUntil { actor, .. } => self.players(actor),
            Operation::WithReflexive { action, .. } => self.operation(action),
            Operation::GiveControl { objects, .. } => self.objects(objects),
            Operation::FightEach { fighters, .. } => self.objects(fighters),
            Operation::AnyPlayerMayPay { players, .. }
            | Operation::PlayersPayRepeatedly { players, .. } => self.players(players),
            Operation::DelayedUnlessPaid { .. } => {}
            Operation::Bid { players, .. }
            | Operation::DrawThenDiscard { players, .. }
            | Operation::RedistributeLife(players) => self.players(players),
            Operation::DistributeCounters {
                amount, objects, ..
            }
            | Operation::PutCountersEach {
                amount, objects, ..
            } => {
                self.expr(amount);
                self.objects(objects);
            }
            Operation::PutChosenCounters { objects, .. } => self.objects(objects),
            Operation::Named { .. } => {}
            Operation::NoteObjects { objects } => self.objects(objects),
            Operation::Primitive { name, .. } => self.primitive(name),
            Operation::CreatePerpetualEffect { objects, changes } => {
                self.objects(objects);
                for change in changes {
                    self.change(change);
                }
            }
            Operation::KeywordEach {
                actor,
                action,
                objects,
                until,
            } => {
                self.form("KeywordAction", action);
                self.players(actor);
                self.objects(objects);
                if let Some(until) = until {
                    self.duration(until);
                }
            }
            Operation::MoveTopCard { actor, cards, .. } => {
                self.players(actor);
                self.filter(cards);
            }
            Operation::PayAnyAmount { actor, resource } => {
                self.form("PaidResource", resource);
                self.players(actor);
            }
            Operation::RevealFromHand {
                actor,
                cards,
                count,
                ..
            } => {
                self.players(actor);
                self.filter(cards);
                if let Some(count) = count {
                    self.expr(count);
                }
            }
            Operation::CopyOnStack {
                actor,
                object,
                times,
                targets,
                card,
                ..
            } => {
                self.players(actor);
                self.object(object);
                self.expr(times);
                self.new_targets(targets);
                card.iter().for_each(|o| self.object(o));
            }
            Operation::Retarget {
                actor,
                object,
                targets,
            } => {
                self.players(actor);
                self.object(object);
                self.new_targets(targets);
            }
            Operation::AbandonScheme { actor } | Operation::BeginOnBattlefield { actor, .. } => {
                self.players(actor)
            }
            Operation::Together(operations) => operations.iter().for_each(|o| self.operation(o)),
            Operation::Repeat { times, body } => {
                self.expr(times);
                self.operation(body);
            }
            Operation::TurnFaceDownAs { objects, .. } => self.objects(objects),
            Operation::Exchange { kind, until, .. } => {
                self.form("ExchangeKind", kind);
                if let Some(until) = until {
                    self.duration(until);
                }
            }
            Operation::ChooseValueWith { actor, kind, .. } => {
                self.players(actor);
                self.form("ValueKind", kind);
            }
            Operation::RevealChoices { kind } => self.form("ValueKind", kind),
            Operation::CoinFlips {
                actor, times, each, ..
            } => {
                self.players(actor);
                if let Some(times) = times {
                    self.expr(times);
                }
                if let Some(each) = each {
                    self.objects(each);
                }
            }
            Operation::AttachEach {
                actor,
                attachments,
                count,
                hosts,
                ..
            } => {
                self.players(actor);
                self.objects(attachments);
                if let Some(count) = count {
                    self.count(count);
                }
                self.recipients(hosts);
            }
            Operation::CopyEach {
                actor,
                objects,
                targets,
            }
            | Operation::RetargetEach {
                actor,
                objects,
                targets,
            } => {
                self.players(actor);
                self.filter(objects);
                self.new_targets(targets);
            }
            Operation::TakeSpell {
                actor,
                object,
                targets,
            } => {
                self.players(actor);
                self.object(object);
                self.new_targets(targets);
            }
            Operation::SetX { object, value } => {
                self.object(object);
                self.expr(value);
            }
            Operation::ChangeCombat { change } => self.form("CombatChange", change),
            Operation::PlayersDo { players, .. } => self.players(players),
            Operation::CreateBoon {
                actor,
                event,
                times,
                ..
            } => {
                self.players(actor);
                self.event(event);
                self.expr(times);
            }
            Operation::AddPhases { actor, count, .. } => {
                self.players(actor);
                self.expr(count);
            }
            Operation::ChooseAndPay {
                actor,
                objects,
                cost,
            } => {
                self.players(actor);
                self.objects(objects);
                self.cost(cost);
            }
            Operation::EffectOn { subject, until, .. } => {
                match subject {
                    EffectSubject::Objects(objects) => self.objects(objects),
                    EffectSubject::EachSpell(spells) => self.filter(spells),
                    EffectSubject::NextSpell { spells, caster, .. } => {
                        self.filter(spells);
                        self.player(caster);
                    }
                    EffectSubject::Players(players) => self.players(players),
                }
                if let Some(until) = until {
                    self.duration(until);
                }
            }
            Operation::GameEffect { until, .. } => self.duration(until),
            Operation::AttachChosen {
                actor,
                attachments,
                hosts,
                ..
            } => {
                self.players(actor);
                self.objects(attachments);
                self.objects(hosts);
            }
            Operation::DrawUpTo { players, amount } => {
                self.players(players);
                self.expr(amount);
            }
            Operation::DiscardAllBut { players, keep } => {
                self.players(players);
                self.expr(keep);
            }
            Operation::SacrificeAllBut {
                players,
                objects,
                keep,
            } => {
                self.players(players);
                self.objects(objects);
                self.expr(keep);
            }
            Operation::TowardPlayer { actor, player, .. } => {
                self.players(actor);
                self.player(player);
            }
            Operation::Intensify { cards, owner } => {
                self.filter(cards);
                self.player(owner);
            }
            Operation::FlipCoins { actor, count } => {
                self.players(actor);
                self.expr(count);
            }
            Operation::Wish { actor, filter, .. } => {
                self.players(actor);
                self.filter(filter);
            }
            Operation::DuplicateCounters { from, to, .. } => {
                self.recipients(from);
                self.recipients(to);
            }
            Operation::PutSticker { actor, objects, .. } => {
                self.players(actor);
                self.objects(objects);
            }
            Operation::Manifest { actor, source, .. } => {
                self.players(actor);
                match source {
                    ManifestSource::TopOfLibrary { player, count } => {
                        self.player(player);
                        self.expr(count);
                    }
                    ManifestSource::Cards(cards) => self.objects(cards),
                }
            }
            Operation::Vote { actor, ballot, .. } => {
                self.players(actor);
                match ballot {
                    Ballot::Words(_) => {}
                    Ballot::Players(players) => self.players(players),
                    Ballot::Objects(objects) | Ballot::UpToOne(objects) => self.objects(objects),
                    Ballot::GraveyardCards(cards) => self.filter(cards),
                }
            }
            Operation::Clash {
                actor,
                opponents,
                won,
                lost,
            } => {
                self.players(actor);
                self.players(opponents);
                for program in won.iter().chain(lost) {
                    self.program(program);
                }
            }
            Operation::DraftFromSpellbook { actor, times, .. } => {
                self.players(actor);
                self.expr(times);
            }
            Operation::CreateEmblem { actor, abilities } => {
                self.players(actor);
                for ability in abilities {
                    self.ability(ability);
                }
            }
            Operation::MoveCounters {
                actor,
                from,
                to,
                amount,
                ..
            } => {
                self.form("MovedCounters", amount);
                self.players(actor);
                self.objects(from);
                self.objects(to);
                if let MovedCounters::Exactly(amount) | MovedCounters::AtLeast(amount) = amount {
                    self.expr(amount);
                }
            }
            Operation::EachCounterKind {
                actor,
                among,
                options,
                ..
            } => {
                self.players(actor);
                self.objects(among);
                for option in options {
                    self.form("CounterStep", option);
                    match option {
                        CounterStep::PutThatKind(object) | CounterStep::RemoveThatKind(object) => {
                            self.object(object)
                        }
                        CounterStep::Put(kind, object) => {
                            self.form("CounterKind", kind);
                            if let crate::counter::CounterKind::Named(name) = kind {
                                self.form("CounterName", name);
                            }
                            self.object(object);
                        }
                    }
                }
            }
            Operation::DoubleCounters { objects, .. } => self.objects(objects),
            Operation::Conjure {
                actor,
                card,
                count,
                zone,
                owner,
                spot,
                ..
            } => {
                self.form("EventZone", zone);
                self.form("ConjuredCard", card);
                self.players(actor);
                self.expr(count);
                if let ConjuredCard::Duplicate(objects) = card {
                    self.objects(objects);
                }
                if let Some(owner) = owner {
                    self.player(owner);
                }
                if let Some(ConjureSpot::AmongTop(n) | ConjureSpot::FromTop(n)) = spot {
                    self.expr(n);
                }
            }
            Operation::ChangeSpell { subject, changes } => {
                match subject {
                    SpellSubject::One(spell) => self.object(spell),
                    SpellSubject::Next { spells, player, .. } => {
                        self.filter(spells);
                        self.player(player);
                    }
                }
                changes.iter().for_each(|change| self.change(change));
            }
            Operation::PlayCards {
                actor,
                action,
                cards,
                count,
                cost,
                ..
            } => {
                self.players(actor);
                self.form("CardPlayAction", action);
                self.objects(cards);
                self.count(count);
                self.form("PlayCost", cost);
            }
            Operation::PlayWith {
                actor,
                action,
                cards,
                count,
                terms,
                ..
            } => {
                self.players(actor);
                self.form("CardPlayAction", action);
                self.objects(cards);
                self.count(count);
                self.form("PlayCost", &terms.cost);
                if let Some(then) = &terms.then {
                    self.destination(then);
                }
            }
            Operation::ChangeZone {
                actor,
                cards,
                count,
                destination,
                ..
            } => {
                self.players(actor);
                self.objects(cards);
                if let Some(count) = count {
                    self.count(count);
                }
                self.destination(destination);
            }
            Operation::CopyCards {
                actor,
                objects,
                times,
            } => {
                self.players(actor);
                self.objects(objects);
                self.expr(times);
            }
            Operation::ChooseAtRandom { kind, .. } => self.form("ValueKind", kind),
            Operation::ChooseAction { actor, .. } => self.players(actor),
            Operation::ChooseValue { actor, kind, .. } => {
                self.form("ValueKind", kind);
                self.players(actor);
            }
            Operation::Keyword {
                actor,
                action,
                object,
                other,
                amount,
                ..
            } => {
                self.form("KeywordAction", action);
                self.players(actor);
                for object in object.iter().chain(other) {
                    self.object(object);
                }
                if let Some(amount) = amount {
                    self.expr(amount);
                }
            }
            Operation::CreateDelayedTrigger {
                event,
                start,
                expires,
                intervening_if,
                body,
            } => {
                self.event(event);
                self.form("DelayedStart", start);
                if let DelayedStart::PlayersNextTurn(player) = start {
                    self.player(player);
                }
                if let Some(duration) = expires {
                    self.duration(duration);
                }
                if let Some(condition) = intervening_if {
                    self.condition(condition);
                }
                self.program(body);
            }
            Operation::LookAt { player, objects } => {
                self.player(player);
                self.objects(objects);
            }
            Operation::FlipCoin { player } => self.player(player),
            Operation::CreateRegenerationShields { objects } => self.objects(objects),
            Operation::Search {
                player,
                library,
                top,
                choices,
                ..
            } => {
                self.player(player);
                self.player(library);
                if let Some(n) = top {
                    self.expr(n);
                }
                for choice in choices {
                    self.filter(&choice.filter);
                    self.count(&choice.count);
                    for c in &choice.constraints {
                        self.selection_constraint(c);
                    }
                }
            }
            Operation::Reveal { objects } => self.objects(objects),
            Operation::RevealUntil {
                library,
                filter,
                count,
                at_most,
            } => {
                self.player(library);
                self.expr(count);
                if let Some(at_most) = at_most {
                    self.expr(at_most);
                }
                self.filter(filter);
            }
            Operation::RevealUntilGroup {
                library,
                constraints,
            } => {
                self.player(library);
                for c in constraints {
                    self.selection_constraint(c);
                }
            }
            Operation::Shuffle { players, excluding } => {
                self.players(players);
                if let Some(o) = excluding {
                    self.objects(o);
                }
            }
            Operation::ReorderLibrary {
                player,
                library,
                objects,
                end,
                order,
            } => {
                self.player(player);
                self.player(library);
                self.objects(objects);
                self.form("SequenceEnd", end);
                self.form("CardOrder", order);
            }
            Operation::Scry { players, amount } | Operation::Surveil { players, amount } => {
                self.players(players);
                self.expr(amount);
            }
            Operation::JoinResults { .. } => {}
            Operation::ChoosePowerToughness { player, .. } => self.player(player),
            Operation::Counter {
                objects,
                destination,
            } => {
                self.objects(objects);
                self.destination(destination);
            }
            Operation::ModifyEvent { change } => {
                self.form("EventChange", change);
                match change {
                    EventChange::SetTapped(_)
                    | EventChange::SearchTop(_)
                    | EventChange::Repeats(_)
                    | EventChange::AddCopy { .. }
                    | EventChange::FlipExtraCoin
                    | EventChange::DamageAsInfect
                    | EventChange::IgnoreCounters(_) => {}
                    EventChange::AddCounters { kind, amount } => {
                        self.counter(kind);
                        self.expr(amount);
                    }
                    EventChange::SetController(player) => self.player(player),
                    EventChange::PreventDamage
                    | EventChange::Cancel
                    | EventChange::SetDestination(_) => {}
                    EventChange::PreventSomeDamage(amount)
                    | EventChange::PreventAllButDamage(amount)
                    | EventChange::SetAmount(amount) => self.expr(amount),
                    EventChange::RedirectDamage(recipients) => self.recipients(recipients),
                    EventChange::RedirectSomeDamage(amount, recipients) => {
                        self.expr(amount);
                        self.recipients(recipients);
                    }
                    EventChange::Then(body) => self.program(body),
                    EventChange::Entry(_)
                    | EventChange::AddMana(_)
                    | EventChange::ReplaceMana(_)
                    | EventChange::ReplaceColoredMana(_)
                    | EventChange::RollExtraDie(_)
                    | EventChange::ReplaceTokens { .. } => {}
                }
            }
            Operation::CreateTrigger {
                event,
                duration,
                announcement,
                body,
                intervening_if,
                ..
            } => {
                self.event(event);
                self.duration(duration);
                intervening_if.iter().for_each(|c| self.condition(c));
                self.announcement(announcement);
                self.program(body);
            }
            Operation::BindValue { value } => self.expr(value),
            Operation::RepeatProcess => {}
            Operation::DiscardObjects { objects } => self.objects(objects),
            Operation::ReflexiveTrigger {
                condition,
                announcement,
                body,
                ..
            } => {
                if let Some(condition) = condition {
                    self.condition(condition);
                }
                self.announcement(announcement);
                self.program(body);
            }
            Operation::ReturnAfter { objects, duration } => {
                self.objects(objects);
                self.duration(duration);
            }
            Operation::CreateReplacement {
                event,
                limit,
                duration,
                body,
            } => {
                self.replacement_event(event);
                self.form("ReplacementLimit", limit);
                if let ReplacementLimit::TotalDamage(amount)
                | ReplacementLimit::DamageToEach(amount) = limit
                {
                    self.expr(amount);
                }
                self.form("Duration", duration);
                self.program(body);
            }
            Operation::PayCost { player, cost, .. } => {
                self.player(player);
                self.cost(cost);
            }
            Operation::PayRepeatedly {
                player,
                cost,
                at_most,
            } => {
                self.player(player);
                self.cost(cost);
                self.expr(at_most);
            }
            Operation::RemoveCounters { removals } => self.removals(removals),
            Operation::Attach { attachments, host } => {
                self.objects(attachments);
                self.object(host);
            }
            Operation::CreateTokens {
                players,
                tokens,
                entry,
            } => {
                self.players(players);
                for token in tokens {
                    self.expr(&token.amount);
                    self.form("TokenWords", &token.words);
                    let definition = match &token.recipe {
                        TokenRecipe::Defined(definition) => definition,
                        TokenRecipe::Copy { object, .. } => {
                            self.form("TokenRecipe", &"Copy");
                            self.object(object);
                            continue;
                        }
                        TokenRecipe::CopyEach { objects, .. } => {
                            self.form("TokenRecipe", &"CopyEach");
                            self.objects(objects);
                            continue;
                        }
                        TokenRecipe::CopyOne { objects, .. } => {
                            self.form("TokenRecipe", &"CopyOne");
                            self.objects(objects);
                            continue;
                        }
                        TokenRecipe::CopyNamed { .. } => {
                            self.form("TokenRecipe", &"CopyNamed");
                            continue;
                        }
                        TokenRecipe::CopyAtRandom { .. } => {
                            self.form("TokenRecipe", &"CopyAtRandom");
                            continue;
                        }
                        TokenRecipe::Named(_) => {
                            self.form("TokenRecipe", &"Named");
                            continue;
                        }
                    };
                    self.form("TokenName", &definition.name);
                    for color in &definition.colors {
                        self.form("ManaColor", color);
                    }
                    for ty in &definition.supertypes {
                        self.form("SuperType", ty);
                    }
                    for ty in &definition.card_types {
                        self.form("CardType", ty);
                    }
                    for ty in &definition.subtypes {
                        self.form("Subtype", ty);
                    }
                    if let Some((objects, _)) = &definition.abilities_of {
                        self.objects(objects);
                    }
                    if let Some((power, toughness)) = &definition.power_toughness {
                        self.expr(power);
                        self.expr(toughness);
                    }
                    for ability in &definition.abilities {
                        self.ability(ability);
                    }
                }
                self.entry_states(entry);
                for (kind, amount) in &entry.counters {
                    self.form("CounterKind", kind);
                    if let crate::counter::CounterKind::Named(n) = kind {
                        self.form("CounterName", n);
                    }
                    self.expr(amount);
                }
            }
            Operation::SeparatePiles {
                separator,
                chooser,
                objects,
            } => {
                self.player(separator);
                self.player(chooser);
                self.objects(objects);
            }
            Operation::ChooseObjects {
                player,
                objects,
                count,
                constraints,
            } => {
                self.player(player);
                self.objects(objects);
                self.count(count);
                for c in constraints {
                    self.selection_constraint(c);
                }
            }
            Operation::Draw { players, amount }
            | Operation::Discard {
                players, amount, ..
            }
            | Operation::SetLifeTotal { players, amount }
            | Operation::GainLife { players, amount }
            | Operation::LoseLife { players, amount }
            | Operation::Mill { players, amount } => {
                self.players(players);
                self.expr(amount);
            }
            Operation::AddMana {
                players,
                mana,
                restriction,
            } => {
                self.players(players);
                self.mana(mana);
                if let Some(r) = restriction {
                    self.mana_use(r);
                }
            }
            Operation::DealDamage {
                source,
                recipients,
                amount,
                preventable: _,
            } => {
                self.object(source);
                self.recipients(recipients);
                self.expr(amount);
            }
            Operation::DealDamageWith {
                source,
                recipients,
                amount,
                rider,
            } => {
                self.object(source);
                self.recipients(recipients);
                self.expr(amount);
                match rider {
                    DamageRider::DividedEvenly(_) => {}
                    DamageRider::ExcessTo(player) | DamageRider::DividedBy(player) => {
                        self.player(player)
                    }
                    DamageRider::Prevented(prevented) => self.expr(prevented),
                }
            }
            Operation::DealDamageParts {
                source,
                parts,
                preventable: _,
            } => {
                self.object(source);
                for part in parts {
                    self.recipients(&part.recipients);
                    self.expr(&part.amount);
                }
            }
            Operation::DealDamageFromEach {
                sources,
                recipients,
                amount,
            } => {
                self.objects(sources);
                self.recipients(recipients);
                self.expr(amount);
            }
            Operation::Destroy {
                objects,
                regeneration: _,
            }
            | Operation::Tap { objects, tapped: _ } => self.objects(objects),
            Operation::Move {
                objects,
                destination,
                entry,
                ..
            } => {
                self.objects(objects);
                self.destination(destination);
                self.entry_states(entry);
                for (k, n) in &entry.counters {
                    self.counter(k);
                    self.expr(n);
                }
            }
            Operation::Sacrifice {
                players,
                objects,
                choose,
            } => {
                self.players(players);
                self.objects(objects);
                if let Some(c) = choose {
                    self.count(c);
                }
            }
            Operation::PutCounters { placements } => {
                for p in placements {
                    self.recipients(&p.recipients);
                    self.form("CounterKind", &p.kind);
                    if let crate::counter::CounterKind::Named(n) = &p.kind {
                        self.form("CounterName", n);
                    }
                    self.expr(&p.amount);
                    if let Some(c) = &p.selection {
                        self.player(&c.player);
                        self.count(&c.count);
                    }
                }
            }
            Operation::CreateContinuousEffect {
                source,
                affected,
                changes,
                duration,
                evaluation,
            } => {
                self.object(source);
                self.subjects(affected);
                for c in changes {
                    self.change(c);
                }
                self.duration(duration);
                self.form("EffectEvaluation", evaluation);
            }
        }
    }
    fn selection_constraint(&mut self, c: &SelectionConstraint) {
        self.form("SelectionConstraint", c);
        match c {
            SelectionConstraint::CommonProperty(property) => {
                self.form("DistinctProperty", property)
            }
            SelectionConstraint::DifferentProperty(property) => self.form("Property", property),
            SelectionConstraint::DistinctCount {
                property, amount, ..
            } => {
                self.form("DistinctProperty", property);
                self.expr(amount);
            }
            SelectionConstraint::TotalManaSymbols { amount, .. } => self.expr(amount),
            _ => {}
        }
        if let SelectionConstraint::TotalProperty {
            property,
            comparison,
            amount,
        } = c
        {
            self.form("Property", property);
            self.form("Comparison", comparison);
            self.expr(amount);
        }
        if let SelectionConstraint::NamedAfterOneEach(objects) = c {
            self.objects(objects);
        }
        if let SelectionConstraint::CanMoveTo {
            destination, entry, ..
        } = c
        {
            self.destination(destination);
            self.entry_states(entry);
            for (kind, amount) in &entry.counters {
                self.counter(kind);
                self.expr(amount);
            }
        }
    }
    fn counter(&mut self, kind: &crate::counter::CounterKind) {
        self.form("CounterKind", kind);
        if let crate::counter::CounterKind::Named(name) = kind {
            self.form("CounterName", name);
        }
    }
    fn protection(&mut self, quality: &Protection) {
        self.form("Protection", quality);
        match quality {
            Protection::Objects { kind, filter } => {
                self.form("ObjectKind", kind);
                self.filter(filter);
            }
            Protection::Players(players) => self.players(players),
            Protection::Each(qualities) => {
                for quality in qualities {
                    self.protection(quality);
                }
            }
        }
    }
    fn keyword(&mut self, keyword: &Keyword) {
        self.form("Keyword", keyword);
        match keyword {
            Keyword::Protection(quality) | Keyword::HexproofFrom(quality) => {
                self.protection(quality)
            }
            Keyword::Landwalk(lands) => self.filter(lands),
            Keyword::Ward(cost) => self.cost(cost),
            _ => {}
        }
    }
    fn removals(&mut self, removals: &[CounterRemoval]) {
        for removal in removals {
            self.player(&removal.player);
            self.recipients(&removal.recipients);
            if let Some(kind) = &removal.kind {
                self.counter(kind);
            }
            self.form("CounterAmount", &removal.amount);
            if let CounterAmount::Count(count) | CounterAmount::AmongThem(count) = &removal.amount {
                self.count(count);
            }
            if let Some(selection) = &removal.selection {
                self.player(&selection.player);
                self.count(&selection.count);
            }
        }
    }
    fn spell_cost_modifier(&mut self, modifier: &SpellCostModifier) {
        self.form("SpellCostModifier", modifier);
        match modifier {
            SpellCostModifier::Increase(cost) => self.cost(cost),
            SpellCostModifier::Alternate { .. }
            | SpellCostModifier::AlternateDuring { .. }
            | SpellCostModifier::PayToReduce { .. }
            | SpellCostModifier::OptionalAdditional { .. } => {}
            SpellCostModifier::Scaled { per, .. } => {
                if let CostScale::Times(times) = per {
                    self.expr(times);
                }
            }
            SpellCostModifier::Minimum(n) => self.expr(n),
            SpellCostModifier::Reduce { reductions, .. } => {
                for r in reductions {
                    if let Some(color) = &r.color {
                        self.form("ManaColor", color);
                    }
                    self.expr(&r.amount);
                }
            }
        }
    }
    fn cost(&mut self, cost: &Cost) {
        for c in &cost.parts {
            self.form("CostPart", c);
            match c {
                CostPart::Primitive { name, .. } => self.primitive(name),
                CostPart::ManaWithX { x, .. } => {
                    if let Some(x) = x {
                        self.expr(x);
                    }
                }
                CostPart::Exert(object)
                | CostPart::Unattach(object)
                | CostPart::Unspecialize(object) => self.object(object),
                CostPart::Keyword { amount, .. } => {
                    if let Some(amount) = amount {
                        self.expr(amount);
                    }
                }
                CostPart::DiscardHand => {}
                CostPart::BeholdAndExile(_) => {}
                CostPart::Behold { count, .. } => self.expr(count),
                CostPart::ExiledToGraveyard(cards) => self.objects(cards),
                CostPart::RevealTop(filter) => self.filter(filter),
                CostPart::Waterbend(_) | CostPart::Teamwork(_) => {}
                CostPart::MoveCounters {
                    from, to, amount, ..
                } => {
                    self.form("MovedCounters", amount);
                    self.objects(from);
                    self.objects(to);
                    if let MovedCounters::Exactly(amount) | MovedCounters::AtLeast(amount) = amount
                    {
                        self.expr(amount);
                    }
                }
                CostPart::WaterbendX { x, .. } => self.expr(x),
                CostPart::PutCounters(placements) => {
                    for p in placements {
                        self.recipients(&p.recipients);
                        self.form("CounterKind", &p.kind);
                        if let crate::counter::CounterKind::Named(n) = &p.kind {
                            self.form("CounterName", n);
                        }
                        self.expr(&p.amount);
                    }
                }
                CostPart::Constrained { part, constraints } => {
                    self.cost(&Cost {
                        parts: vec![part.as_ref().clone()],
                    });
                    for constraint in constraints {
                        self.selection_constraint(constraint);
                    }
                }
                CostPart::ManaCostOf(object) => self.object(object),
                CostPart::Move {
                    selections,
                    entry,
                    destination,
                    ..
                } => {
                    self.destination(destination);
                    for s in selections {
                        self.objects(&s.objects);
                        if let Some(count) = &s.choose {
                            self.count(count);
                        }
                    }
                    self.entry_states(entry);
                    for (kind, n) in &entry.counters {
                        self.counter(kind);
                        self.expr(n);
                    }
                }
                CostPart::Choice(options) => {
                    for cost in options {
                        self.cost(cost);
                    }
                }
                CostPart::Discard { count, filter, .. } => {
                    self.expr(count);
                    self.filter(filter);
                }
                CostPart::RemoveCounters { removals } => self.removals(removals),
                CostPart::Mana(m) => {
                    for symbol in m {
                        self.form("ManaSymbol", symbol);
                    }
                }
                CostPart::ChangeTapState { selections, .. } => {
                    for selection in selections {
                        self.objects(&selection.objects);
                        if let Some(count) = &selection.choose {
                            self.count(count);
                        }
                    }
                }
                CostPart::TapSymbol
                | CostPart::LifeEqualToOwnManaValue
                | CostPart::ManaAnyNumberOfTimes(_)
                | CostPart::RevealChoice(_)
                | CostPart::EnergyOneOrMore
                | CostPart::ManaFromTreasure(_)
                | CostPart::ShuffleFromHand
                | CostPart::ManaCostReduced { .. } => {}
                CostPart::ManaXBounded { value, .. } => self.expr(value),
                CostPart::UnattachFrom { attachments, hosts } => {
                    self.objects(attachments);
                    self.objects(hosts);
                }
                CostPart::EachPlayerAction { players, .. } => self.players(players),
                CostPart::ManaReducedForEach { per, .. } => self.expr(per),
                CostPart::PlayFromTop(filter) => self.filter(filter),
                CostPart::ObjectAction { objects, .. } => self.objects(objects),
                CostPart::SinglePlayersCards(part) => self.cost(&Cost {
                    parts: vec![part.as_ref().clone()],
                }),
                CostPart::Life(n)
                | CostPart::Energy(n)
                | CostPart::Loyalty(n)
                | CostPart::Draw { count: n }
                | CostPart::Mill { count: n } => self.expr(n),
                CostPart::PlayerAction { player, body } => {
                    self.player(player);
                    self.program(body);
                }
                CostPart::Reveal { count, filter, .. } => {
                    self.count(count);
                    self.filter(filter);
                }
                CostPart::Repeated { times, .. } => self.expr(times),
                CostPart::Sacrifice { objects, choose } => {
                    self.objects(objects);
                    if let Some(c) = choose {
                        self.count(c);
                    }
                }
            }
        }
    }
    fn casting_rule(&mut self, rule: &CastingRule) {
        self.form("CastingRule", rule);
        match rule {
            CastingRule::FlashWithRider(_) => {}
            CastingRule::TaxPaidAsLife(life) => self.expr(life),
            CastingRule::Permission {
                origin,
                player,
                from,
                require,
                cost,
                leave_stack,
            } => {
                self.form("CastingOrigin", origin);
                self.player(player);
                self.zone_scope(from);
                self.filter(require);
                self.cost(cost);
                if let Some(destination) = leave_stack {
                    self.destination(destination);
                }
            }
            CastingRule::AdditionalCost {
                cost, repetition, ..
            } => {
                self.cost(cost);
                self.form("CostRepetition", repetition);
            }
            CastingRule::AlternativeCost(cost) | CastingRule::AlternativeCostWithFlash(cost) => {
                self.cost(cost)
            }
            CastingRule::ReduceForEachPaid { cost, .. } => self.cost(cost),
            CastingRule::ReduceCost(reductions) => {
                for r in reductions {
                    if let Some(color) = &r.color {
                        self.form("ManaColor", color);
                    }
                    self.expr(&r.amount);
                }
            }
            CastingRule::Require(c) => self.condition(c),
            CastingRule::Flash { additional_cost } => {
                if let Some(c) = additional_cost {
                    self.cost(c);
                }
            }
            CastingRule::ProhibitTargets { domain, filter } => {
                self.form("ObjectKind", &domain.kind);
                for z in &domain.zones {
                    self.form("EventZone", z);
                }
                self.filter(filter);
            }
            CastingRule::RestrictMana { portion, filter } => {
                self.form("ManaCostPortion", portion);
                self.form("ManaPaymentFilter", filter);
                match filter {
                    ManaPaymentFilter::Colors(colors) => {
                        for color in colors {
                            self.form("ManaColor", color);
                        }
                    }
                    ManaPaymentFilter::ProducedBy(f) => self.filter(f),
                    ManaPaymentFilter::None => {}
                }
            }
            CastingRule::SpendAsThoughAnyType => {}
            CastingRule::If { condition, rule } => {
                self.condition(condition);
                self.casting_rule(rule);
            }
        }
    }
    fn mana(&mut self, mana: &ManaProduction) {
        self.form("ManaProduction", mana);
        match mana {
            ManaProduction::SameTypeAsProduced
            | ManaProduction::CommanderIdentity
            | ManaProduction::DifferentColors(_)
            | ManaProduction::OtherColor
            | ManaProduction::LastNoted { .. }
            | ManaProduction::LostThisWay(_) => {}
            ManaProduction::DraftNotedColor(note) => self.player(&note.player),
            ManaProduction::ColorsAmong { objects, .. } => self.objects(objects),
            ManaProduction::ManaCostOf(object) | ManaProduction::SymbolsOf(object) => {
                self.object(object)
            }
            ManaProduction::CouldProduce { objects, .. } => self.objects(objects),
            ManaProduction::Primitive { name, .. } => self.primitive(name),
            ManaProduction::Amount { color, amount } => {
                self.form("ManaColor", color);
                self.expr(amount);
            }
            ManaProduction::All(v) | ManaProduction::ChooseOne(v) => {
                for p in v {
                    self.mana(p);
                }
            }
            ManaProduction::Repeat {
                amount,
                production,
                choice,
            } => {
                self.expr(amount);
                self.mana(production);
                self.form("ManaChoiceTiming", choice);
            }
        }
    }
    fn mana_use(&mut self, r: &ManaUse) {
        self.form("ManaUse", r);
        match r {
            ManaUse::CastSpell(f) | ManaUse::ActivateAbility { sources: f, .. } => self.filter(f),
            ManaUse::All(v) | ManaUse::Either(v) => {
                for r in v {
                    self.mana_use(r);
                }
            }
            ManaUse::Not(r) => self.mana_use(r),
            ManaUse::Persists(until) => self.duration(until),
            ManaUse::SpentOn {
                spells,
                changes,
                until,
            } => {
                self.filter(spells);
                changes.iter().for_each(|c| self.change(c));
                self.duration(until);
            }
            ManaUse::Purpose(_) | ManaUse::WhenSpent { .. } => {}
            ManaUse::SpentGives { spells, .. } => self.filter(spells),
            ManaUse::Primitive { name, .. } => self.primitive(name),
        }
    }
    fn activation_filter(&mut self, f: &ActivationFilter) {
        self.form("ActivationFilter", f);
        match f {
            ActivationFilter::Is(_)
            | ActivationFilter::Any
            | ActivationFilter::Mana
            | ActivationFilter::Cycling => {}
            ActivationFilter::Of(sources) => self.filter(sources),
            ActivationFilter::Not(f) => self.activation_filter(f),
            ActivationFilter::All(v) | ActivationFilter::Either(v) => {
                for f in v {
                    self.activation_filter(f);
                }
            }
        }
    }
    fn defenders(&mut self, d: &AttackDefenders) {
        self.form("AttackDefenders", d);
        match d {
            AttackDefenders::Any => {}
            AttackDefenders::Objects(f) => self.filter(f),
            AttackDefenders::Players(p) | AttackDefenders::PlayersAndPlaneswalkers(p) => {
                self.players(p)
            }
        }
    }
    fn object_action(&mut self, a: &ObjectAction) {
        self.form("ObjectAction", a);
        match a {
            ObjectAction::Attack(d) => self.defenders(d),
            ObjectAction::Block(f) | ObjectAction::BeBlocked(f) => self.filter(f),
            ObjectAction::Activate(f) => self.activation_filter(f),
            ObjectAction::BeTargeted(f) => self.filter(f),
            ObjectAction::BeAttachedBy { attachments, .. } => self.filter(attachments),
            ObjectAction::PhaseIn
            | ObjectAction::BePlayed
            | ObjectAction::TurnFaceUp
            | ObjectAction::BecomeSuspected
            | ObjectAction::BeBeamedUp
            | ObjectAction::ChangeControl
            | ObjectAction::LoseCounters(_)
            | ObjectAction::BeAttacked
            | ObjectAction::Untap
            | ObjectAction::Cast
            | ObjectAction::Sacrifice
            | ObjectAction::Regenerate
            | ObjectAction::Counter
            | ObjectAction::ReceiveCounters(_)
            | ObjectAction::Transform
            | ObjectAction::PhaseOut
            | ObjectAction::Crew
            | ObjectAction::Copy => {}
        }
    }
    fn rule(&mut self, rule: &RuleChange) {
        self.form("RuleChange", rule);
        match rule {
            RuleChange::Primitive { name, .. } => self.primitive(name),
            RuleChange::Prohibit(a) => self.object_action(a),
            RuleChange::GoadedBy(player) => self.player(player),
            RuleChange::CantAttackDefenderIf(condition) => self.condition(condition),
            RuleChange::TapPowerBonus { bonus, .. } => self.expr(bonus),
            RuleChange::MayExert(_)
            | RuleChange::BlockedAsThoughLacking(_)
            | RuleChange::CantGainAbility(_) => {}
            RuleChange::DamageStaysMarked | RuleChange::LegendRuleExempt => {}
            RuleChange::TargetableDespite { sources: f, .. }
            | RuleChange::AttachOnlyTo(f)
            | RuleChange::AttackDrags(f)
            | RuleChange::AttacksWhenAttacking(f)
            | RuleChange::BlocksDespite { attackers: f, .. } => self.filter(f),
            RuleChange::BlocksOnlyIf(condition) => self.condition(condition),
            RuleChange::BlockedByDefender { players, .. } => self.players(players),
            RuleChange::TapCostUsesToughness(_) | RuleChange::Unique(_) => {}
            RuleChange::CounterLimit { max, .. } => self.expr(max),
            RuleChange::ActivationsEachTurn { abilities, times } => {
                self.filter(abilities);
                self.expr(times);
            }
            RuleChange::OwnActivationCost { abilities, .. } => self.filter(abilities),
            RuleChange::AssignDamageTo {
                players,
                permanents,
            } => {
                if let Some(player) = players {
                    self.player(player);
                }
                self.filter(permanents);
            }
            RuleChange::RequireCombat { action, amount } => {
                self.object_action(action);
                self.form("RequirementAmount", amount);
                if let RequirementAmount::AtLeast(n) = amount {
                    self.expr(n);
                }
            }
            RuleChange::CombatCost {
                action,
                payer,
                cost,
            } => {
                self.object_action(action);
                self.player(payer);
                self.cost(cost);
            }
            RuleChange::CombatGroup {
                role,
                participants,
                comparison,
                amount,
                ..
            } => {
                self.form("CombatRole", role);
                self.filter(participants);
                self.form("Comparison", comparison);
                self.expr(amount);
            }
            RuleChange::AttackException { ignore, defenders } => {
                self.form("AttackCheck", ignore);
                self.defenders(defenders);
            }
            RuleChange::BlockWhileTapped => {}
            RuleChange::BlockCapacity(capacity) => {
                self.form("BlockCapacity", capacity);
                if let BlockCapacity::Additional(n) = capacity {
                    self.expr(n);
                }
            }
            RuleChange::CombatDamage(rule) => self.form("CombatDamageRule", rule),
            RuleChange::UntapStep { players, behavior } => {
                self.players(players);
                self.form("UntapBehavior", behavior);
            }
        }
    }
    fn change(&mut self, c: &Change) {
        self.form("Change", c);
        match c {
            Change::Continuing { changes, until } => {
                self.duration(until);
                changes.iter().for_each(|c| self.change(c));
            }
            Change::Copiable(changes) => changes.iter().for_each(|c| self.change(c)),
            Change::Primitive { name, .. } => self.primitive(name),
            Change::PlayerRule(rule) => self.player_rule(rule),
            Change::Rule(rule) => self.rule(rule),
            Change::Pump { power, toughness }
            | Change::SetBasePowerToughness { power, toughness } => {
                self.expr(power);
                self.expr(toughness);
            }
            Change::SetPower(n) | Change::SetToughness(n) => self.expr(n),
            Change::SetController(p) => self.player(p),
            Change::GrantKeyword(k) => self.keyword(k),
            Change::GainAbilitiesOf { objects, .. } => self.objects(objects),
            Change::AddNames(filter) => self.filter(filter),
            Change::CreatureTypesOf { object, .. } => self.object(object),
            Change::OnlyAbilities(abilities) => abilities.iter().for_each(|a| self.ability(a)),
            Change::GrantAbilityIfAbsent(a) => self.ability(a),
            Change::GrantAbility(a) | Change::OnlyAbility(a) => self.ability(a),
            Change::CreatureTypes { mode, types } => {
                self.form("ChangeMode", mode);
                self.form("CreatureTypeSet", types);
            }
            Change::Colors { mode, colors } => {
                self.form("ChangeMode", mode);
                for color in colors {
                    self.form("ManaColor", color);
                }
            }
            Change::ChosenColor { mode } | Change::ColorOfMana { mode, .. } => {
                self.form("ChangeMode", mode)
            }
            Change::ChosenType { kind, mode } => {
                self.form("ValueKind", kind);
                self.form("ChangeMode", mode);
            }
            Change::ChosenAbility => {}
            Change::AddCardType(_)
            | Change::RemoveCardType(_)
            | Change::AddSupertype(_)
            | Change::RemoveSupertype(_)
            | Change::AddLandType(_)
            | Change::AddArtifactType(_)
            | Change::SetArtifactType(_)
            | Change::SetCardTypes(_)
            | Change::RemoveAllAbilities
            | Change::SwitchPowerToughness
            | Change::LosesAbility(_)
            | Change::SetName(_)
            | Change::LandTypes { .. }
            | Change::RemoveAllLandTypes
            | Change::RemoveSubtype(_)
            | Change::ColorlessDamageSource
            | Change::Incorporate(_)
            | Change::TextOf { .. }
            | Change::ReplaceWord { .. }
            | Change::AddEnchantmentType(_)
            | Change::SetLandType(_) => {}
            Change::EntersWithCounters { kind, amount } => {
                self.form("CounterKind", kind);
                self.expr(amount);
            }
            Change::SpellCost(modifier) => self.spell_cost_modifier(modifier),
            Change::CantBeCountered => {}
            Change::ActivationRule(rule) => {
                self.form("ActivationRule", rule);
                match rule {
                    ActivationRule::RestrictMana { .. } | ActivationRule::ManaInstead(_) => {}
                    ActivationRule::Cost {
                        modifier,
                        condition,
                    } => {
                        self.spell_cost_modifier(modifier);
                        if let Some(condition) = condition {
                            self.condition(condition);
                        }
                    }
                    ActivationRule::MaxPerTurn(n)
                    | ActivationRule::XAtMost(n)
                    | ActivationRule::XIs(n) => self.expr(n),
                    ActivationRule::OnlyBy(players) => self.players(players),
                    ActivationRule::XNotZero
                    | ActivationRule::CantBeCopied
                    | ActivationRule::AnyTimeOfInstant => {}
                }
            }
            Change::CopyOf { object, .. } => self.object(object),
        }
    }
    fn subjects(&mut self, subjects: &EffectSubjects) {
        self.form("EffectSubjects", subjects);
        match subjects {
            EffectSubjects::Objects(objects) => self.objects(objects),
            EffectSubjects::Players(players) => self.players(players),
        }
    }
    fn player_action(&mut self, action: &PlayerAction) {
        self.form("PlayerAction", action);
        match action {
            PlayerAction::PlayLand { from, filter } | PlayerAction::CastSpell { from, filter } => {
                if let Some(zone) = from {
                    self.zone_scope(zone);
                }
                self.filter(filter);
            }
            PlayerAction::ReceiveCounters(Some(kind)) => self.counter(kind),
            PlayerAction::AttackWith(filter) | PlayerAction::BlockWith(filter) => {
                self.filter(filter)
            }
            PlayerAction::AttackInto(recipients) => self.recipients(recipients),
            PlayerAction::PayLife(payment) => self.mana_payment(payment),
            PlayerAction::Sacrifice { objects, paying } => {
                self.filter(objects);
                if let Some(payment) = paying {
                    self.mana_payment(payment);
                }
            }
            PlayerAction::Activate(filter) => self.activation_filter(filter),
            PlayerAction::LoseGame {
                reason: Some(reason),
            } => self.form("GameLossReason", reason),
            _ => {}
        }
    }
    fn mana_payment(&mut self, payment: &ManaPayment) {
        if let ManaPayment::Spells(filter) | ManaPayment::Abilities(filter) = payment {
            self.filter(filter);
        }
    }
    fn player_rule(&mut self, rule: &PlayerRule) {
        self.form("PlayerRule", rule);
        match rule {
            PlayerRule::PlayPermissionWith { permission, terms } => {
                self.player_rule(permission);
                if let Some(players) = &terms.during {
                    self.players(players);
                }
                if let Some(condition) = &terms.condition {
                    self.condition(condition);
                }
            }
            PlayerRule::PlayPermission {
                cards,
                from,
                action,
                filter,
            } => {
                self.form("EventZone", from);
                self.objects(cards);
                self.form("CardPlayAction", action);
                self.filter(filter);
            }
            PlayerRule::Primitive { name, .. } => self.primitive(name),
            PlayerRule::SpellsHaveAbilities {
                spells,
                nth_each_turn,
                ..
            } => {
                self.filter(spells);
                if let Some(nth) = nth_each_turn {
                    self.expr(nth);
                }
            }
            PlayerRule::SpendManaAs { on, .. } => {
                if let ManaPayment::Spells(filter) | ManaPayment::Abilities(filter) = on {
                    self.filter(filter);
                }
            }
            PlayerRule::ActivationCost { abilities, .. } => self.filter(abilities),
            PlayerRule::Prohibit(action) => self.player_action(action),
            PlayerRule::LimitPerTurn { action, amount } => {
                self.player_action(action);
                self.expr(amount);
            }
            PlayerRule::HandSize(rule) => {
                self.form("HandSizeRule", rule);
                if let HandSizeRule::Set(n) | HandSizeRule::Adjust(n) = rule {
                    self.expr(n);
                }
            }
            PlayerRule::AdditionalLandPlays(count) => self.count(count),
            PlayerRule::SpellCost { spells, modifier } => {
                self.filter(spells);
                self.spell_cost_modifier(modifier);
            }
            PlayerRule::SpellTiming { spells, timing } => {
                self.filter(spells);
                self.form("SpellTiming", timing);
            }
            PlayerRule::Skip(section) => self.form("TurnSection", section),
            PlayerRule::Visibility { objects, audience } => {
                self.objects(objects);
                self.form("VisibilityAudience", audience);
            }
            PlayerRule::RetainMana {
                colors: Some(colors),
            } => {
                for color in colors {
                    self.form("ManaColor", color);
                }
            }
            PlayerRule::UntapLimit { permanents, amount } => {
                self.filter(permanents);
                self.expr(amount);
            }
            PlayerRule::GrantKeyword(keyword) => {
                self.form("PlayerKeyword", keyword);
                if let PlayerKeyword::Protection(quality) | PlayerKeyword::HexproofFrom(quality) =
                    keyword
                {
                    self.protection(quality);
                }
            }
            _ => {}
        }
    }
    fn zone_scope(&mut self, scope: &ZoneScope) {
        self.form("ZoneScope", scope);
        match scope {
            ZoneScope::Shared(zone) => self.form("EventZone", zone),
            ZoneScope::Players { zone, players } => {
                self.form("EventZone", zone);
                self.players(players);
            }
        }
    }
    fn destination(&mut self, destination: &Destination) {
        self.form("Destination", destination);
        if let Destination::OwnersLibraryFromTop(depth) = destination {
            self.expr(depth);
        }
        if let Destination::Battlefield(player) = destination {
            self.player(player);
        }
    }
    fn object(&mut self, r: &ObjectRef) {
        self.form("ObjectRef", r);
        if let ObjectRef::Damage(role) = r {
            self.form("DamageSubject", role);
        }
        if let ObjectRef::Query { objects, .. } = r {
            self.objects(objects);
        }
        if let ObjectRef::AttachedTo(attachment) | ObjectRef::PairedWith(attachment) = r {
            self.object(attachment);
        }
        if let ObjectRef::Result { phase, .. } = r {
            self.form("ObjectPhase", phase);
        }
        if let ObjectRef::TheOther { among, excluding } = r {
            self.objects(among);
            self.objects(excluding);
        }
        if let ObjectRef::AtEnd { objects, end } = r {
            self.objects(objects);
            self.form("SequenceEnd", end);
        }
    }
    fn objects(&mut self, r: &ObjectSet) {
        self.form("ObjectSet", r);
        match r {
            ObjectSet::Scoped => {}
            ObjectSet::Named(_) => {}
            ObjectSet::Earlier { by, .. } => {
                if let Some(by) = by {
                    self.players(by);
                }
            }
            ObjectSet::Primitive { name, .. } => self.primitive(name),
            ObjectSet::DamageSources(filter)
            | ObjectSet::AnyZone(filter)
            | ObjectSet::OutsideTheGame(filter) => self.filter(filter),
            ObjectSet::LibraryTops(players) => self.players(players),
            ObjectSet::SplicedOnto(object) | ObjectSet::TargetsOf(object) => self.object(object),
            ObjectSet::FromHistory { event, window } => {
                self.event(event);
                self.form("HistoryWindow", window);
            }
            ObjectSet::Random { objects, count } => {
                self.objects(objects);
                self.expr(count);
            }
            ObjectSet::Chosen {
                chooser,
                objects,
                count,
            } => {
                self.players(chooser);
                self.objects(objects);
                self.count(count);
            }
            ObjectSet::Affected(_) => {}
            ObjectSet::CostObjects {
                payment,
                view,
                phase,
                ..
            } => {
                self.form("PaymentRef", payment);
                self.form("CostObjectView", view);
                self.form("ObjectPhase", phase);
            }
            ObjectSet::Observed(_) => {}
            ObjectSet::Difference { objects, excluding } => {
                self.objects(objects);
                self.objects(excluding);
            }
            ObjectSet::Matching { objects, filter } => {
                self.objects(objects);
                self.filter(filter);
            }
            ObjectSet::EventGroup => {}
            ObjectSet::Damage(role) => self.form("DamageSubject", role),
            ObjectSet::Zone {
                scope,
                kind,
                filter,
            } => {
                self.zone_scope(scope);
                self.form("ObjectKind", kind);
                self.filter(filter);
            }
            ObjectSet::Take {
                objects,
                count,
                from,
            } => {
                self.objects(objects);
                self.expr(count);
                self.form("SequenceEnd", from);
            }
            ObjectSet::Union(sets) => {
                for set in sets {
                    self.objects(set);
                }
            }
            ObjectSet::Transition { phase, .. } => self.form("ObjectPhase", phase),
            ObjectSet::MovedBy { phase, player, .. } => {
                self.form("ObjectPhase", phase);
                self.player(player);
            }
            ObjectSet::One(r) => self.object(r),
            ObjectSet::Battlefield(f) => self.filter(f),
            ObjectSet::InZone { objects, zone } => {
                self.objects(objects);
                self.form("EventZone", zone);
            }
            ObjectSet::Targets(_)
            | ObjectSet::Selected(_)
            | ObjectSet::Unselected(_)
            | ObjectSet::Created(_)
            | ObjectSet::Copies(_)
            | ObjectSet::Unpaid(_) => {}
        }
    }
    fn player(&mut self, r: &PlayerRef) {
        self.form("PlayerRef", r);
        if let PlayerRef::CasterOf(r)
        | PlayerRef::ControllerOf(r)
        | PlayerRef::OwnerOf(r)
        | PlayerRef::EnchantedBy(r) = r
        {
            self.object(r);
        }
        if let PlayerRef::PlayerOrControllerOf(recipient) = r {
            self.recipients(recipient);
        }
        if let PlayerRef::AsOf { moment, player } = r {
            if let Moment::LastEvent { event, window } = moment.as_ref() {
                self.event(event);
                self.form("HistoryWindow", window);
            }
            self.player(player);
        }
    }
    fn players(&mut self, r: &PlayerSet) {
        self.form("PlayerSet", r);
        match r {
            PlayerSet::Scoped => {}
            PlayerSet::Intersection(sets) => {
                for set in sets {
                    self.players(set);
                }
            }
            PlayerSet::Matching { condition, .. } => self.condition(condition),
            PlayerSet::Union(sets) => {
                for players in sets {
                    self.players(players);
                }
            }
            PlayerSet::One(p) | PlayerSet::OpponentsOf(p) | PlayerSet::OthersThan(p) => {
                self.player(p)
            }
            PlayerSet::All
            | PlayerSet::Targets(_)
            | PlayerSet::DamageRecipients
            | PlayerSet::DamagedBy(_) => {}
        }
    }
    fn recipients(&mut self, r: &Recipients) {
        self.form("Recipients", r);
        match r {
            Recipients::Objects(o) => self.objects(o),
            Recipients::Players(p) => self.players(p),
            Recipients::Chosen | Recipients::DamagedByEvent => {}
            Recipients::AttackedBy(attacker) => self.object(attacker),
            Recipients::Union(items) => {
                for r in items {
                    self.recipients(r);
                }
            }
            Recipients::Targets(_) | Recipients::Target { .. } => {}
        }
    }
    fn vote_option(&mut self, option: &VoteOption) {
        match option {
            VoteOption::Word(_) => {}
            VoteOption::Player(player) => self.player(player),
            VoteOption::Object(object) => self.object(object),
        }
    }
    fn player_number(&mut self, number: &PlayerNumber) {
        if let PlayerNumber::Permanents(filter) = number {
            self.filter(filter);
        }
    }
    fn expr(&mut self, e: &Expr) {
        self.form("Expr", e);
        match e {
            Expr::DamageBy(_)
            | Expr::ModesChosen(_)
            | Expr::AdditionalCostPaid(_)
            | Expr::DraftNoted { .. }
            | Expr::TurnsSinceForetold
            | Expr::ActivationCostSymbols(_)
            | Expr::ChosenColorsOf(_)
            | Expr::AbilitiesAmong { .. } => {}
            Expr::Earlier { filter, by, .. } => {
                if let Some(filter) = filter {
                    self.filter(filter);
                }
                if let Some(by) = by {
                    self.player(by);
                }
            }
            Expr::Dice(_) | Expr::CoinFlipsWon(_) | Expr::Game(_) => {}
            Expr::Stickers { objects, .. } => self.objects(objects),
            Expr::Noted { object, .. } => {
                if let Some(object) = object {
                    self.object(object);
                }
            }
            Expr::Difference(a, b) => {
                self.expr(a);
                self.expr(b);
            }
            Expr::LargestSharedGroup { objects, .. } => self.objects(objects),
            Expr::ManaSpentFrom { object, sources } => {
                self.object(object);
                self.filter(sources);
            }
            Expr::AsOf { value, .. } => self.expr(value),
            Expr::Power(base, exponent) => {
                self.expr(base);
                self.expr(exponent);
            }
            Expr::TimesActivatedThisTurn | Expr::TimesResolvedThisTurn | Expr::TurnNumber => {}
            Expr::PartySize(player) | Expr::StartingLife(player) => self.player(player),
            Expr::Votes(option) => self.vote_option(option),
            Expr::PlayerNumber { player, number } => {
                self.player(player);
                self.player_number(number);
            }
            Expr::AmongPlayers {
                players, number, ..
            } => {
                self.players(players);
                self.player_number(number);
            }
            Expr::TimesKicked(object) => self.object(object),
            Expr::TimesMutated(object) => self.object(object),
            Expr::ManaOf { mana, filter } => {
                self.form("ManaOccurrence", mana);
                for color in filter.colors.iter().flatten() {
                    self.form("ManaColor", color);
                }
            }
            Expr::CountersPaid { object, .. } => self.object(object),
            Expr::SharedValues {
                object,
                property,
                with,
            } => {
                self.object(object);
                self.form("DistinctProperty", property);
                self.objects(with);
            }
            Expr::RepeatedCostsUnpaid { player, .. } => self.player(player),
            Expr::BoundValue(_)
            | Expr::ChosenParity
            | Expr::ChosenNumber
            | Expr::CountersRemoved(_)
            | Expr::PaidCountersRemoved { .. }
            | Expr::Performed(_)
            | Expr::ClampedDamage(_)
            | Expr::ChosenPowerToughness { .. }
            | Expr::DieResultAt { .. }
            | Expr::DistributedShare(_)
            | Expr::DieResult(_)
            | Expr::HeadsFlipped(_) => {}
            Expr::ExcessDamage { to, .. } => self.filter(to),
            Expr::Primitive { name, .. } => self.primitive(name),
            Expr::EventHistory {
                event,
                window,
                measure,
            } => {
                self.event(event);
                self.form("HistoryWindow", window);
                self.form("HistoryMeasure", measure);
            }
            Expr::CountPlayers(players) => self.players(players),
            Expr::ManaSpent {
                object,
                filter,
                aggregation,
            } => {
                self.object(object);
                self.form("SpentManaAggregation", aggregation);
                if let Some(colors) = &filter.colors {
                    for color in colors {
                        self.form("ManaColor", color);
                    }
                }
            }
            Expr::CountManaSymbols { objects, colors } => {
                self.objects(objects);
                for c in colors {
                    self.form("ManaColor", c);
                }
            }
            Expr::Aggregate {
                objects,
                property,
                aggregation,
            } => {
                self.objects(objects);
                self.form("Property", property);
                self.form("Aggregation", aggregation);
            }
            Expr::CountDistinct { objects, property } => {
                self.objects(objects);
                self.form("DistinctProperty", property);
            }
            Expr::ObjectProperty {
                object,
                property,
                information,
            } => {
                self.object(object);
                self.form("Property", property);
                self.form("Information", information);
            }
            Expr::PlayerLife(p) => self.player(p),
            Expr::AnyOf(values) => values.iter().for_each(|value| self.expr(value)),
            Expr::Add(a, b)
            | Expr::Subtract(a, b)
            | Expr::Multiply(a, b)
            | Expr::Minimum(a, b)
            | Expr::Maximum(a, b) => {
                self.expr(a);
                self.expr(b);
            }
            Expr::Conditional {
                condition,
                then,
                otherwise,
            } => {
                self.condition(condition);
                self.expr(then);
                self.expr(otherwise);
            }
            Expr::Divide {
                numerator,
                denominator,
                rounding,
            } => {
                self.expr(numerator);
                self.expr(denominator);
                self.form("Rounding", rounding);
            }
            Expr::CountObjects(o) => self.objects(o),
            Expr::CastX(object) => self.object(object),
            Expr::CountCounters {
                recipients,
                kind,
                information,
            } => {
                self.recipients(recipients);
                self.form("Information", information);
                if let Some(k) = kind {
                    self.counter(k);
                }
            }
            Expr::Constant(_) | Expr::AnnouncedX | Expr::EventAmount | Expr::EventActivationX => {}
        }
    }
    fn count(&mut self, c: &Count) {
        self.form("Count", c);
        match c {
            Count::Exactly(n) | Count::UpTo(n) | Count::AtLeast(n) | Count::AsManyAsCan(n) => {
                self.expr(n)
            }
            Count::AnyNumber => {}
            Count::Between { minimum, maximum } => {
                self.expr(minimum);
                self.expr(maximum);
            }
            Count::PerPlayer { players, each } => {
                self.players(players);
                self.count(each);
            }
        }
    }
    fn filter(&mut self, f: &ObjectFilter) {
        self.form("ObjectFilter", f);
        match f {
            ObjectFilter::TargetCount(_, value) => self.expr(value),
            ObjectFilter::Noted(_)
            | ObjectFilter::NthChosen(_)
            | ObjectFilter::MostCommonCreatureTypeIn(_)
            | ObjectFilter::DraftNoted(_)
            | ObjectFilter::InPile(_)
            | ObjectFilter::InSector(_) => {}
            ObjectFilter::ChosenBy { player, .. } => self.player(player),
            ObjectFilter::ChosenOn { object, .. }
            | ObjectFilter::AttackingSame(object)
            | ObjectFilter::SharesTypeFrom { with: object, .. } => self.object(object),
            ObjectFilter::CastFromAmong(objects) | ObjectFilter::SharesMostCommonColor(objects) => {
                self.objects(objects)
            }
            ObjectFilter::ArrivedThisTurn { .. } | ObjectFilter::ChosenFilter => {}
            ObjectFilter::SharesColorWith(object)
            | ObjectFilter::ExiledWith(object)
            | ObjectFilter::MadeBy(object)
            | ObjectFilter::WasAttachedTo(object) => self.object(object),
            ObjectFilter::DamagedBy(_) | ObjectFilter::DealtExcessBy(_) => {}
            ObjectFilter::Primitive { name, .. } => self.primitive(name),
            ObjectFilter::History { event, window } => {
                match event {
                    ObjectHistory::AttackedDefender(filter) | ObjectHistory::Enlisted(filter) => {
                        self.filter(filter)
                    }
                    ObjectHistory::Drawn(Some(players))
                    | ObjectHistory::ExiledFromGraveyard(players) => self.players(players),
                    ObjectHistory::DamagedBy(object)
                    | ObjectHistory::Convoked(object)
                    | ObjectHistory::Exploited(object)
                    | ObjectHistory::TappedToPayFor(object) => self.object(object),
                    _ => {}
                }
                match window {
                    ObjectWindow::LastTurnOf(player)
                    | ObjectWindow::SinceLastUpkeepOf(player)
                    | ObjectWindow::SinceLastTurnOf(player)
                    | ObjectWindow::LastCombatOf(player) => self.player(player),
                    _ => {}
                }
            }
            ObjectFilter::ManaSpent {
                sources,
                payers,
                value,
                ..
            } => {
                if let Some(sources) = sources {
                    self.filter(sources);
                }
                if let Some(payers) = payers {
                    self.players(payers);
                }
                self.expr(value);
            }
            ObjectFilter::AlternateCostPaid(_)
            | ObjectFilter::ManaCostIs(_)
            | ObjectFilter::ManaSymbolInCost(_)
            | ObjectFilter::NameFirstPrintedIn(_)
            | ObjectFilter::InResult(_)
            | ObjectFilter::CouldProduce(_) => {}
            ObjectFilter::BlockingAny(filter)
            | ObjectFilter::BlockedByAny(filter)
            | ObjectFilter::PairedWith(filter)
            | ObjectFilter::CanTargetOnly(filter)
            | ObjectFilter::WouldDestroy(filter) => self.filter(filter),
            ObjectFilter::BandedWith(object)
            | ObjectFilter::TargetableBy(object)
            | ObjectFilter::CouldProduceManaOf(object) => self.object(object),
            ObjectFilter::AttachedCount {
                attachments, value, ..
            } => {
                self.filter(attachments);
                self.expr(value);
            }
            ObjectFilter::ProtectedBy(players) | ObjectFilter::InStartingDeck(players) => {
                self.players(players)
            }
            ObjectFilter::CouldEnchant(objects) => self.objects(objects),
            ObjectFilter::ExiledBy { player, with, .. } => {
                self.player(player);
                if let Some(object) = with {
                    self.object(object);
                }
            }
            ObjectFilter::AttachedTo(filter) => self.filter(filter),
            ObjectFilter::BlockingObject(object) => self.object(object),
            ObjectFilter::Condition(condition) => self.condition(condition),
            ObjectFilter::ManaSpentFrom(sources) => self.filter(sources),
            ObjectFilter::TargetedBy(filter) => self.filter(filter),
            ObjectFilter::ColorOutsideCommanderIdentity(player) => self.player(player),
            ObjectFilter::SharesColorWithMana(mana) | ObjectFilter::ManaCostPayableWith(mana) => {
                self.form("ManaOccurrence", mana)
            }
            ObjectFilter::KickedWith(_) => {}
            ObjectFilter::HasAttached(filter) => self.filter(filter),
            ObjectFilter::AttachedToPlayer(player) => self.player(player),
            ObjectFilter::Extreme { among, .. } => self.objects(among),
            ObjectFilter::NthCast {
                ordinal,
                spells,
                by,
                ..
            } => {
                self.expr(ordinal);
                self.filter(spells);
                if let Some(by) = by {
                    self.players(by);
                }
            }
            ObjectFilter::Cast { by, from } => {
                if let Some(players) = by {
                    self.players(players);
                }
                if let Some(scope) = from {
                    self.zone_scope(scope);
                }
            }
            ObjectFilter::EnteredFrom(scope) => self.zone_scope(scope),
            ObjectFilter::HasKeywordFamily(family) => self.form("KeywordFamily", family),
            ObjectFilter::TargetsAny(targets) => self.recipients(targets),
            ObjectFilter::OnStack(stack) => self.form("StackTrait", stack),
            ObjectFilter::AbilityOf(sources) => self.filter(sources),
            ObjectFilter::HasActivatedAbility(filter) => self.activation_filter(filter),
            ObjectFilter::InSet(objects) | ObjectFilter::SharesColorWithAny(objects) => {
                self.objects(objects)
            }
            ObjectFilter::InZone(scope) => self.zone_scope(scope),
            ObjectFilter::HasKeyword(k) => self.keyword(k),
            ObjectFilter::Supertype(ty) => self.form("SuperType", ty),
            ObjectFilter::All(items) | ObjectFilter::Either(items) => {
                for f in items {
                    self.filter(f);
                }
            }
            ObjectFilter::Not(f) => self.filter(f),
            ObjectFilter::ControlledBy(p) | ObjectFilter::OwnedBy(p) => self.players(p),
            ObjectFilter::SameObject(o)
            | ObjectFilter::SharesNameWith(o)
            | ObjectFilter::Shares { with: o, .. } => self.object(o),
            ObjectFilter::SharesNameWithAny(objects)
            | ObjectFilter::SharesNameWithAnother(objects)
            | ObjectFilter::SharesWithAny { with: objects, .. } => self.objects(objects),
            ObjectFilter::Property {
                property,
                comparison,
                value,
            } => {
                self.form("Property", property);
                self.form("Comparison", comparison);
                self.expr(value);
            }
            ObjectFilter::Counters {
                kind,
                comparison,
                value,
            } => {
                if let Some(k) = kind {
                    self.counter(k);
                }
                self.form("Comparison", comparison);
                self.expr(value);
            }
            ObjectFilter::BlockedByObject(object) => self.object(object),
            ObjectFilter::AttackingDefender(defenders) => self.defenders(defenders),
            ObjectFilter::Any
            | ObjectFilter::IsCard
            | ObjectFilter::Name(_)
            | ObjectFilter::CardType(_)
            | ObjectFilter::Subtype(_)
            | ObjectFilter::Color(_)
            | ObjectFilter::Colorless
            | ObjectFilter::Token
            | ObjectFilter::Tapped
            | ObjectFilter::Attacking
            | ObjectFilter::Blocking
            | ObjectFilter::State(_)
            | ObjectFilter::Chosen(_)
            | ObjectFilter::Modified => {}
        }
    }
    fn condition(&mut self, c: &Condition) {
        self.form("Condition", c);
        match c {
            Condition::IsPrime(value) => self.expr(value),
            Condition::ChoseNth { .. }
            | Condition::WonSubgame { .. }
            | Condition::CompletedDungeon { .. }
            | Condition::NotedForDraft { .. } => {}
            Condition::TargetCount { filter, value, .. } => {
                self.filter(filter);
                self.expr(value);
            }
            Condition::AsCast { player, action } => {
                self.player(player);
                match action {
                    AsCastAction::Revealed(filter) => self.filter(filter),
                    AsCastAction::Controlled(objects) | AsCastAction::Chose(objects) => {
                        self.objects(objects)
                    }
                }
            }
            Condition::PlayerIn { player, players } => {
                self.player(player);
                self.players(players);
            }
            Condition::ValueChosenIs(player) | Condition::ChoseExtreme { player, .. } => {
                self.player(player)
            }
            Condition::HadExtreme { player, filter, .. } => {
                self.player(player);
                if let Some(filter) = filter {
                    self.filter(filter);
                }
            }
            Condition::VotedAs { player, other, .. } => {
                self.player(player);
                self.player(other);
            }
            Condition::CoinCameUp { player, .. } => {
                if let Some(player) = player {
                    self.player(player);
                }
            }
            Condition::Outcome(outcome) => self.form("PriorOutcome", outcome),
            Condition::AllVotedFor(_) => {}
            Condition::MostCommonColor { objects, .. } => self.objects(objects),
            Condition::GraveyardPosition {
                card,
                player,
                above,
                amount,
                ..
            } => {
                self.object(card);
                self.player(player);
                self.filter(above);
                self.expr(amount);
            }
            Condition::PlayerDid {
                player,
                event,
                window,
            } => {
                self.player(player);
                match event {
                    PlayerHistory::AttackedWithTotalPower(_, value)
                    | PlayerHistory::PaidOrLostEnergy(_, value)
                    | PlayerHistory::RolledDie(_, value) => self.expr(value),
                    PlayerHistory::AttackedBy(filter)
                    | PlayerHistory::AttackedWith(filter)
                    | PlayerHistory::AttackedDefender(filter)
                    | PlayerHistory::CastSpell(filter)
                    | PlayerHistory::PlayedFromExile(filter)
                    | PlayerHistory::PermanentEntered(filter) => self.filter(filter),
                    PlayerHistory::AttackedPlayer(other) => self.player(other),
                    PlayerHistory::Cycled(_, value, filter) => {
                        self.expr(value);
                        self.filter(filter);
                    }
                    _ => {}
                }
                match window {
                    ObjectWindow::LastTurnOf(player)
                    | ObjectWindow::SinceLastUpkeepOf(player)
                    | ObjectWindow::SinceLastTurnOf(player)
                    | ObjectWindow::LastCombatOf(player) => self.player(player),
                    _ => {}
                }
            }
            Condition::ValueChosen(_) => {}
            Condition::Primitive { name, .. } => self.primitive(name),
            Condition::SubsetMeets {
                objects, amount, ..
            } => {
                self.objects(objects);
                self.expr(amount);
            }
            Condition::Window(window) => self.form("TurnWindow", window),
            Condition::ItsNeitherDayNorNight | Condition::ItsNight => {}
            Condition::MostVotes { option, .. } | Condition::Voted(option) => {
                self.vote_option(option)
            }
            Condition::PlayerAttacks { attacker, defender } => {
                self.player(attacker);
                self.player(defender);
            }
            Condition::SameTeam { player, other } => {
                self.player(player);
                self.player(other);
            }
            Condition::VotedFor { player, option } => {
                self.player(player);
                self.vote_option(option);
            }
            Condition::ChosenWord(_) | Condition::GiftPromised(_) => {}
            Condition::PlayerState { player, state } => {
                self.form("PlayerState", state);
                self.player(player);
            }
            Condition::Searched(_) => {}
            Condition::PlayerTook { player, .. } => self.player(player),
            Condition::TargetsMatch {
                object,
                kind,
                filter,
            } => {
                self.object(object);
                self.form("ObjectKind", kind);
                self.filter(filter);
            }
            Condition::WasCastFrom { object, zone } => {
                self.object(object);
                self.form("EventZone", zone);
            }
            Condition::ExistsPlayer {
                players, condition, ..
            } => {
                self.players(players);
                self.condition(condition);
            }
            Condition::PaymentChosen(_) => {}
            Condition::PlayerIsActive(p) => self.player(p),
            Condition::All(items) | Condition::Either(items) => {
                for c in items {
                    self.condition(c);
                }
            }
            Condition::Not(c) => self.condition(c),
            Condition::ObjectMatches { object, filter } => {
                self.object(object);
                self.filter(filter);
            }
            Condition::Controls {
                player,
                filter,
                minimum: _,
            } => {
                self.player(player);
                self.filter(filter);
            }
            Condition::Compare {
                left,
                comparison,
                right,
            } => {
                self.expr(left);
                self.form("Comparison", comparison);
                self.expr(right);
            }
            Condition::CoinFlip { outcome, .. } => self.form("CoinOutcome", outcome),
        }
    }
    fn duration(&mut self, d: &Duration) {
        self.form("Duration", d);
        match d {
            Duration::UntilEvent(event) => self.event(event),
            Duration::While(condition) => self.condition(condition),
            Duration::WhileItMatches(filter) => self.filter(filter),
            Duration::ThroughNextTurn
            | Duration::ExtraCombat(_)
            | Duration::UntilStateBasedActionsChecked
            | Duration::DuringControllersNextUntapSteps(_) => {}
            Duration::EndOfCombatOnNextTurn(player)
            | Duration::UntilNextUntapStep(player)
            | Duration::UntilLibraryTopChanges(player)
            | Duration::WhilePlayerHasCounter { player, .. } => self.player(player),
            Duration::UntilAnotherExiled { by, with } => {
                self.players(by);
                self.object(with);
            }
            Duration::Primitive { name, .. } => self.primitive(name),
            Duration::ExtraTurn(_) => {}
            Duration::EndOfTurn
            | Duration::EndOfCombat
            | Duration::Indefinite
            | Duration::DuringControllersNextUntapStep => {}
            Duration::UntilNextTurn(p)
            | Duration::UntilEndOfNextTurn(p)
            | Duration::DuringNextUntapStep(p)
            | Duration::DuringNextTurn(p) => self.player(p),
            Duration::WhileOnBattlefield(object) | Duration::WhileExiled(object) => {
                self.object(object)
            }
            Duration::UntilPaid { player, cost } => {
                self.player(player);
                self.cost(cost);
            }
            Duration::WhileAnyExiled(objects) => self.objects(objects),
            Duration::UntilCast(object) => self.object(object),
            Duration::WhileControlled { player, object } => {
                self.player(player);
                self.object(object);
            }
            Duration::WhileMatches { object, filter } => {
                self.object(object);
                self.filter(filter);
            }
            Duration::UntilNext { point, players } => {
                self.form("TurnPoint", point);
                self.players(players);
            }
            Duration::Either(durations) => durations.iter().for_each(|d| self.duration(d)),
        }
    }
    fn entry_states(&mut self, entry: &EntrySpec) {
        for state in &entry.states {
            if let EntryState::Primitive { name, .. } = state {
                self.primitive(name);
            }
        }
    }

    fn replacement_event(&mut self, event: &ReplacementEvent) {
        self.form("ReplacementEvent", event);
        match event {
            ReplacementEvent::Enter { objects } => self.filter(objects),
            ReplacementEvent::Discard { players, .. } => self.players(players),
            ReplacementEvent::Damage(damage) => self.event(&EventSpec::Damage(damage.clone())),
            ReplacementEvent::ZoneChange { objects, .. } => self.filter(objects),
            ReplacementEvent::CopySpell { players, spells } => {
                self.players(players);
                self.filter(spells);
            }
            ReplacementEvent::Countered { spells, by } => {
                self.filter(spells);
                self.filter(by);
            }
            ReplacementEvent::DrawCausedBy { players, abilities } => {
                self.players(players);
                self.filter(abilities);
            }
            ReplacementEvent::ProduceColor { sources, .. } => self.filter(sources),
            ReplacementEvent::Primitive { name, .. } => self.primitive(name),
            ReplacementEvent::Player { event, players } => {
                self.form("ReplacedPlayerEvent", event);
                self.players(players);
            }
            ReplacementEvent::Object { event, objects } => {
                self.form("ReplacedObjectEvent", event);
                self.filter(objects);
            }
            ReplacementEvent::Tokens {
                tokens,
                controllers,
                creators,
                ..
            } => {
                self.filter(tokens);
                for players in controllers.iter().chain(creators) {
                    self.players(players);
                }
            }
            ReplacementEvent::Counters {
                objects,
                players,
                by,
                ..
            } => {
                self.filter(objects);
                for players in players.iter().chain(by) {
                    self.players(players);
                }
            }
            ReplacementEvent::ProduceMana { sources, by } => {
                self.filter(sources);
                if let Some(player) = by {
                    self.player(player);
                }
            }
            ReplacementEvent::Caused { cause, event } => {
                self.filter(cause);
                self.replacement_event(event);
            }
            ReplacementEvent::PutOntoBattlefield { players, objects } => {
                self.players(players);
                self.filter(objects);
            }
            ReplacementEvent::Measured { event, amount, .. } => {
                self.replacement_event(event);
                self.expr(amount);
            }
            ReplacementEvent::FirstEachTurn(event) | ReplacementEvent::EachPoint(event) => {
                self.replacement_event(event)
            }
            ReplacementEvent::Either(events) => {
                for event in events {
                    self.replacement_event(event);
                }
            }
        }
    }
    fn event(&mut self, e: &EventSpec) {
        self.form("EventSpec", e);
        match e {
            EventSpec::Primitive { name, .. } => self.primitive(name),
            EventSpec::ClassLevel { objects, level } => {
                self.filter(objects);
                self.expr(level);
            }
            EventSpec::FirstEachGame(event)
            | EventSpec::FirstEachTurn(event)
            | EventSpec::FirstForObjectEachTurn(event) => self.event(event),
            EventSpec::Caused { cause, event } => {
                self.filter(cause);
                self.event(event);
            }
            EventSpec::ZoneChangeFromElsewhere { objects, .. } => self.filter(objects),
            EventSpec::AttackUnblocked { attackers, .. } => self.filter(attackers),
            EventSpec::AllAttack(objects)
            | EventSpec::OntoStack(objects)
            | EventSpec::Resolves(objects)
            | EventSpec::ExploresCard { objects, .. }
            | EventSpec::TappedForManaOf { objects, .. }
            | EventSpec::TargetsChosen { objects, .. }
            | EventSpec::ControlChanges { objects, .. } => self.filter(objects),
            EventSpec::ManaAdded { abilities, .. } => self.filter(abilities),
            EventSpec::CountersOnPlayer { players, .. } => self.players(players),
            EventSpec::CausesTrigger { cause, .. } => self.event(cause),
            EventSpec::AttacksWith { others, .. } => self.filter(others),
            EventSpec::AttacksAsGroup { attackers, .. } => self.filter(attackers),
            EventSpec::SpellTargeting { spells, .. }
            | EventSpec::SpellOrActivation { spells, .. } => self.filter(spells),
            EventSpec::PaysToActivate { abilities, .. } => self.filter(abilities),
            EventSpec::ObjectBecomes(objects) => self.filter(objects),
            EventSpec::NthCounter { objects, nth, .. } => {
                self.filter(objects);
                self.expr(nth);
            }
            EventSpec::Player {
                event,
                players,
                objects,
            } => {
                self.form("PlayerEvent", event);
                self.players(players);
                if let Some(filter) = objects {
                    self.filter(filter);
                }
            }
            EventSpec::NthEachTurn { event, value, .. } => {
                self.event(event);
                self.expr(value);
            }
            EventSpec::Pair {
                event,
                objects,
                others,
            } => {
                self.form("PairEvent", event);
                self.filter(objects);
                self.filter(others);
            }
            EventSpec::BecomesTarget { objects, kind, .. } => {
                self.form("TargetingKind", kind);
                self.filter(objects);
            }
            EventSpec::Attack {
                attackers,
                by,
                defenders,
            } => {
                self.filter(attackers);
                if let Some(by) = by {
                    self.players(by);
                }
                self.defenders(defenders);
            }
            EventSpec::Batch {
                event,
                per,
                count,
                including,
            } => {
                self.form("BatchPer", per);
                self.event(event);
                if let Some((_, value)) = count {
                    self.expr(value);
                }
                if let Some(object) = including {
                    self.object(object);
                }
            }
            EventSpec::Counters {
                change,
                objects,
                by,
                ..
            } => {
                self.form("CounterChange", change);
                self.filter(objects);
                if let Some(by) = by {
                    self.players(by);
                }
            }
            EventSpec::Activated { players, abilities } => {
                self.players(players);
                self.filter(abilities);
            }
            EventSpec::SagaChapter(_) => {}
            EventSpec::Discard {
                players,
                cards,
                kind,
                grouping,
                ..
            } => {
                self.players(players);
                self.filter(cards);
                self.form("DiscardKind", kind);
                self.form("DiscardGrouping", grouping);
            }
            EventSpec::ZoneChange { objects, from, to } => {
                self.filter(objects);
                if let Some(from) = from {
                    self.form("EventZone", from);
                }
                self.form("EventZone", to);
            }
            EventSpec::Object { event, objects } => {
                self.form("ObjectEvent", event);
                self.filter(objects);
            }
            EventSpec::Spell {
                event,
                players,
                spells,
            } => {
                self.form("SpellEvent", event);
                self.players(players);
                self.filter(spells);
            }
            EventSpec::Turn { point, players } => {
                self.form("TurnPoint", point);
                self.players(players);
            }
            EventSpec::Damage(event) => {
                self.form("ObjectKind", &event.source_kind);
                self.filter(&event.sources);
                self.form("DamageKind", &event.kind);
                self.form("DamageGrouping", &event.grouping);
                self.form("DamageSubject", &event.subject);
                self.form("DamageRecipients", &event.recipients);
                match &event.recipients {
                    DamageRecipients::Any
                    | DamageRecipients::Target { .. }
                    | DamageRecipients::Targets(_) => {}
                    DamageRecipients::Objects(f) => self.filter(f),
                    DamageRecipients::Players(p) => self.players(p),
                    DamageRecipients::ObjectsAndPlayers { objects, players } => {
                        self.filter(objects);
                        self.players(players);
                    }
                }
            }
            EventSpec::State { condition } => self.condition(condition),
            EventSpec::Either(events) => {
                for event in events {
                    self.event(event);
                }
            }
            EventSpec::Filtered { event, condition } => {
                self.event(event);
                self.condition(condition);
            }
        }
    }
}
