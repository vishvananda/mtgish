//! Strict, streaming access to the existing parser's RON corpus.

use crate::mtg_types::OracleCard;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::cell::Cell;
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::sync_channel;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Lowering recurses through large frames in debug builds, so a deep card must
/// not depend on the caller's stack.
const STACK_BYTES: usize = 256 << 20;
/// Large enough to avoid contention on the shared counter; small enough to balance.
const CHUNK_LINES: usize = 32;
// Set on any thread already running with `STACK_BYTES`.
std::thread_local!(static LARGE_STACK: Cell<bool> = const { Cell::new(false) });

/// Run `work` with a stack sized for lowering. Cheap when already on one.
pub fn on_large_stack<R: Send>(work: impl FnOnce() -> R + Send) -> R {
    if LARGE_STACK.get() {
        return work();
    }
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(STACK_BYTES)
            .spawn_scoped(scope, || {
                LARGE_STACK.set(true);
                work()
            })
            .expect("large stack thread")
            .join()
            .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
    })
}

fn worker_count() -> usize {
    std::env::var("MTGISH_THREADS")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map_or(1, |n| n.get())
                .min(32)
        })
        .max(1)
}

fn parse_line(path: &Path, number: usize, line: &str) -> Result<OracleCard> {
    ron::from_str(line).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{}:{number}: {e}", path.display()),
        )
        .into()
    })
}

/// Parse and `map` records on worker threads, then `visit` every result on the
/// calling thread in line order, so a report never depends on scheduling. Put
/// expensive work in `map`. Do not skip blank, malformed, or unreadable records:
/// the earliest failing line is reported, and a success accounts for every line.
pub fn map_cards<T: Send>(
    path: &Path,
    map: impl Fn(usize, OracleCard) -> Result<T> + Sync,
    mut visit: impl FnMut(usize, T) -> Result<()>,
) -> Result<()> {
    let text = fs::read_to_string(path)
        .map_err(|e| io::Error::new(e.kind(), format!("{}: {e}", path.display())))?;
    let lines: Vec<&str> = text.lines().collect();
    let chunks: Vec<&[&str]> = lines.chunks(CHUNK_LINES).collect();
    let workers = worker_count().min(chunks.len().max(1));
    let next = AtomicUsize::new(0);
    let finished = std::thread::scope(|scope| -> Result<usize> {
        let (sender, receiver) = sync_channel(workers * 2);
        for _ in 0..workers {
            let sender = sender.clone();
            let (next, chunks, map) = (&next, &chunks, &map);
            std::thread::Builder::new()
                .stack_size(STACK_BYTES)
                .spawn_scoped(scope, move || {
                    LARGE_STACK.set(true);
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some(chunk) = chunks.get(index) else {
                            break;
                        };
                        let mapped = chunk
                            .iter()
                            .enumerate()
                            .map(|(offset, line)| {
                                let number = index * CHUNK_LINES + offset + 1;
                                let card = parse_line(path, number, line)?;
                                Ok((number, map(number, card)?))
                            })
                            .collect::<Result<Vec<_>>>();
                        if sender.send((index, mapped)).is_err() {
                            break;
                        }
                    }
                })?;
        }
        drop(sender);
        let mut pending = BTreeMap::new();
        let mut wanted = 0;
        for (chunk, mapped) in receiver {
            pending.insert(chunk, mapped);
            while let Some(mapped) = pending.remove(&wanted) {
                for (number, value) in mapped? {
                    visit(number, value)?;
                }
                wanted += 1;
            }
        }
        Ok(wanted)
    })?;
    if finished != chunks.len() {
        return Err("corpus worker stopped before finishing".into());
    }
    Ok(())
}

/// Every record on the calling thread, in line order. See `map_cards`.
pub fn for_each_card(
    path: &Path,
    visit: impl FnMut(usize, OracleCard) -> Result<()>,
) -> Result<()> {
    map_cards(path, |_, card| Ok(card), visit)
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
    file: BufWriter<File>,
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
            file: BufWriter::with_capacity(1 << 20, file),
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
        map_cards(
            path,
            |_, card| {
                let mut forms = BTreeMap::new();
                crate::inspect::count(&crate::inspect::value(&card)?, &mut forms);
                Ok((forms, Identity::of(&card)?))
            },
            |_, (forms, identity): (BTreeMap<String, BTreeMap<String, usize>>, Identity)| {
                for (ty, variants) in forms {
                    let merged = result.forms.entry(ty).or_default();
                    for (variant, count) in variants {
                        *merged.entry(variant).or_default() += count;
                    }
                }
                result.records.push(identity);
                Ok(())
            },
        )?;
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
