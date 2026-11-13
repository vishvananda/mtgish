//! Whole-tree stage coverage and deterministic fixture selection. Structural
//! coverage is explicitly independent of complete semantic lowering.
use super::*;
use crate::{
    corpus::{self, Identity, Manifest},
    lower,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct Record {
    pub line: usize,
    pub identity: Identity,
    pub nodes: usize,
    pub faces: usize,
    pub rewrites: usize,
    pub number_rewrites: usize,
    pub selector_rewrites: usize,
    pub program_rewrites: usize,
    pub operand_rewrites: usize,
    pub mechanic_rewrites: usize,
    pub semantic_complete: bool,
    /// Unreduced forms, nested ones included. Unlike `source_forms`, this lists only
    /// what actually failed to reduce, so it is the true worklist.
    #[serde(default)]
    pub named: BTreeMap<String, usize>,
    /// Per-record membership for dependency/impact queries over every subtree.
    pub source_forms: BTreeSet<String>,
    pub diagnostics: Vec<lower::Diagnostic>,
    /// Every action form still outside the generated structural registry,
    /// including nested forms hidden by an unsupported parent/layout. These
    /// are NOT all semantically unsupported: existing lowerers can bind them.
    pub remaining_registry_forms: BTreeMap<String, usize>,
    pub fixture_features: BTreeSet<String>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct FixtureSelection {
    pub universe: usize,
    pub covered: usize,
    pub selected_lines: Vec<usize>,
    pub unseen_registry_rules: Vec<String>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Report {
    pub pipeline_version: u32,
    pub final_schema_version: u32,
    pub corpus_sha256: String,
    pub inputs: BTreeMap<String, String>,
    pub total: usize,
    pub structurally_preserved: usize,
    pub source_nodes: usize,
    pub face_records: usize,
    pub rewritten_actions: usize,
    pub rewritten_numbers: usize,
    pub rewritten_selectors: usize,
    pub rewritten_programs: usize,
    pub rewritten_costs: usize,
    pub rewritten_conditions: usize,
    pub rewritten_mechanics: usize,
    pub records_with_mechanic_rewrites: usize,
    pub mechanic_rules: BTreeMap<String, usize>,
    pub records_with_operand_rewrites: usize,
    pub operand_rules: BTreeMap<String, usize>,
    pub remaining_operand_forms: BTreeMap<String, usize>,
    pub records_with_program_rewrites: usize,
    pub program_rules: BTreeMap<String, usize>,
    pub remaining_program_forms: BTreeMap<String, usize>,
    pub records_with_selector_rewrites: usize,
    pub selector_rules: BTreeMap<String, usize>,
    pub remaining_selector_forms: BTreeMap<String, usize>,
    pub records_with_number_rewrites: usize,
    pub number_rules: BTreeMap<String, usize>,
    pub remaining_number_forms: BTreeMap<String, usize>,
    pub records_with_rewrites: usize,
    pub semantic_complete: usize,
    #[serde(default)]
    pub unified: usize,
    pub source_forms: crate::inventory::Forms,
    pub remaining_registry_forms: BTreeMap<String, usize>,
    pub rewrite_rules: BTreeMap<String, usize>,
    pub expanded_recipes: BTreeMap<String, usize>,
    /// Count operands/opcodes too, so outer recipe counts cannot hide complexity.
    pub recipe_forms: crate::inventory::Forms,
    pub final_forms: crate::inventory::Forms,
    pub records: Vec<Record>,
    pub fixtures: FixtureSelection,
}
fn features(value: &serde_json::Value, context: &str, found: &mut BTreeSet<String>) {
    if let (Some(ty), Some(form)) = (
        value.get("enum").and_then(serde_json::Value::as_str),
        value.get("variant").and_then(serde_json::Value::as_str),
    ) {
        found.insert(format!("{context}/operand:{ty}::{form}"));
    }
    match value {
        serde_json::Value::Object(fields) => {
            for child in fields.values() {
                features(child, context, found);
            }
        }
        serde_json::Value::Array(values) => {
            for child in values {
                features(child, context, found);
            }
        }
        _ => {}
    }
}
fn recipe_forms(value: &(impl Serialize + Sync), out: &mut crate::inventory::Forms) -> corpus::Result<()> {
    let mut forms = BTreeMap::new();
    crate::inspect::count(&crate::inspect::value(value)?, &mut forms);
    for (ty, variants) in forms {
        if matches!(
            ty.as_str(),
            "SurfaceAction"
                | "ExpandedRecipe"
                | "Quantity"
                | "SelectionInput"
                | "Subjects"
                | "DurationInput"
                | "ChangeTemplate"
                | "PlayerOperation"
                | "ObjectOperation"
                | "Destination"
                | "EffectEvaluation"
                | "NumberObject"
                | "PaymentAction"
                | "NumberQuery"
                | "NumberPlayers"
                | "NumberRecipients"
                | "Property"
                | "Aggregation"
                | "DistinctProperty"
                | "Rounding"
                | "EventZone"
                | "Predicate"
                | "SelectorSource"
                | "PredicateLiteral"
                | "PredicateVocabulary"
                | "PredicateObject"
                | "PredicateComparison"
                | "InstructionRecipe"
                | "ProgramSource"
                | "ActorInput"
                | "ModeBounds"
                | "CostRecipe"
                | "CheckRecipe"
                | "CostSelection"
                | "CostFilter"
                | "AmountOperation"
                | "OperandRecipe"
                | "Mechanic"
        ) {
            for (form, count) in variants {
                *out.entry(ty.clone()).or_default().entry(form).or_default() += count;
            }
        }
    }
    Ok(())
}
// Predicate recursion is checked by its own source nodes and rule IDs. Cover
// immediate argument variants here instead of every descendant combination of
// every All/Or node; numeric/action operands retain their deeper feature policy.
fn shallow_features(value: &serde_json::Value, context: &str, found: &mut BTreeSet<String>) {
    if let (Some(ty), Some(form)) = (
        value.get("enum").and_then(serde_json::Value::as_str),
        value.get("variant").and_then(serde_json::Value::as_str),
    ) {
        found.insert(format!("{context}/operand:{ty}::{form}"));
        return;
    }
    match value {
        serde_json::Value::Array(values) => {
            for value in values {
                shallow_features(value, context, found)
            }
        }
        serde_json::Value::Object(values) => {
            for value in values.values() {
                shallow_features(value, context, found)
            }
        }
        _ => {}
    }
}
fn rewrite_features(
    source: &SourceIndex,
    origin: &NodeId,
    id: &str,
    arity: usize,
    deep: bool,
    found: &mut BTreeSet<String>,
) {
    found.insert(format!("rule:{id}"));
    let value = source.at(origin);
    let operand_features = if deep { features } else { shallow_features };
    if arity == 1 {
        operand_features(&value["args"], &format!("rule:{id}/argument:0"), found);
    } else if let Some(args) = value["args"].as_array() {
        for (i, arg) in args.iter().enumerate() {
            operand_features(arg, &format!("rule:{id}/argument:{i}"), found);
        }
    }
    let mut parent = source.nodes[origin.0 as usize].parent.as_ref();
    while let Some(p) = parent {
        let node = &source.nodes[p.0 as usize];
        if matches!(
            node.source_type.as_str(),
            "Action" | "Actions" | "Rule" | "OracleCard" | "GameNumber" | "Cost" | "Condition"
        ) || crate::selector_spec::Domain::from_rust_name(&node.source_type).is_some()
        {
            found.insert(format!(
                "rule:{id}/scope:{}::{}",
                node.source_type, node.form
            ));
        }
        parent = node.parent.as_ref();
    }
}
impl Report {
    fn empty(corpus_sha256: String, inputs: BTreeMap<String, String>) -> Self {
        Self {
            pipeline_version: 8,
            final_schema_version: ir::SCHEMA_VERSION,
            corpus_sha256,
            inputs,
            total: 0,
            structurally_preserved: 0,
            source_nodes: 0,
            face_records: 0,
            rewritten_actions: 0,
            rewritten_numbers: 0,
            rewritten_selectors: 0,
            rewritten_programs: 0,
            rewritten_costs: 0,
            rewritten_conditions: 0,
            rewritten_mechanics: 0,
            records_with_mechanic_rewrites: 0,
            mechanic_rules: BTreeMap::new(),
            records_with_operand_rewrites: 0,
            operand_rules: BTreeMap::new(),
            remaining_operand_forms: BTreeMap::new(),
            records_with_program_rewrites: 0,
            program_rules: BTreeMap::new(),
            remaining_program_forms: BTreeMap::new(),
            records_with_selector_rewrites: 0,
            selector_rules: BTreeMap::new(),
            remaining_selector_forms: BTreeMap::new(),
            records_with_number_rewrites: 0,
            number_rules: BTreeMap::new(),
            remaining_number_forms: BTreeMap::new(),
            records_with_rewrites: 0,
            semantic_complete: 0,
            unified: 0,
            source_forms: BTreeMap::new(),
            remaining_registry_forms: BTreeMap::new(),
            rewrite_rules: BTreeMap::new(),
            expanded_recipes: BTreeMap::new(),
            recipe_forms: BTreeMap::new(),
            final_forms: BTreeMap::new(),
            records: vec![],
            fixtures: FixtureSelection {
                universe: 0,
                covered: 0,
                selected_lines: vec![],
                unseen_registry_rules: vec![],
            },
        }
    }

    /// Destructures every field, so adding one cannot silently skip the merge.
    fn merge(&mut self, part: Self) {
        fn add(into: &mut BTreeMap<String, usize>, from: BTreeMap<String, usize>) {
            for (key, count) in from {
                *into.entry(key).or_default() += count;
            }
        }
        fn add_forms(into: &mut crate::inventory::Forms, from: crate::inventory::Forms) {
            for (family, counts) in from {
                add(into.entry(family).or_default(), counts);
            }
        }
        let Self {
            total,
            structurally_preserved,
            source_nodes,
            face_records,
            rewritten_actions,
            rewritten_numbers,
            rewritten_selectors,
            rewritten_programs,
            rewritten_costs,
            rewritten_conditions,
            rewritten_mechanics,
            records_with_mechanic_rewrites,
            records_with_operand_rewrites,
            records_with_program_rewrites,
            records_with_selector_rewrites,
            records_with_number_rewrites,
            records_with_rewrites,
            semantic_complete,
            unified,
            mechanic_rules,
            operand_rules,
            remaining_operand_forms,
            program_rules,
            remaining_program_forms,
            selector_rules,
            remaining_selector_forms,
            number_rules,
            remaining_number_forms,
            remaining_registry_forms,
            rewrite_rules,
            expanded_recipes,
            source_forms,
            recipe_forms,
            final_forms,
            records,
            pipeline_version: _,
            final_schema_version: _,
            corpus_sha256: _,
            inputs: _,
            fixtures: _,
        } = part;
        self.total += total;
        self.structurally_preserved += structurally_preserved;
        self.source_nodes += source_nodes;
        self.face_records += face_records;
        self.rewritten_actions += rewritten_actions;
        self.rewritten_numbers += rewritten_numbers;
        self.rewritten_selectors += rewritten_selectors;
        self.rewritten_programs += rewritten_programs;
        self.rewritten_costs += rewritten_costs;
        self.rewritten_conditions += rewritten_conditions;
        self.rewritten_mechanics += rewritten_mechanics;
        self.records_with_mechanic_rewrites += records_with_mechanic_rewrites;
        self.records_with_operand_rewrites += records_with_operand_rewrites;
        self.records_with_program_rewrites += records_with_program_rewrites;
        self.records_with_selector_rewrites += records_with_selector_rewrites;
        self.records_with_number_rewrites += records_with_number_rewrites;
        self.records_with_rewrites += records_with_rewrites;
        self.semantic_complete += semantic_complete;
        self.unified += unified;
        add(&mut self.mechanic_rules, mechanic_rules);
        add(&mut self.operand_rules, operand_rules);
        add(&mut self.remaining_operand_forms, remaining_operand_forms);
        add(&mut self.program_rules, program_rules);
        add(&mut self.remaining_program_forms, remaining_program_forms);
        add(&mut self.selector_rules, selector_rules);
        add(&mut self.remaining_selector_forms, remaining_selector_forms);
        add(&mut self.number_rules, number_rules);
        add(&mut self.remaining_number_forms, remaining_number_forms);
        add(&mut self.remaining_registry_forms, remaining_registry_forms);
        add(&mut self.rewrite_rules, rewrite_rules);
        add(&mut self.expanded_recipes, expanded_recipes);
        add_forms(&mut self.source_forms, source_forms);
        add_forms(&mut self.recipe_forms, recipe_forms);
        add_forms(&mut self.final_forms, final_forms);
        self.records.extend(records);
    }
    pub fn check(path: &Path, root: &Path, baseline: Option<&Manifest>) -> corpus::Result<Self> {
        let mut report = Self::empty(
            corpus::digest(&std::fs::read(path)?),
            input_fingerprints(root)?,
        );
        // Each record reports into its own `Report`, so all of this runs on workers.
        let prepare = |line: usize, card: crate::mtg_types::OracleCard| -> corpus::Result<Self> {
            let mut report = Self::empty(String::new(), BTreeMap::new());
            let structural = Structural::new(&card).map_err(|e| format!("line {line}: {e}"))?;
            let restored = structural
                .source
                .restore()
                .map_err(|e| format!("line {line}: source preservation: {e}"))?;
            if crate::inspect::value(&restored)? != structural.source.original {
                return Err(format!("line {line}: structural pass changed source data").into());
            }
            let lowered = crate::unified::lower(&card);
            let invalid = |e: String| format!("line {line}: invalid unified AST: {e}");
            crate::validate::unified(&lowered.card).map_err(invalid)?;
            crate::unified::round_trip(&lowered.card).map_err(invalid)?;
            let named = crate::unified::named_forms(&lowered.card);
            // Whole cards bind through the staged recipes, so a recipe that diverges
            // from native lowering still surfaces here.
            let diagnostics = if lowered.card.layout == crate::ir::Layout::Card {
                match bind_prepared(&card, &structural) {
                    Ok(_) => vec![],
                    Err(errors) => errors,
                }
            } else {
                lowered.diagnostics
            };
            let complete = diagnostics.is_empty() && named.is_empty();
            report.unified += 1;
            if complete {
                for face in &lowered.card.faces {
                    crate::validate::card(face)
                        .map_err(|e| format!("line {line}: invalid final AST: {e}"))?;
                    crate::inventory::Inventory(&mut report.final_forms).card(face);
                }
                report.semantic_complete += 1;
            }
            let mut remaining = BTreeMap::new();
            let mut source_forms = BTreeSet::new();
            for node in &structural.source.nodes {
                source_forms.insert(format!("{}::{}", node.source_type, node.form));
                *report
                    .source_forms
                    .entry(node.source_type.clone())
                    .or_default()
                    .entry(node.form.clone())
                    .or_default() += 1;
                if node.source_type == "Action"
                    && !registry().rules.iter().any(|r| r.source == node.form)
                {
                    let key = format!("{}::{}", node.source_type, node.form);
                    *remaining.entry(key.clone()).or_default() += 1;
                    *report.remaining_registry_forms.entry(key).or_default() += 1;
                }
            }
            let mut fixture_features = BTreeSet::new();
            for rewrite in &structural.rewrites {
                recipe_forms(&rewrite.surface, &mut report.recipe_forms)?;
                recipe_forms(&rewrite.expanded, &mut report.recipe_forms)?;
                *report
                    .rewrite_rules
                    .entry(rewrite.rule.clone())
                    .or_default() += 1;
                let rule = registry()
                    .rules
                    .iter()
                    .find(|r| r.id == rewrite.rule)
                    .unwrap();
                rewrite_features(
                    &structural.source,
                    &rewrite.origin,
                    &rewrite.rule,
                    rule.arity,
                    true,
                    &mut fixture_features,
                );
                let kind = rewrite.expanded.kind();
                *report.expanded_recipes.entry(kind.into()).or_default() += 1;
            }
            for rewrite in &structural.programs {
                recipe_forms(&rewrite.recipe, &mut report.recipe_forms)?;
                *report
                    .program_rules
                    .entry(rewrite.rule.clone())
                    .or_default() += 1;
                let rule = programs::registry()
                    .rules
                    .iter()
                    .find(|r| r.id == rewrite.rule)
                    .unwrap();
                rewrite_features(
                    &structural.source,
                    &rewrite.origin,
                    &rewrite.rule,
                    rule.arity,
                    false,
                    &mut fixture_features,
                );
            }
            for node in &structural.source.nodes {
                let domain = match node.source_type.as_str() {
                    "Action" => crate::program_spec::Domain::Action,
                    "Actions" => crate::program_spec::Domain::Actions,
                    _ => continue,
                };
                if !programs::registry()
                    .rules
                    .iter()
                    .any(|r| r.domain == domain && r.source == node.form)
                {
                    *report
                        .remaining_program_forms
                        .entry(format!("{}::{}", node.source_type, node.form))
                        .or_default() += 1;
                }
            }
            for rewrite in &structural.mechanics {
                recipe_forms(&rewrite.recipe, &mut report.recipe_forms)?;
                *report
                    .mechanic_rules
                    .entry(rewrite.rule.clone())
                    .or_default() += 1;
                let rule = mechanics::registry()
                    .rules
                    .iter()
                    .find(|r| r.id == rewrite.rule)
                    .unwrap();
                rewrite_features(
                    &structural.source,
                    &rewrite.origin,
                    &rewrite.rule,
                    rule.arity,
                    false,
                    &mut fixture_features,
                );
            }
            for rewrite in &structural.operands {
                recipe_forms(&rewrite.recipe, &mut report.recipe_forms)?;
                *report
                    .operand_rules
                    .entry(rewrite.rule.clone())
                    .or_default() += 1;
                match rewrite.recipe {
                    operands::OperandRecipe::Cost(_) => report.rewritten_costs += 1,
                    operands::OperandRecipe::Condition(_) => report.rewritten_conditions += 1,
                }
                let rule = operands::registry()
                    .rules
                    .iter()
                    .find(|r| r.id == rewrite.rule)
                    .unwrap();
                rewrite_features(
                    &structural.source,
                    &rewrite.origin,
                    &rewrite.rule,
                    rule.arity,
                    false,
                    &mut fixture_features,
                );
            }
            for node in &structural.source.nodes {
                let domain = match node.source_type.as_str() {
                    "Cost" => crate::operand_spec::Domain::Cost,
                    "Condition" => crate::operand_spec::Domain::Condition,
                    _ => continue,
                };
                if !operands::registry()
                    .rules
                    .iter()
                    .any(|r| r.domain == domain && r.source == node.form)
                {
                    *report
                        .remaining_operand_forms
                        .entry(format!("{}::{}", node.source_type, node.form))
                        .or_default() += 1;
                }
            }
            for rewrite in &structural.numbers {
                recipe_forms(&rewrite.expression, &mut report.recipe_forms)?;
                *report.number_rules.entry(rewrite.rule.clone()).or_default() += 1;
                let rule = numbers::registry()
                    .rules
                    .iter()
                    .find(|r| r.id == rewrite.rule)
                    .unwrap();
                rewrite_features(
                    &structural.source,
                    &rewrite.origin,
                    &rewrite.rule,
                    rule.arity,
                    true,
                    &mut fixture_features,
                );
            }
            for rewrite in &structural.selectors {
                recipe_forms(&rewrite.predicate, &mut report.recipe_forms)?;
                *report
                    .selector_rules
                    .entry(rewrite.rule.clone())
                    .or_default() += 1;
                let rule = selectors::registry()
                    .rules
                    .iter()
                    .find(|r| r.id == rewrite.rule)
                    .unwrap();
                rewrite_features(
                    &structural.source,
                    &rewrite.origin,
                    &rewrite.rule,
                    rule.arity,
                    false,
                    &mut fixture_features,
                );
            }
            for node in &structural.source.nodes {
                if let Some(domain) =
                    crate::selector_spec::Domain::from_rust_name(&node.source_type)
                {
                    if !selectors::registry()
                        .rules
                        .iter()
                        .any(|r| r.domain == domain && r.source == node.form)
                    {
                        *report
                            .remaining_selector_forms
                            .entry(format!("{}::{}", node.source_type, node.form))
                            .or_default() += 1;
                    }
                }
            }
            for node in &structural.source.nodes {
                if node.source_type == "GameNumber"
                    && !numbers::registry()
                        .rules
                        .iter()
                        .any(|r| r.source == node.form)
                {
                    *report
                        .remaining_number_forms
                        .entry(format!("GameNumber::{}", node.form))
                        .or_default() += 1;
                }
            }
            report.total += 1;
            report.structurally_preserved += 1;
            report.source_nodes += structural.source.nodes.len();
            report.face_records += structural.source.faces.len();
            report.rewritten_actions += structural.rewrites.len();
            report.rewritten_numbers += structural.numbers.len();
            report.rewritten_selectors += structural.selectors.len();
            report.rewritten_programs += structural.programs.len();
            report.rewritten_mechanics += structural.mechanics.len();
            report.records_with_mechanic_rewrites += usize::from(!structural.mechanics.is_empty());
            report.records_with_operand_rewrites += usize::from(!structural.operands.is_empty());
            report.records_with_program_rewrites += usize::from(!structural.programs.is_empty());
            report.records_with_selector_rewrites += usize::from(!structural.selectors.is_empty());
            report.records_with_number_rewrites += usize::from(!structural.numbers.is_empty());
            report.records_with_rewrites += usize::from(!structural.rewrites.is_empty());
            report.records.push(Record {
                line,
                identity: Identity::of(&card)?,
                nodes: structural.source.nodes.len(),
                faces: structural.source.faces.len(),
                rewrites: structural.rewrites.len(),
                number_rewrites: structural.numbers.len(),
                selector_rewrites: structural.selectors.len(),
                program_rewrites: structural.programs.len(),
                operand_rewrites: structural.operands.len(),
                mechanic_rewrites: structural.mechanics.len(),
                semantic_complete: complete,
                named,
                source_forms,
                diagnostics,
                remaining_registry_forms: remaining,
                fixture_features,
            });
            Ok(report)
        };
        corpus::map_cards(path, prepare, |_, part| {
            report.merge(part);
            Ok(())
        })?;
        if report.total == 0 {
            return Err("source corpus is empty".into());
        }
        if let Some(baseline) = baseline {
            Manifest {
                manifest_version: 1,
                corpus_sha256: report.corpus_sha256.clone(),
                inputs: report.inputs.clone(),
                records: report.records.iter().map(|r| r.identity.clone()).collect(),
                forms: BTreeMap::new(),
            }
            .check_identities(baseline)?;
        }
        report.fixtures = select_fixtures(&report.records);
        report.fixtures.unseen_registry_rules = registry()
            .rules
            .iter()
            .filter(|r| !report.rewrite_rules.contains_key(&r.id))
            .map(|r| r.id.clone())
            .chain(
                numbers::registry()
                    .rules
                    .iter()
                    .filter(|r| !report.number_rules.contains_key(&r.id))
                    .map(|r| r.id.clone()),
            )
            .chain(
                selectors::registry()
                    .rules
                    .iter()
                    .filter(|r| !report.selector_rules.contains_key(&r.id))
                    .map(|r| r.id.clone()),
            )
            .chain(
                programs::registry()
                    .rules
                    .iter()
                    .filter(|r| !report.program_rules.contains_key(&r.id))
                    .map(|r| r.id.clone()),
            )
            .chain(
                operands::registry()
                    .rules
                    .iter()
                    .filter(|r| !report.operand_rules.contains_key(&r.id))
                    .map(|r| r.id.clone()),
            )
            .chain(
                mechanics::registry()
                    .rules
                    .iter()
                    .filter(|r| !report.mechanic_rules.contains_key(&r.id))
                    .map(|r| r.id.clone()),
            )
            .collect();
        Ok(report)
    }
}

pub fn select_fixtures(records: &[Record]) -> FixtureSelection {
    use std::{cmp::Reverse, collections::BinaryHeap};
    // Remove a feature once, updating only records that contain it. Heap
    // entries are lazy: an older score is discarded when it reaches the top.
    let mut feature_records: BTreeMap<&String, Vec<usize>> = BTreeMap::new();
    let mut counts = vec![0; records.len()];
    let mut heap = BinaryHeap::new();
    for (i, record) in records.iter().enumerate() {
        counts[i] = record.fixture_features.len();
        if counts[i] > 0 {
            heap.push((
                counts[i],
                record.semantic_complete,
                Reverse(record.nodes),
                Reverse(record.line),
                i,
            ));
        }
        for feature in &record.fixture_features {
            feature_records.entry(feature).or_default().push(i);
        }
    }
    let universe = feature_records.len();
    let mut selected_lines = vec![];
    let mut touched_in_round = vec![usize::MAX; records.len()];
    while !feature_records.is_empty() {
        let (score, _, _, _, best) = heap.pop().expect("observed feature has a source record");
        if score != counts[best] {
            continue;
        }
        selected_lines.push(records[best].line);
        let round = selected_lines.len();
        let mut touched = vec![];
        for feature in &records[best].fixture_features {
            if let Some(affected) = feature_records.remove(feature) {
                for i in affected {
                    counts[i] -= 1;
                    if touched_in_round[i] != round {
                        touched_in_round[i] = round;
                        touched.push(i);
                    }
                }
            }
        }
        for i in touched {
            if counts[i] > 0 {
                let r = &records[i];
                heap.push((
                    counts[i],
                    r.semantic_complete,
                    Reverse(r.nodes),
                    Reverse(r.line),
                    i,
                ));
            }
        }
    }
    FixtureSelection {
        universe,
        covered: universe,
        selected_lines,
        unseen_registry_rules: vec![],
    }
}
pub fn write_fixtures(input: &Path, report: &Report, output: &Path) -> corpus::Result<usize> {
    use std::io::{BufRead, Write};
    let bytes = std::fs::read(input)?;
    if corpus::digest(&bytes) != report.corpus_sha256 {
        return Err("fixture report does not match source corpus".into());
    }
    for (path, hash) in registry_fingerprints() {
        if report.inputs.get(path) != Some(hash) {
            return Err("fixture report uses another rewrite registry".into());
        }
    }
    let wanted: BTreeSet<_> = report.fixtures.selected_lines.iter().copied().collect();
    let mut artifact = corpus::Artifact::create(output)?;
    let mut count = 0;
    for (i, line) in std::io::BufReader::new(bytes.as_slice())
        .lines()
        .enumerate()
    {
        let line = line?;
        if wanted.contains(&(i + 1)) {
            writeln!(artifact, "{line}")?;
            count += 1;
        }
    }
    if count != wanted.len() {
        return Err("fixture line missing from source".into());
    }
    artifact.commit()?;
    Ok(count)
}
