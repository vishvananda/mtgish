//! Proposed trigger capture keeps Fired's existing roles and payload intact.
use super::*;

pub(super) const FEATURE: &str = "trigger-time secondary object capture";

impl Compiler {
    pub(super) fn capture_event_object(&mut self, event: &EventSpec, object: &ObjectRef) -> Result<triggers::Matched> {
        if self.history.is_some() {
            return Err(gap("secondary object capture in a historical matcher"));
        }
        let mut matched = self.event(event)?;
        if matched.capture_other.is_some() || matched.frame.other.is_some() {
            return Err(gap("overlapping secondary event object bindings"));
        }
        let capture = self.matching(matched.frame, |c| c.object(object))?;
        matched.capture_other = Some(capture);
        matched.frame.other = Some(Carried::CapturedObject);
        matched.planned.push(FEATURE.into());
        Ok(matched)
    }

    pub(super) fn captured_object_trigger(&mut self, matched: &triggers::Matched, label: &str,
                                          extra: &[String], capture: &str) -> String {
        let mut body = String::new();
        for arm in &matched.arms {
            let tests = if arm.tests.is_empty() { "true".into() } else { arm.tests.join(" && ") }.replace("cast_of(q, ", "cast_at(q, event, ");
            let guard = if extra.is_empty() { "true".into() } else { extra.join(" && ") };
            writeln!(body, "if let {} = event {{ {} let captured_other: Option<ObjectId> = {capture}; if ({tests}) && ({guard}) {{ return Some((Fired {{ subject, data }}, captured_other)); }} }}", arm.pattern, arm.locals).unwrap();
        }
        let mut matcher = format!("#[allow(unused_variables)] fn {label}(q: &dyn Query, asker: &Asker, event: &Event) -> Option<(Fired, Option<ObjectId>)> {{ {body} None }}");
        // All underlying requirements remain real requirements too.
        for feature in matched.planned.iter().rev() {
            matcher = planned::planned_item(feature, &matcher);
        }
        writeln!(self.functions, "{matcher}").unwrap();
        let unavailable = format!("{label}_unavailable");
        writeln!(self.functions, "#[allow(unused_variables)] fn {unavailable}(q: &dyn Query, asker: &Asker, event: &Event) -> Option<Fired> {{ None }}").unwrap();
        let mask = matched.masks.iter().map(|m| format!("EventMask::{m}.0")).collect::<Vec<_>>().join(" | ");
        let proposed = format!("when_asked_with_other_object(EventMask({mask}), {label})");
        let fallback = format!("when_asked(EventMask(0), {unavailable})");
        let mut trigger = proposed;
        for feature in &matched.planned {
            if !planned::landed(feature) {
                self.gaps.push(gap(feature.clone()));
                trigger = planned::planned_value(feature, &fallback, &trigger);
            }
        }
        // trigger() retains the inner matcher's functioning zones.
        trigger
    }
}
