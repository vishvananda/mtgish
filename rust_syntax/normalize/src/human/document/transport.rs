//! Carry actual definitions first, then reconstruct receipt-group expressions.
//! A single numeric receipt operand can expand into several physical sites
//! without splitting any of its constituent branch definitions.
use super::*;

fn members(bindings: &[BindingName], index: usize) -> BTreeSet<usize> {
    if bindings[index].members.is_empty() {
        BTreeSet::from([index])
    } else {
        bindings[index].members.iter().copied().collect()
    }
}
impl Document {
    /// Apply a complete structural map atomically. Actual definition indices and
    /// explicit names remain stable. Receipt expressions may expand or coalesce;
    /// ambiguous merges/splits of actual definitions still require a decision.
    pub fn transport(
        &self,
        card: &UnifiedCard,
        map: &crate::binding_map::SiteMap,
    ) -> Result<Self, String> {
        self.write()?;
        checked(card)?;
        let ty = Ty::Named("UnifiedCard".into());
        let before = serde_json::to_value(&self.card).map_err(|e| e.to_string())?;
        let after = serde_json::to_value(card).map_err(|e| e.to_string())?;
        map.validate(
            &crate::binding_map::sites(schema(), &ty, &before),
            &crate::binding_map::sites(schema(), &ty, &after),
        )?;
        if meaning(&self.card)? != meaning(card)? {
            return Err("mapped rewrite changes the card's meaning".into());
        }
        let resolved = resolve_card(card)?;
        let groups: BTreeMap<_, _> = resolved
            .uses
            .iter()
            .map(|u| (u.path.as_str(), u.binding))
            .collect();
        let mut incoming = vec![BTreeSet::new(); resolved.bindings.len()];
        let mut alias_origins = incoming.clone();
        let mut outgoing = vec![BTreeSet::new(); self.bindings.len()];
        let mut annotated = HashSet::new();
        for use_ in &self.uses {
            annotated.insert(use_.binding);
            for path in &map.sites[&use_.path] {
                let group = *groups
                    .get(path.as_str())
                    .ok_or("unresolved mapped ID site")?;
                match (
                    self.bindings[use_.binding].members.is_empty(),
                    resolved.bindings[group].members.is_empty(),
                ) {
                    (true, true) => {
                        incoming[group].insert(use_.binding);
                        outgoing[use_.binding].insert(group);
                    }
                    (false, false) => {
                        alias_origins[group].insert(use_.binding);
                    }
                    _ => {}
                }
            }
        }
        for (index, groups) in outgoing.iter().enumerate() {
            if !self.bindings[index].members.is_empty() {
                continue;
            }
            if groups.len() > 1 {
                return Err(format!(
                    "rewrite splits binding {index}; explicit names for the split are required"
                ));
            }
            if groups.is_empty() && annotated.contains(&index) && self.bindings[index].explicit {
                return Err(format!("rewrite removes explicitly named binding {index}"));
            }
        }
        let projection = |index: usize| -> BTreeSet<usize> {
            members(&self.bindings, index)
                .iter()
                .flat_map(|member| outgoing[*member].iter().copied())
                .collect()
        };
        // Both sides must be covered. Cloning one aggregate reference covers a
        // different member at each output; coalescing references covers their
        // union. Neither permits a reference to attach to an unrelated member.
        let known: HashSet<_> = self.uses.iter().map(|u| u.path.as_str()).collect();
        let unconstrained: HashSet<_> = map
            .introduced
            .iter()
            .chain(
                map.sites
                    .iter()
                    .filter(|(path, _)| !known.contains(path.as_str()))
                    .flat_map(|(_, paths)| paths.iter()),
            )
            .map(String::as_str)
            .collect();
        let mut coverage: BTreeMap<&str, BTreeSet<usize>> = BTreeMap::new();
        for use_ in &self.uses {
            let destinations = &map.sites[&use_.path];
            if destinations.is_empty() {
                continue;
            }
            let expected = projection(use_.binding);
            let mut covered = BTreeSet::new();
            for path in destinations {
                let target = members(&resolved.bindings, groups[path.as_str()]);
                let common: BTreeSet<_> = expected.intersection(&target).copied().collect();
                if common.is_empty() {
                    return Err("mapped reference crosses a binding or receipt scope".into());
                }
                covered.extend(common.iter().copied());
                coverage.entry(path).or_default().extend(common);
            }
            if covered != expected {
                return Err("mapped receipt omits an alternative definition".into());
            }
        }
        for (path, covered) in coverage {
            if !unconstrained.contains(path) && covered != members(&resolved.bindings, groups[path])
            {
                return Err("mapped receipt introduces an unrelated alternative".into());
            }
        }
        let mut bindings = self.bindings.clone();
        let mut assigned = Vec::new();
        let mut reused_aliases = HashSet::new();
        for (group, origins) in incoming.iter().enumerate() {
            if origins.len() > 1 {
                return Err(format!("rewrite merges bindings {origins:?}; a retained identity must be chosen explicitly"));
            }
            let descriptor = &resolved.bindings[group];
            let origin = origins.iter().next().copied().or_else(|| {
                alias_origins[group].iter().copied().find(|index| {
                    projection(*index) == members(&resolved.bindings, group)
                        && reused_aliases.insert(*index)
                })
            });
            let index = if let Some(index) = origin {
                bindings[index].scope = descriptor.scope;
                bindings[index].scope_name = descriptor.scope_name.clone();
                if !bindings[index].explicit {
                    bindings[index].name = descriptor.name.clone();
                }
                index
            } else {
                let index = bindings.len();
                bindings.push(descriptor.clone());
                index
            };
            assigned.push(index);
        }
        for (group, descriptor) in resolved.bindings.iter().enumerate() {
            bindings[assigned[group]].members =
                descriptor.members.iter().map(|i| assigned[*i]).collect();
        }
        let qualified: BTreeSet<_> = self
            .uses
            .iter()
            .filter(|u| u.qualified)
            .flat_map(|u| map.sites[&u.path].iter())
            .collect();
        let uses = resolved
            .uses
            .iter()
            .map(|u| BindingUse {
                path: u.path.clone(),
                binding: assigned[u.binding],
                qualified: u.qualified || qualified.contains(&u.path),
            })
            .collect();
        let transported = Self {
            version: self.version,
            card: card.clone(),
            bindings,
            uses,
        };
        transported
            .write()
            .map_err(|e| format!("mapped binding names are ambiguous: {e}"))?;
        Ok(transported)
    }
}
