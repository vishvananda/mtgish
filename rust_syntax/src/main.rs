//! Compatibility entry point for the existing just recipes.
use rust_syntax::corpus::Result;
#[cfg(any(feature = "ronl_to_bincode", feature = "ronl_to_pretty_ron"))]
use rust_syntax::corpus;
use std::io::Write;
#[cfg(any(feature = "ronl_to_bincode", feature = "ronl_to_pretty_ron"))]
use std::path::Path;

fn run() -> Result<()> {
    let modes = [
        cfg!(feature = "ronl_to_bincode"),
        cfg!(feature = "ronl_to_pretty_ron"),
        cfg!(feature = "write_out_json"),
    ];
    if modes.into_iter().filter(|enabled| *enabled).count() != 1 {
        return Err("select exactly one converter feature: ronl_to_bincode, ronl_to_pretty_ron, write_out_json".into());
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    #[cfg(feature = "ronl_to_bincode")]
    if modes[0] {
        if args.len() != 2 {
            return Err("usage: ronl_to_bincode <corpus.ronl> <output.bincode>".into());
        }
        let cards = corpus::read_cards(Path::new(&args[0]))?;
        let encoded = bincode::encode_to_vec(&cards, bincode::config::standard())?;
        let mut out = corpus::Artifact::create(Path::new(&args[1]))?;
        out.write_all(&encoded)?;
        return out.commit();
    }
    #[cfg(feature = "ronl_to_pretty_ron")]
    if modes[1] {
        if args.len() != 1 {
            return Err("usage: ronl_to_pretty_ron <corpus.ronl>".into());
        }
        let cards = corpus::read_cards(Path::new(&args[0]))?;
        let config = ron::ser::PrettyConfig::new()
            .indentor("  ".into())
            .separate_tuple_members(true);
        let mut out = std::io::BufWriter::new(std::io::stdout().lock());
        for card in cards {
            writeln!(
                out,
                "------------------\n{}",
                ron::ser::to_string_pretty(&card, config.clone())?
            )?;
        }
        out.flush()?;
        return Ok(());
    }
    #[cfg(feature = "write_out_json")]
    if modes[2] {
        if args.len() != 1 {
            return Err("usage: bincode_to_jsonl <corpus.bincode>".into());
        }
        let bytes = std::fs::read(&args[0])?;
        let (cards, used): (Vec<rust_syntax::mtg_types::OracleCard>, usize) =
            bincode::decode_from_slice(&bytes, bincode::config::standard())?;
        if used != bytes.len() {
            return Err("trailing bytes after bincode corpus".into());
        }
        if cards.is_empty() {
            return Err("card corpus is empty".into());
        }
        let mut out = std::io::BufWriter::new(std::io::stdout().lock());
        for card in cards {
            serde_json::to_writer(&mut out, &card)?;
            writeln!(out)?;
        }
        out.flush()?;
        return Ok(());
    }
    Err("no converter selected".into())
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}
