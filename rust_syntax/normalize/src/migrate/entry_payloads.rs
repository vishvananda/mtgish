//! Explicit origins for the entry-payload migration. Shapes are checked with
//! the archived input descriptors before these bottom-up rewrites run.
use super::*;

pub(super) struct Region {
    pub source: String,
    pub moves: Vec<(String, String)>,
}
fn region(source: &str, moves: Vec<(String, String)>) -> Region {
    Region {
        source: source.into(),
        moves,
    }
}
fn operand(value: &Value) -> Result<Operand, String> {
    serde_json::from_value(value.clone()).map_err(|e| e.to_string())
}
fn child(path: &str, index: usize) -> String {
    format!("{path}/args/operands/{index}")
}
fn term(value: &Operand) -> Option<(&str, &str, &[Operand])> {
    match value {
        Operand::Term {
            family,
            form,
            operands,
        } => Some((family, form, operands)),
        Operand::Vocabulary { family, form } => Some((family, form, &[])),
        _ => None,
    }
}
pub(super) fn structure(name: &str, value: &mut Value, path: &str) -> Result<Vec<Region>, String> {
    if name != "FaceDownAs" {
        return Ok(vec![]);
    }
    let pt = operand(&value["power_toughness"])?;
    let kind = (!value["creature_type"].is_null())
        .then(|| operand(&value["creature_type"]))
        .transpose()?;
    let shape = crate::source_values::face_down(
        &pt,
        kind.as_ref(),
        value["artifact"]
            .as_bool()
            .ok_or("face-down artifact flag requires Boolean")?,
    )?;
    let size =
        crate::source_values::power_toughness(&pt, &format!("{path}/power_toughness"), path)?;
    *value = serde_json::to_value(shape).map_err(|e| e.to_string())?;
    Ok(vec![region(path, size.moves)])
}
pub(super) fn variant(owner: &str, value: &mut Value, path: &str) -> Result<Vec<Region>, String> {
    let mut regions = vec![];
    match owner {
        "EntryState::FaceDownWith" => {
            let at = format!("{path}/FaceDownWith/abilities");
            let input = value["FaceDownWith"]["abilities"]
                .as_array()
                .ok_or("face-down abilities require a list")?;
            let mut abilities = vec![];
            let mut moves = vec![];
            for (i, a) in input.iter().enumerate() {
                let Operand::Ability(ability) = operand(a)? else {
                    return Err("face-down granted rule requires a typed ability".into());
                };
                abilities.push(*ability);
                moves.push((format!("{at}/{i}/args"), format!("{at}/{i}")));
            }
            value["FaceDownWith"]["abilities"] =
                serde_json::to_value(abilities).map_err(|e| e.to_string())?;
            regions.push(region(&at, moves));
        }
        "EntryState::FaceDownLand" => {
            let land: crate::mtg_types::LandType =
                crate::source_values::word(&operand(&value["FaceDownLand"])?, "LandType")?;
            value["FaceDownLand"] = serde_json::to_value(land).map_err(|e| e.to_string())?;
            regions.push(region(&format!("{path}/FaceDownLand"), vec![]));
        }
        "EntryState::ExiledForWith" => {
            let at = format!("{path}/ExiledForWith/body");
            let old = &value["ExiledForWith"]["body"];
            if old["kind"] != "Program" {
                return Err("return-entry trigger requires a program".into());
            }
            value["ExiledForWith"]["body"] = old["args"].clone();
            regions.push(region(&at, vec![(format!("{at}/args"), at.clone())]));
        }
        "EntryState::May" => {
            let flags: Vec<Operand> =
                serde_json::from_value(value["May"].clone()).map_err(|e| e.to_string())?;
            let entry = crate::movement::entry_from_source(&flags);
            let moves = flag_origins(
                &flags,
                &format!("{path}/May"),
                &format!("{path}/Optional/entry"),
                0,
            )?;
            *value = json!({"Optional":{"decider":{"kind":"AbilityController"},"entry":entry}});
            regions.push(region(path, moves));
        }
        "EntryState::RulesUntil" => {
            let args = &value["RulesUntil"];
            let rules: Vec<Operand> =
                serde_json::from_value(args["rules"].clone()).map_err(|e| e.to_string())?;
            let changes = crate::source_values::rules(&rules)?;
            let mut moves = vec![(
                format!("{path}/RulesUntil/until"),
                format!("{path}/ChangesUntil/duration"),
            )];
            moves.extend(rule_origins(
                &rules,
                &format!("{path}/RulesUntil/rules"),
                &format!("{path}/ChangesUntil/changes"),
            ));
            *value = json!({"ChangesUntil":{"changes":changes,"duration":args["until"]}});
            regions.push(region(path, moves));
        }
        "EntryState::Primitive" => {
            *value = json!({"Draft":value["Primitive"]});
            regions.push(region(
                path,
                vec![(format!("{path}/Primitive"), format!("{path}/Draft"))],
            ));
        }
        _ => {}
    }
    Ok(regions)
}

