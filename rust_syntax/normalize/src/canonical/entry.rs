//! Entry matchers share a controller filter without changing reference timing.
use crate::ir_schema::{Fields, Schema, Ty, TypeDef};
use serde_json::{json, Value};

fn contains(filter: &mut Value, wanted: &Value, tracing: bool) -> bool {
    if super::trace::same(filter, wanted, tracing) {
        if tracing {
            super::trace::merge(filter, wanted);
        }
        return true;
    }
    filter["kind"] == "AllOf"
        && filter["args"]
            .as_array_mut()
            .is_some_and(|items| items.iter_mut().any(|item| contains(item, wanted, tracing)))
}

fn controlled(filter: &Value) -> bool {
    filter["kind"] == "ControlledBy"
        || filter["kind"] == "AllOf"
            && filter["args"]
                .as_array()
                .is_some_and(|items| items.iter().any(controlled))
}

/// The matching player's controller restriction can live in its object filter.
/// The event remains a player event: its recorded controller is never replaced
/// by a later ControllerOf(EventSubject) query.
pub(super) fn player(event: &mut Value, schema: &Schema, tracing: bool) {
    let Some(args) = event.get_mut("Player") else {
        return;
    };
    if args["event"] != "EntersUnderControl" || args["players"]["kind"] == "All" {
        return;
    }
    // Moving a selector into an object filter must not introduce a different
    // Candidate binding, or make an originally invalid Candidate read valid.
    if !safe(
        &args["players"],
        &Ty::Named("PlayerSet".into()),
        schema,
        Guard::Actor,
    ) {
        return;
    }
    let players = args["players"].take();
    let restriction = json!({"kind":"ControlledBy","args":players});
    if args["objects"].is_null() {
        args["objects"] = restriction;
    } else if !contains(&mut args["objects"], &restriction, tracing) {
        args["objects"] = json!({"kind":"AllOf","args":[args["objects"].take(), restriction]});
    }
    args["players"] = json!({"kind":"All"});
}

/// Two capabilities differ: player entry records a player but exposes no
/// EventDestination, whereas ZoneChange exposes a destination but no player.
/// Only a frame that observes neither capability can change matcher family.
/// Nested abilities and future trigger bodies establish independent frames.
#[derive(Clone, Copy)]
enum Guard {
    Frame,
    Actor,
}

fn safe(value: &Value, ty: &Ty, schema: &Schema, guard: Guard) -> bool {
    match ty {
        Ty::Opaque => true,
        Ty::Option(inner) => value.is_null() || safe(value, inner, schema, guard),
        Ty::Vec(inner) => value
            .as_array()
            .is_none_or(|items| items.iter().all(|item| safe(item, inner, schema, guard))),
        Ty::Tuple(types) => value.as_array().is_none_or(|items| {
            items
                .iter()
                .zip(types)
                .all(|(v, t)| safe(v, t, schema, guard))
        }),
        Ty::Named(name) => {
            if name == "Ability" {
                return true;
            }
            let blocked = match guard {
                Guard::Frame => {
                    (name == "PlayerRef" && value["kind"] == "EventSubject")
                        || (name == "ObjectRef" && value["kind"] == "EventDestination")
                }
                Guard::Actor => name == "ObjectRef" && value["kind"] == "Candidate",
            };
            if blocked {
                return false;
            }
            match schema.types.get(name) {
                Some(TypeDef::Struct(types)) => fields(value, types, schema, false, guard),
                Some(TypeDef::Enum { adjacent, .. }) => {
                    let variant = if *adjacent {
                        value["kind"].as_str()
                    } else {
                        value
                            .as_object()
                            .and_then(|map| map.keys().next().map(String::as_str))
                    };
                    let Some((_, types)) = variant.and_then(|v| schema.variant(name, v)) else {
                        return true;
                    };
                    let content = if *adjacent {
                        &value["args"]
                    } else {
                        &value[variant.unwrap()]
                    };
                    fields(
                        content,
                        types,
                        schema,
                        name == "Operation" && variant == Some("CreateTrigger")
                            || matches!(name.as_str(), "EntryState" | "ExileState")
                                && variant == Some("ExiledForWith"),
                        guard,
                    )
                }
                None => true,
            }
        }
    }
}

fn fields(value: &Value, types: &Fields, schema: &Schema, future: bool, guard: Guard) -> bool {
    match types {
        Fields::Unit => true,
        Fields::Tuple(types) => {
            if let [ty] = types.as_slice() {
                safe(value, ty, schema, guard)
            } else {
                value.as_array().is_none_or(|items| {
                    items
                        .iter()
                        .zip(types)
                        .all(|(v, t)| safe(v, t, schema, guard))
                })
            }
        }
        Fields::Named(types) => types.iter().all(|(field, ty)| {
            future
                && matches!(
                    field.as_str(),
                    "body" | "announcement" | "intervening_if" | "objects"
                )
                || value
                    .get(field)
                    .is_none_or(|item| safe(item, ty, schema, guard))
        }),
    }
}

fn zone(event: &mut Value) {
    if let Some(args) = event.get("ZoneChange") {
        if args["from"].is_null() && args["to"] == "Battlefield" && controlled(&args["objects"]) {
            *event = json!({"Player":{"event":"EntersUnderControl",
                "players":{"kind":"All"},"objects":args["objects"].clone()}});
        }
        return;
    }
    if let Some(items) = event.get_mut("AnyOf").and_then(Value::as_array_mut) {
        items.iter_mut().for_each(zone);
    }
    // These wrappers do not alter the wrapped event's object/player roles.
    for wrapper in [
        "Filtered",
        "Batch",
        "NthEachTurn",
        "Caused",
        "WhileActivating",
    ] {
        if let Some(inner) = event
            .get_mut(wrapper)
            .and_then(|args| args.get_mut("event"))
        {
            zone(inner);
        }
    }
    for wrapper in ["FirstEachTurn", "FirstEachGame", "FirstForObjectEachTurn"] {
        if let Some(inner) = event.get_mut(wrapper) {
            zone(inner);
        }
    }
}

/// Called on a real owning frame, before its header and body are visited.
pub(super) fn header(args: &mut Value, types: &Fields, schema: &Schema) {
    if fields(args, types, schema, false, Guard::Frame) {
        if let Some(event) = args.get_mut("event") {
            zone(event);
        }
    }
}
