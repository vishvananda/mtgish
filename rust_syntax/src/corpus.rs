//! Strict, streaming access to the existing parser's RON corpus.

use crate::mtg_types::OracleCard;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Do not skip blank, malformed, or unreadable records. The caller decides how
/// to stage its output; a successful pass accounts for every input line.
pub fn for_each_card(
    path: &Path,
    mut visit: impl FnMut(usize, OracleCard) -> Result<()>,
) -> Result<()> {
    for (index, line) in BufReader::new(File::open(path)?).lines().enumerate() {
        let number = index + 1;
        let line = line
            .map_err(|e| io::Error::new(e.kind(), format!("{}:{number}: {e}", path.display())))?;
        let card = ron::from_str(&line).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{}:{number}: {e}", path.display()),
            )
        })?;
        visit(number, card)?;
    }
    Ok(())
}

pub fn read_cards(path: &Path) -> Result<Vec<OracleCard>> {
    let mut cards = Vec::new();
    for_each_card(path, |_, card| {
        cards.push(card);
        Ok(())
    })?;
    if cards.is_empty() {
        return Err("card corpus is empty".into());
    }
    Ok(cards)
}

pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Hash every byte of files affecting source parsing or Rust interpretation.
/// The paths are repository-relative, making the manifest portable.
pub fn input_fingerprints(root: &Path) -> Result<BTreeMap<String, String>> {
    fn collect(root: &Path, dir: &Path, out: &mut BTreeMap<String, String>) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            if path.is_dir() {
                collect(root, &path, out)?;
            } else if matches!(
                path.extension().and_then(|s| s.to_str()),
                Some("rs" | "go" | "json5")
            ) {
                out.insert(
                    path.strip_prefix(root)?.to_string_lossy().into_owned(),
                    digest(&fs::read(path)?),
                );
            }
        }
        Ok(())
    }
    let mut out = BTreeMap::new();
    for dir in [
        "grammars",
        "go_mtg_parser",
        "rust_syntax/src",
        "rust_syntax/normalize/src",
    ] {
        collect(root, &root.join(dir), &mut out)?;
    }
    for path in [
        "data/oracle.json",
        "data/tokens.json",
        "data/dungeons.json",
        "data/additional_cards.json",
        "rust_syntax/Cargo.toml",
        "rust_syntax/normalize/Cargo.toml",
        "rust_syntax/Cargo.lock",
        "go_mtg_parser/go.mod",
        "grammars/generate_plurals",
        "justfile",
    ] {
        out.insert(path.into(), digest(&fs::read(root.join(path))?));
    }
    Ok(out)
}

/// A temporary sibling of the destination: validation must finish before commit.
/// Dropping an unsuccessful write removes the temporary file and preserves the
/// previous artifact, including when an input record fails late in the corpus.
pub struct Artifact {
    path: PathBuf,
    destination: PathBuf,
    file: File,
}

impl Artifact {
    pub fn create(destination: &Path) -> Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let name = destination
            .file_name()
            .ok_or("artifact destination needs a file name")?
            .to_string_lossy();
        let path = destination.with_file_name(format!(
            ".{name}.{}.{}.tmp",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        Ok(Self {
            path,
            destination: destination.into(),
            file,
        })
    }

    pub fn commit(mut self) -> Result<()> {
        self.file.flush()?;
        fs::rename(&self.path, &self.destination)?;
        Ok(())
    }
}

impl Write for Artifact {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.file.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

impl Drop for Artifact {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

pub fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let mut out = Artifact::create(path)?;
    serde_json::to_writer_pretty(&mut out, value)?;
    writeln!(out)?;
    out.commit()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
    pub layout: String,
    /// All named faces and token definitions, in structural order.
    pub names: Vec<String>,
    /// Hash of the complete typed source representation, including unnamed faces.
    pub source_sha256: String,
}

impl Identity {
    pub fn of(card: &OracleCard) -> Result<Self> {
        let value = crate::inspect::value(card)?;
        fn names(value: &serde_json::Value, out: &mut Vec<String>) {
            match value {
                serde_json::Value::Object(fields) => {
                    if let Some(name) = fields.get("Name").and_then(|v| v.as_str()) {
                        out.push(name.into());
                    }
                    for child in fields.values() {
                        names(child, out);
                    }
                }
                serde_json::Value::Array(items) => {
                    for child in items {
                        names(child, out);
                    }
                }
                _ => {}
            }
        }
        let mut result = Self {
            layout: value["variant"]
                .as_str()
                .ok_or("missing card layout")?
                .into(),
            names: Vec::new(),
            source_sha256: digest(&serde_json::to_vec(&value)?),
        };
        names(&value, &mut result.names);
        Ok(result)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub manifest_version: u32,
    pub corpus_sha256: String,
    pub inputs: BTreeMap<String, String>,
    pub records: Vec<Identity>,
    pub forms: BTreeMap<String, BTreeMap<String, usize>>,
}

impl Manifest {
    pub fn build(path: &Path, root: &Path) -> Result<Self> {
        let mut result = Self {
            manifest_version: 1,
            corpus_sha256: digest(&fs::read(path)?),
            inputs: input_fingerprints(root)?,
            records: Vec::new(),
            forms: BTreeMap::new(),
        };
        for_each_card(path, |_, card| {
            crate::inspect::count(&crate::inspect::value(&card)?, &mut result.forms);
            result.records.push(Identity::of(&card)?);
            Ok(())
        })?;
        if result.records.is_empty() {
            return Err("card corpus is empty".into());
        }
        Ok(result)
    }

    /// Source hashes can change for an intentional grammar repair. Identity
    /// comparison still requires every layout and named face, with multiplicity.
    pub fn check_identities(&self, baseline: &Self) -> Result<()> {
        if self.manifest_version != 1 || baseline.manifest_version != 1 {
            return Err("unsupported corpus manifest version".into());
        }
        fn identities(m: &Manifest) -> Vec<(&String, &Vec<String>, Option<&String>)> {
            let mut keys: Vec<_> = m
                .records
                .iter()
                .map(|r| {
                    (
                        &r.layout,
                        &r.names,
                        r.names.is_empty().then_some(&r.source_sha256),
                    )
                })
                .collect();
            keys.sort();
            keys
        }
        if identities(self) != identities(baseline) {
            return Err("corpus record identities or layouts differ from baseline".into());
        }
        Ok(())
    }
}