fn rule_origins(rules: &[Operand], source: &str, target: &str) -> Vec<(String, String)> {
    let mut moves = vec![];
    for (i, rule) in rules.iter().enumerate() {
        let Some(("PermanentRule", form, args)) = term(rule) else {
            continue;
        };
        if let ("MustAttackPlayer", [Operand::Player(_)]) = (form, args) {
            moves.push((
                format!("{}/args", child(&format!("{source}/{i}"), 0)),
                format!("{target}/{i}/args/RequireCombat/action/Attack/Players/args"),
            ));
        }
    }
    moves
}

/// The source signature of MayActions is Vec<EnterFlag>. Preserve the origins
/// of each reviewed typed subtree, with explicit offsets for accumulated entry
/// states/counters. Unknown source material is never guessed by matching IDs;
/// final map coverage refuses a rewrite whose origins are not explained.
fn flag_origins(
    flags: &[Operand],
    source: &str,
    target: &str,
    depth: usize,
) -> Result<Vec<(String, String)>, String> {
    if depth > 128 {
        return Err("optional-entry origin nesting limit exceeded".into());
    }
    let (mut moves, mut states, mut counters) = (vec![], 0, 0);
    for (i, flag) in flags.iter().enumerate() {
        let s = format!("{source}/{i}");
        let Some(("EnterFlag", form, args)) = term(flag) else {
            return Err("optional entry requires EnterFlag source forms".into());
        };
        let entry = crate::movement::entry_from_source(std::slice::from_ref(flag));
        // Controller flags overwrite the earlier override. Its discarded
        // reference has no destination; it must never bind to the later one.
        if entry.controller.is_some() {
            let controller = format!("{target}/controller");
            moves.retain(|(_, to): &(String, String)| {
                to != &controller && !to.starts_with(&format!("{controller}/"))
            });
        }
        let at = format!("{target}/states/{states}");
        let input = |i| format!("{}/args", child(&s, i));
        match (form, args) {
            ("EntersUnderPlayersControl", [Operand::Player(_)]) => {
                moves.push((input(0), format!("{target}/controller/Player")))
            }
            ("EntersUnderAPlayersControl", [Operand::Players(_)]) => {
                moves.push((input(0), format!("{target}/controller/Player/args")))
            }
            ("EntersAttachedToPermanent", [Operand::Object(_)]) => {
                moves.push((input(0), format!("{at}/AttachedTo")))
            }
            ("EntersAttachedToAPermanent", [Operand::Objects(_)]) => {
                moves.push((input(0), format!("{at}/AttachedToChosen/args")))
            }
            ("EntersAttachedToPlayer", [Operand::Player(_)]) => {
                moves.push((input(0), format!("{at}/AttachedToPlayer")))
            }
            (
                "EntersAttackingPlayer" | "EntersAttackingPlayerOrPlaneswalkerControlledBy",
                [Operand::Player(_)],
            ) => moves.push((input(0), format!("{at}/Attacking/player"))),
            ("EntersBlockingAttacker", [Operand::Object(_)]) => {
                moves.push((input(0), format!("{at}/Blocking")))
            }
            ("EntersWithNumberCounters", [Operand::Number(_), _]) => {
                moves.push((input(0), format!("{target}/counters/{counters}/1")))
            }
            ("EntersAsFaceDownArtifactCreature", [pt, _]) => moves.extend(
                crate::source_values::power_toughness(
                    pt,
                    &child(&s, 0),
                    &format!("{at}/FaceDownAs"),
                )?
                .moves,
            ),
            (
                "EntersAsFaceDownCreatureWithAbilitiesAndNotedName",
                [pt, Operand::List(abilities), _],
            ) => {
                moves.extend(
                    crate::source_values::power_toughness(
                        pt,
                        &child(&s, 0),
                        &format!("{at}/FaceDownWith/face_down"),
                    )?
                    .moves,
                );
                for j in 0..abilities.len() {
                    moves.push((
                        format!("{}/args/{j}/args", child(&s, 1)),
                        format!("{at}/FaceDownWith/abilities/{j}"),
                    ));
                }
                moves.push((
                    format!("{}/args", child(&child(&s, 2), 0)),
                    format!("{at}/FaceDownWith/noted"),
                ));
            }
            ("MayActions", nested) => moves.extend(flag_origins(
                nested,
                &format!("{s}/args/operands"),
                &format!("{at}/Optional/entry"),
                depth + 1,
            )?),
            ("EntersWithPerpetualEffect", effects) => {
                for (j, effect) in effects.iter().enumerate() {
                    if let Some((_, "AddAbility", [Operand::Ability(_)])) = term(effect) {
                        moves.push((
                            format!("{}/args", child(&child(&s, j), 0)),
                            format!("{at}/Perpetual/{j}/args"),
                        ));
                    }
                    if let Some((_, "SetPT", [pt])) = term(effect) {
                        moves.extend(
                            crate::source_values::power_toughness(
                                pt,
                                &child(&child(&s, j), 0),
                                &format!("{at}/Perpetual/{j}/args"),
                            )?
                            .moves,
                        );
                    }
                }
            }
            _ => {}
        }
        states += entry.states.len();
        counters += entry.counters.len();
    }
    Ok(moves)
}
