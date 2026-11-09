//! A v1 document could use one identity for several alternative definitions.
//! Validate its old grouping first, then split by current semantic scopes. The
//! card and all explicit declaration names survive; the original input is never
//! mutated in place. This migration changes authoring metadata, not engine IR.
use super::*;

/// Carry saved annotation identities through the exact schema migration before
/// deserializing the new card type. The semantic converter owns every path move;
/// raw labels never serve as an identity heuristic. Native projection then
/// checks the remapped groups, scopes and names as a single transaction.
pub(super) fn upgrade_schema(value: &mut Value) -> Result<(), String> {
    let source = &value["card"];
    let mapped = crate::migrate::mapped_read_json(
        &serde_json::to_string(source).map_err(|e| e.to_string())?,
    )?;
    let sites = crate::migrate::input_sites(mapped.source_version, source)?;
    let bindings: Vec<BindingName> =
        serde_json::from_value(value["bindings"].clone()).map_err(|e| e.to_string())?;
    let uses: Vec<BindingUse> =
        serde_json::from_value(value["uses"].clone()).map_err(|e| e.to_string())?;
    if serde_json::to_value(&bindings).map_err(|e| e.to_string())? != value["bindings"]
        || serde_json::to_value(&uses).map_err(|e| e.to_string())? != value["uses"]
    {
        return Err("unknown or missing legacy authoring annotation fields".into());
    }
    let mut seen = HashSet::new();
    let mut present = HashSet::new();
    let mut retained = HashSet::new();
    let mut output = Vec::new();
    for use_ in uses {
        let binding = bindings.get(use_.binding).ok_or("unknown legacy binding")?;
        if !seen.insert(use_.path.clone()) || sites.get(&use_.path) != Some(&binding.kind) {
            return Err(
                "legacy annotation has a duplicate, mistyped or missing source site".into(),
            );
        }
        present.insert(use_.binding);
        for path in &mapped.map.sites[&use_.path] {
            retained.insert(use_.binding);
            output.push(BindingUse {
                path: path.clone(),
                binding: use_.binding,
                qualified: use_.qualified,
            });
        }
    }
    for index in present.difference(&retained) {
        if bindings[*index].explicit {
            return Err(format!(
                "schema upgrade removes explicitly named binding {index}"
            ));
        }
    }
    value["card"] = serde_json::to_value(mapped.card).map_err(|e| e.to_string())?;
    value["uses"] = serde_json::to_value(output).map_err(|e| e.to_string())?;
    Ok(())
}

impl Document {
    pub(super) fn upgrade_v1(self) -> Result<Self, String> {
        if self.uses.iter().any(|u| u.qualified)
            || self
                .bindings
                .iter()
                .any(|b| !b.members.is_empty() || !b.scope_name.is_empty())
        {
            return Err("version 1 authoring metadata cannot contain version 2 receipt groups or scope paths".into());
        }
        checked(&self.card)?;
        let value = serde_json::to_value(&self.card).map_err(|e| e.to_string())?;
        let mut sites = BTreeMap::new();
        id_sites(&Ty::Named("UnifiedCard".into()), &value, "", &mut sites);
        let code = super::super::write(&self.card)?;
        let node = syntax::parse(&code)?;
        let (mut projection, bindings) = codec::read_legacy_bindings(&node)?;
        let mut uses = Vec::new();
        extract(&mut projection, "", &mut uses)?;
        let legacy = Document {
            version: 1,
            card: self.card.clone(),
            bindings,
            uses,
        };
        self.project_against(value, sites, &legacy, false)?;

        let current = resolve_card(&self.card)?;
        let by_path: BTreeMap<_, _> = current
            .uses
            .iter()
            .map(|u| (u.path.as_str(), u.binding))
            .collect();
        let mut incoming = vec![BTreeSet::new(); current.bindings.len()];
        for use_ in &self.uses {
            incoming[by_path[use_.path.as_str()]].insert(use_.binding);
        }
        let mut bindings = self.bindings;
        let (mut assigned, mut used) = (Vec::new(), HashSet::new());
        for (group, origins) in incoming.iter().enumerate() {
            if origins.len() > 1 {
                return Err("legacy metadata gives conflicting names to one binding".into());
            }
            let descriptor = &current.bindings[group];
            let mut binding = origins
                .iter()
                .next()
                .map(|i| bindings[*i].clone())
                .unwrap_or_else(|| descriptor.clone());
            binding.scope = descriptor.scope;
            binding.scope_name = descriptor.scope_name.clone();
            binding.members = vec![];
            if !descriptor.members.is_empty() {
                binding.name = descriptor.name.clone();
                binding.explicit = false;
            }
            let index = origins
                .iter()
                .next()
                .copied()
                .filter(|i| used.insert(*i))
                .unwrap_or_else(|| {
                    let index = bindings.len();
                    used.insert(index);
                    bindings.push(binding.clone());
                    index
                });
            bindings[index] = binding;
            assigned.push(index);
        }
        for (group, descriptor) in current.bindings.iter().enumerate() {
            bindings[assigned[group]].members =
                descriptor.members.iter().map(|i| assigned[*i]).collect();
        }
        let uses = current
            .uses
            .iter()
            .map(|u| BindingUse {
                path: u.path.clone(),
                binding: assigned[u.binding],
                qualified: u.qualified,
            })
            .collect();
        let doc = Document {
            version: 2,
            card: self.card,
            bindings,
            uses,
        };
        doc.write()
            .map_err(|e| format!("legacy name migration is ambiguous: {e}"))?;
        Ok(doc)
    }
}
