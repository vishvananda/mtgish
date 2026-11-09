//! Typed characteristic values at the source/legacy boundary. Consumers keep
//! explicit subtree origins; repeated compiler labels never identify a source.
use crate::ir::*;

type Result<T> = std::result::Result<T, String>;
pub(crate) struct Size {
    pub power: Expr,
    pub toughness: Expr,
    pub moves: Vec<(String, String)>,
}
fn term<'a>(value: &'a Operand, family: &str) -> Result<(String, &'a [Operand])> {
    match value {
        Operand::Term {
            family: actual,
            form,
            operands,
        } if actual == family => Ok((form.clone(), operands)),
        Operand::Vocabulary {
            family: actual,
            form,
        } if actual == family => Ok((form.clone(), &[])),
        _ => Err(format!(
            "characteristic requires {family} source vocabulary"
        )),
    }
}
fn child(path: &str, index: usize) -> String {
    format!("{path}/args/operands/{index}")
}
fn number(
    value: &Operand,
    source: String,
    target: String,
    moves: &mut Vec<(String, String)>,
) -> Result<Expr> {
    match value {
        Operand::Integer(n) => Ok(Expr::Constant(*n)),
        Operand::Number(n) => {
            moves.push((format!("{source}/args"), target));
            Ok(n.clone())
        }
        _ => Err("characteristic requires a numeric expression".into()),
    }
}
pub(crate) fn power_toughness(value: &Operand, source: &str, target: &str) -> Result<Size> {
    let (form, args) = term(value, "PT")?;
    let mut moves = vec![];
    let (power, toughness) = match (form.as_str(), args) {
        ("PT" | "ManualPT", [p, t]) => (
            number(p, child(source, 0), format!("{target}/power"), &mut moves)?,
            number(
                t,
                child(source, 1),
                format!("{target}/toughness"),
                &mut moves,
            )?,
        ),
        ("PTX", [p, t, x]) => {
            let mut component = |value: &Operand, index: usize, key: &str| -> Result<Expr> {
                let (form, args) = term(value, "PTXValue")?;
                match (form.as_str(), args) {
                    ("Integer", [n]) => number(
                        n,
                        child(&child(source, index), 0),
                        format!("{target}/{key}"),
                        &mut moves,
                    ),
                    ("X", []) => number(x, child(source, 2), format!("{target}/{key}"), &mut moves),
                    _ => Err("unsupported PTX component".into()),
                }
            };
            (component(p, 0, "power")?, component(t, 1, "toughness")?)
        }
        _ => return Err("unsupported power/toughness source shape".into()),
    };
    Ok(Size {
        power,
        toughness,
        moves,
    })
}
pub(crate) fn word<T: serde::de::DeserializeOwned>(value: &Operand, family: &str) -> Result<T> {
    let Operand::Vocabulary {
        family: actual,
        form,
    } = value
    else {
        return Err(format!("requires {family} vocabulary"));
    };
    if actual != family {
        return Err(format!("expected {family}, found {actual}"));
    }
    serde_json::from_value(serde_json::json!(form)).map_err(|e| e.to_string())
}
pub(crate) fn face_down(
    value: &Operand,
    creature_type: Option<&Operand>,
    artifact: bool,
) -> Result<FaceDownAs> {
    let size = power_toughness(value, "", "")?;
    Ok(FaceDownAs {
        power: size.power,
        toughness: size.toughness,
        creature_type: creature_type.map(|t| word(t, "CreatureType")).transpose()?,
        artifact,
    })
}

/// Permanent rules on an arrival use the ordinary typed rule vocabulary.
pub(crate) fn rules(values: &[Operand]) -> Result<Vec<Change>> {
    values
        .iter()
        .map(|value| {
            let Operand::Term {
                family,
                form,
                operands,
            } = value
            else {
                return Err("entry rule requires PermanentRule source form".into());
            };
            if family != "PermanentRule" {
                return Err("entry rule requires PermanentRule source family".into());
            }
            let rule = crate::semantics::reduce_rule(format!("{family}::{form}"), operands.clone());
            if matches!(rule, RuleChange::Primitive { .. }) {
                return Err(format!("unsupported typed entry rule {form}"));
            }
            Ok(Change::Rule(rule))
        })
        .collect()
}

pub(crate) fn requires_typed_payload(row: &str) -> bool {
    matches!(
        row.split_once("::"),
        Some((
            "Action",
            "CastCardInHandWithoutPayingAsAFacedownCreatureSpell"
                | "PutEachExiledCardOntoTheBattlefieldFaceDownAsAnArtifactCreature"
                | "PutEachGraveyardCardOntoBattlefieldFaceDownAsAnArtifactCreature"
                | "PutTheTopNumberCardsOfPlayersLibraryOntoBattlefieldAsFaceDownArtifactCreatures"
                | "PutAnyNumberOfCardsFromHandOntoBattlefieldAsFaceDownArtifactCreatures"
                | "PutEachCardOfTypeMilledThisWayOntoTheBattlefieldAsFaceDownArtifactCreatures"
                | "TurnPermanentFaceDownAsArtifactCreature"
                | "TurnEachPermanentFaceDownAsCreature"
        ))
    )
}
