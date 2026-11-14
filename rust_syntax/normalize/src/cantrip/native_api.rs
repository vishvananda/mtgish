//! Versioned native interfaces; no changes to the IR or engine quantity limits.
use super::*;

/// A later action on the drawn cards uses the original instruction's receipt (CR 121.6c), not a hand diff or all
/// nested draw events. Keep that capability explicit for native targets that do not expose the receipt yet.
pub(super) fn draw_receipt(player: &str, count: &str) -> String {
    planned::planned_value(
        "original-draw-card-receipts",
        &format!("{{ g.draw({player}, {count}); Vec::<ObjectId>::new() }}"),
        &format!("g.draw_with_receipt({player}, {count})"),
    )
}

pub(super) fn permanent_cost_fields() -> &'static str {
    if planned::switched_on("permanent-cost-exclusions") { ", excluded: None" } else { "" }
}
pub(super) fn alternative_fields() -> &'static str {
    if planned::switched_on("spell-alternative-kinds") { ", kind: WayKind::Other" } else { "" }
}
pub(super) fn exile_cast_fields(any_color: bool) -> String {
    if planned::switched_on("exile-cast-fields") {
        format!(", from: ZoneKind::Exile, any_color: {any_color}")
    } else if any_color { ", any_color: true".into() } else { String::new() }
}
impl Compiler {
    /// Retain the two cardinalities: one departed permanent can yield two cards.
    pub(super) fn departure_views(&mut self, result: &ResultId, local: &str, out: &mut String) {
        if planned::switched_on("left-battlefield-receipts") {
            writeln!(out, "let done_permanent_count_{local} = i64::from({local}.count()); let {local} = {local}.into_objects();").unwrap();
            self.performed.insert(result.0, format!("done_permanent_count_{local}"));
        }
    }
}
