//! One source registry for normalized types, traversal, schema and fingerprints.
//! Keep roles explicit: authoring helpers do not change the frozen wire shape,
//! and external vocabularies remain leaves of the semantic visitor.

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Semantic,
    Vocabulary,
    Authoring,
}

pub struct Source {
    pub path: &'static str,
    pub text: &'static str,
    pub role: Role,
}

pub const SOURCES: &[Source] = &[
    Source {
        path: "src/ir.rs",
        text: include_str!("ir.rs"),
        role: Role::Semantic,
    },
    Source {
        path: "src/ir/cards.rs",
        text: include_str!("ir/cards.rs"),
        role: Role::Semantic,
    },
    Source {
        path: "src/ir/abilities.rs",
        text: include_str!("ir/abilities.rs"),
        role: Role::Semantic,
    },
    Source {
        path: "src/ir/program.rs",
        text: include_str!("ir/program.rs"),
        role: Role::Semantic,
    },
    Source {
        path: "src/ir/selection.rs",
        text: include_str!("ir/selection.rs"),
        role: Role::Semantic,
    },
    Source {
        path: "src/ir/movement.rs",
        text: include_str!("ir/movement.rs"),
        role: Role::Semantic,
    },
    Source {
        path: "src/ir/objects.rs",
        text: include_str!("ir/objects.rs"),
        role: Role::Semantic,
    },
    Source {
        path: "src/ir/players.rs",
        text: include_str!("ir/players.rs"),
        role: Role::Semantic,
    },
    Source {
        path: "src/ir/values.rs",
        text: include_str!("ir/values.rs"),
        role: Role::Semantic,
    },
    Source {
        path: "src/ir/events.rs",
        text: include_str!("ir/events.rs"),
        role: Role::Semantic,
    },
    Source {
        path: "src/ir/changes.rs",
        text: include_str!("ir/changes.rs"),
        role: Role::Semantic,
    },
    Source {
        path: "src/ir/costs.rs",
        text: include_str!("ir/costs.rs"),
        role: Role::Semantic,
    },
    Source {
        path: "src/ir/mana.rs",
        text: include_str!("ir/mana.rs"),
        role: Role::Semantic,
    },
    Source {
        path: "src/ir/damage.rs",
        text: include_str!("ir/damage.rs"),
        role: Role::Semantic,
    },
    Source {
        path: "src/ir/keywords.rs",
        text: include_str!("ir/keywords.rs"),
        role: Role::Semantic,
    },
    Source {
        path: "src/ir/actions.rs",
        text: include_str!("ir/actions.rs"),
        role: Role::Semantic,
    },
    Source {
        path: "src/ir/conditions.rs",
        text: include_str!("ir/conditions.rs"),
        role: Role::Semantic,
    },
    Source {
        path: "src/ir/tokens.rs",
        text: include_str!("ir/tokens.rs"),
        role: Role::Semantic,
    },
    Source {
        path: "src/ir/copying.rs",
        text: include_str!("ir/copying.rs"),
        role: Role::Semantic,
    },
    Source {
        path: "src/counter.rs",
        text: include_str!("counter.rs"),
        role: Role::Vocabulary,
    },
    Source {
        path: "src/targets.rs",
        text: include_str!("targets.rs"),
        role: Role::Authoring,
    },
];

pub const SOURCE_VOCABULARY: &str = include_str!("../../src/mtg_types.rs");

pub fn wire_texts() -> Vec<&'static str> {
    SOURCES
        .iter()
        .filter(|source| source.role != Role::Authoring)
        .map(|source| source.text)
        .collect()
}

pub fn authoring_texts() -> Vec<&'static str> {
    std::iter::once(SOURCE_VOCABULARY)
        .chain(SOURCES.iter().map(|source| source.text))
        .collect()
}

pub fn traversal_text() -> String {
    SOURCES
        .iter()
        .filter(|source| source.role == Role::Semantic)
        // Inner module documentation is valid per file, not in the middle of
        // the flat declaration stream consumed by the traversal generator.
        .flat_map(|source| source.text.lines().filter(|line| !line.starts_with("//!")))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Infrastructure and model files that can change generated semantics.
pub fn fingerprint_sources() -> impl Iterator<Item = (&'static str, &'static str)> {
    std::iter::once(("src/declarations.rs", include_str!("declarations.rs")))
        .chain(SOURCES.iter().map(|source| (source.path, source.text)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn every_external_semantic_module_is_registered() {
        let paths: BTreeSet<_> = SOURCES.iter().map(|source| source.path).collect();
        assert_eq!(paths.len(), SOURCES.len(), "duplicate declaration source");
        for source in SOURCES
            .iter()
            .filter(|source| source.role == Role::Semantic)
        {
            for item in syn::parse_file(source.text).unwrap().items {
                if let syn::Item::Mod(module) = item {
                    if module.content.is_none() {
                        let path = format!(
                            "{}/{}.rs",
                            source.path.strip_suffix(".rs").unwrap(),
                            module.ident
                        );
                        assert!(
                            paths.contains(path.as_str()),
                            "unregistered declaration module {path}"
                        );
                    }
                }
            }
        }
    }
}
