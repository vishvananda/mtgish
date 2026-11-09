use mtgish_ast::corpus::{self, Manifest, Result};
use std::io::Write;
use std::path::{Path, PathBuf};

fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [command,input] if command=="migrate-ir" => {
            let card=mtgish_ast::migrate::read_json(&std::fs::read_to_string(input)?)?;
            println!("{}",serde_json::to_string_pretty(&card)?);
        }
        [command,input,name] if command=="card-code" => {
            let source=corpus::read_cards(Path::new(input))?.into_iter().find(|c|corpus::Identity::of(c).is_ok_and(|id|id.names.iter().any(|n|n==name))).ok_or("card not found")?;
            let card=mtgish_ast::unified::lower(&source).card;
            print!("{}",mtgish_ast::human::write(&card)?);
        }
        [command,input] if command=="read-card-code" => {
            let card=mtgish_ast::human::read(&std::fs::read_to_string(input)?)?;
            println!("{}",serde_json::to_string_pretty(&card)?);
        }

        [command,input] if command=="canonical-ir" => {
            // The map addresses this exact input epoch. A migration needs its
            // own site map; silently upgrading first would mislabel the origins.
            let value=mtgish_ast::json_input::value(&std::fs::read_to_string(input)?)?;
            let card:mtgish_ast::ir::UnifiedCard=serde_json::from_value(value.clone())?;
            if serde_json::to_value(&card)?!=value {return Err("unknown or missing IR fields".into());}
            let mapped=mtgish_ast::canonical::mapped_unified(&card)?;
            println!("{}",serde_json::to_string_pretty(&mapped)?);
        }
        [command,input,mapped] if command=="transport-card-document" => {
            let document=mtgish_ast::human::document::Document::from_json(&std::fs::read_to_string(input)?)?;
            let mapped=mtgish_ast::canonical::MappedCard::from_json(&std::fs::read_to_string(mapped)?)?;
            let transported=document.transport(&mapped.card,&mapped.map)?;
            println!("{}",serde_json::to_string_pretty(&transported)?);
        }

        [command,input] if command=="format-card-code" => {
            let document=mtgish_ast::human::document::read(&std::fs::read_to_string(input)?)?;
            print!("{}",document.write()?);
        }
        [command,input] if command=="read-card-document" => {
            let document=mtgish_ast::human::document::read(&std::fs::read_to_string(input)?)?;
            println!("{}",serde_json::to_string_pretty(&document)?);
        }
        [command,input] if command=="format-card-document" => {
            let document=mtgish_ast::human::document::Document::from_json(&std::fs::read_to_string(input)?)?;
            print!("{}",document.write()?);
        }

        [command,report,source_type,output] if command=="pipeline-candidates" => {
            let report=serde_json::from_reader(std::io::BufReader::new(std::fs::File::open(report)?))?;
            let source=std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src/mtg_types.rs"))?;
            let candidates=mtgish_ast::pipeline::candidates::Candidates::from_report(&report,&source,source_type)?;
            corpus::write_json(Path::new(output),&candidates)?;
            eprintln!("{} observed {} forms grouped into {} Rust field signatures; mappings require semantic review",candidates.observed_forms,source_type,candidates.groups.len());
        }
        [command,report,source_type,oracle,output] if command=="primitive-draft" => {
            let report=serde_json::from_reader(std::io::BufReader::new(std::fs::File::open(report)?))?;
            let source=std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src/mtg_types.rs"))?;
            let oracle=mtgish_ast::pipeline::candidates::oracle_text(Path::new(oracle))?;
            let drafts=mtgish_ast::pipeline::candidates::draft_rows(&report,&source,source_type,&oracle)?;
            corpus::write_json(Path::new(output),&drafts)?;
            eprintln!("{} draft rows covering {} blocked records; review before adding any to rewrites/primitives.json5",drafts.len(),drafts.iter().map(|d|d.records).sum::<usize>());
        }
        [command,input,oracle,reviewed] if command=="render-back" => {
            let findings=mtgish_ast::render::audit(Path::new(input),Path::new(oracle),Path::new(reviewed))?;
            for finding in &findings {println!("{finding}");}
            eprintln!("{} mined pairs break on some unreviewed card",findings.len());
        }
        [command,input,reviewed] if command=="value-audit" => {
            let findings=mtgish_ast::value_audit::audit(Path::new(input),Path::new(reviewed))?;
            for finding in &findings {println!("{finding}");}
            if !findings.is_empty() {return Err(format!("{} source values are missing from complete records",findings.len()).into());}
            eprintln!("every kept source value is present");
        }
        [command,report,oracle] if command=="primitive-audit" => {
            let report=serde_json::from_reader(std::io::BufReader::new(std::fs::File::open(report)?))?;
            let oracle=mtgish_ast::pipeline::candidates::oracle_text(Path::new(oracle))?;
            let findings=mtgish_ast::pipeline::candidates::audit_primitives(&report,&oracle);
            for finding in &findings {println!("{finding}");}
            if !findings.is_empty() {return Err(format!("{} primitive uses disagree with Oracle text",findings.len()).into());}
            eprintln!("every primitive use matches its Oracle text");
        }
        [command,before,after,reviewed] if command=="coverage-compare" => {
            let read=|path:&String|->Result<mtgish_ast::report::Report>{Ok(serde_json::from_reader(std::io::BufReader::new(std::fs::File::open(path)?))?)};
            let delta=mtgish_ast::report::CoverageDelta::between(&read(before)?,&read(after)?);
            let reviewed=serde_json::from_reader(std::io::BufReader::new(std::fs::File::open(reviewed)?))?;
            eprintln!("{} complete cards gained, {} lost",delta.gained.len(),delta.lost.len());
            let unreviewed=delta.unreviewed_losses(&reviewed);
            if !unreviewed.is_empty() {return Err(format!("complete cards lost without review: {unreviewed:?}").into());}
        }
        [command,input,output] | [command,input,output,_] if command=="pipeline-check" => {
            let root=PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().to_owned();
            let baseline:Option<Manifest>=args.get(3).map(|path|->Result<Manifest>{Ok(serde_json::from_reader(std::io::BufReader::new(std::fs::File::open(path)?))?)}).transpose()?;
            let report=mtgish_ast::pipeline::report::Report::check(Path::new(input),&root,baseline.as_ref())?;
            corpus::write_json(Path::new(output),&report)?;
            eprintln!("{} / {} source records structurally preserved; {} action occurrences rewritten by {} rules into {} recipe forms; {} records fully semantic",report.structurally_preserved,report.total,report.rewritten_actions,report.rewrite_rules.len(),report.expanded_recipes.len(),report.semantic_complete);
            eprintln!("{} / {} records unified; {} complete",report.unified,report.total,report.semantic_complete);
            eprintln!("{} numeric occurrences consolidated by {} rules across {} records",report.rewritten_numbers,report.number_rules.len(),report.records_with_number_rewrites);
            eprintln!("{} selector occurrences consolidated by {} rules across {} records",report.rewritten_selectors,report.selector_rules.len(),report.records_with_selector_rewrites);
            eprintln!("{} program occurrences consolidated by {} rules across {} records",report.rewritten_programs,report.program_rules.len(),report.records_with_program_rewrites);
            eprintln!("{} mechanic occurrences expanded by {} reusable definitions across {} records", report.rewritten_mechanics,report.mechanic_rules.len(),report.records_with_mechanic_rewrites);
            eprintln!("{} cost and {} condition occurrences consolidated by {} rules across {} records",report.rewritten_costs,report.rewritten_conditions,report.operand_rules.len(),report.records_with_operand_rewrites);
            eprintln!("{} fixtures cover {} observed rule/operand features; final export still requires complete semantic lowering",report.fixtures.selected_lines.len(),report.fixtures.covered);
        }
        [command,input,report,output] if command=="pipeline-fixtures" => {
            let report=serde_json::from_reader(std::io::BufReader::new(std::fs::File::open(report)?))?;
            let count=mtgish_ast::pipeline::report::write_fixtures(Path::new(input),&report,Path::new(output))?;
            eprintln!("selected {count} source fixtures");
        }
        [command,input,output] if command=="pipeline-export" => {
            let intermediate:mtgish_ast::pipeline::Intermediate=serde_json::from_reader(std::io::BufReader::new(std::fs::File::open(input)?))?;
            if intermediate.format!="mtgish.pipeline.intermediate.v8" {return Err("unsupported intermediate format".into());}
            let card=mtgish_ast::pipeline::bind(&intermediate.structural).map_err(|errors|format!("staged card is not fully semantic: {errors:?}"))?;
            let mut artifact=corpus::Artifact::create(Path::new(output))?;
            writeln!(artifact,"{}",ron::ser::to_string(&card)?)?;artifact.commit()?;
        }
        [command,input,line,output] if command=="pipeline-record" => {
            use std::io::BufRead;
            let line:usize=line.parse()?;
            let text=std::io::BufReader::new(std::fs::File::open(input)?).lines().nth(line.checked_sub(1).ok_or("line numbers start at one")?).ok_or("line not found")??;
            let card=ron::from_str(&text)?;
            let intermediate=mtgish_ast::pipeline::Intermediate::new(&card)?;
            corpus::write_json(Path::new(output),&intermediate)?;
        }
        [command, input, output] if command == "baseline" => {
            let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().to_owned();
            let manifest = Manifest::build(Path::new(input), &root)?;
            corpus::write_json(Path::new(output), &manifest)?;
            eprintln!("validated {} records; {} Action forms; baseline {}", manifest.records.len(),
                manifest.forms.get("Action").map_or(0, |v| v.len()), output);
        }
        [command, input, baseline] if command == "check-source" => {
            let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().to_owned();
            let current = Manifest::build(Path::new(input), &root)?;
            let baseline = serde_json::from_reader(std::io::BufReader::new(std::fs::File::open(baseline)?))?;
            current.check_identities(&baseline)?;
            eprintln!("validated {} records; identities and layouts preserved", current.records.len());
        }
        [command, input, output] | [command, input, output, _] if command == "check"||command=="check-cantrip" => {
            let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().to_owned();
            let baseline: Option<Manifest> = args.get(3).map(|path| -> Result<Manifest> { Ok(serde_json::from_reader(std::io::BufReader::new(std::fs::File::open(path)?))?) }).transpose()?;
            let report = if command=="check-cantrip"{mtgish_ast::report::Report::check_with_backend(Path::new(input),&root,baseline.as_ref())?}else{mtgish_ast::report::Report::check(Path::new(input), &root, baseline.as_ref())?};
            corpus::write_json(Path::new(output), &report)?;
            eprintln!("{} / {} records fully normalized; {} generic operation kinds used; {} unsupported source forms reported", report.complete, report.total, report.operations.len(), report.unsupported.len());
            eprintln!("{} / {} records unified; {} unreduced forms across {} distinct",report.unified,report.total,report.named.values().sum::<usize>(),report.named.len());
            eprintln!("{} complete records depend on no primitive; {} distinct forms block the rest",report.complete_without_primitives,report.blocker_records.len());
            eprintln!("{} semantic families inventoried",report.forms.len());
            if report.backend_checked{eprintln!("{} records have all required Cantrip generator capabilities",report.backend_capable);}
            if report.complete != report.total { return Err("normalization is incomplete; see coverage report".into()); }
        }
        [command, input, output] if command == "export" => {
            let count = mtgish_ast::report::export_ron(Path::new(input), Path::new(output))?;
            eprintln!("exported {count} fully normalized records");
        }
        [command, input, output] if command == "export-unified" => {
            let count = mtgish_ast::report::export_unified(Path::new(input), Path::new(output))?;
            eprintln!("exported {count} unified records");
        }
        [command, input, output] if command == "cantrip" => {
            #[cfg(feature="cantrip")]
            {
            let count = mtgish_ast::cantrip::export(Path::new(input), Path::new(output))?;
            eprintln!("generated {count} Cantrip modules");
            }
            #[cfg(not(feature="cantrip"))]
            {let _=(input,output);return Err("Cantrip generation requires --features cantrip".into());}
        }
        [command, input, oracle, output] if command == "cantrip-files" => {
            #[cfg(feature="cantrip")]
            {
            // Every record, its file as `cantrip-tracked` writes it or its gap, with the same render cache.
            let cache = mtgish_ast::cantrip::pr_cache::default_dir();
            let count = mtgish_ast::cantrip::planned::lane_rows(Path::new(input), Path::new(oracle), Path::new(output), cache.as_deref())?;
            eprintln!("{count} records written");
            }
            #[cfg(not(feature="cantrip"))]
            {let _=(input,oracle,output);return Err("Cantrip generation requires --features cantrip".into());}
        }
        [command, input, oracle, output] if command == "cantrip-tracked" => {
            #[cfg(feature="cantrip")]
            {
            // pr_form's renders are kept in the target directory, or MTGISH_PR_FORM_CACHE (`off` for none).
            let cache = mtgish_ast::cantrip::pr_cache::default_dir();
            let count = mtgish_ast::cantrip::planned::tracked_cached(Path::new(input), Path::new(oracle), Path::new(output), cache.as_deref())?;
            println!("{count} tracked cards");
            }
            #[cfg(not(feature="cantrip"))]
            {let _=(input,oracle,output);return Err("Cantrip generation requires --features cantrip".into());}
        }
        [command, input, names @ ..] if command == "cantrip-module" && !names.is_empty() => {
            #[cfg(feature="cantrip")]
            for (name, module) in mtgish_ast::cantrip::modules_named(Path::new(input), names)? {
                println!("// ===== {name}\n{module}");
            }
            #[cfg(not(feature="cantrip"))]
            {let _=(input,names);return Err("Cantrip generation requires --features cantrip".into());}
        }
        [command, input, output] if command == "cantrip-gaps" => {
            #[cfg(feature="cantrip")]
            {
            let survey = mtgish_ast::cantrip::survey(Path::new(input))?;
            corpus::write_json(Path::new(output), &survey)?;
            eprintln!("{} / {} complete primitive-free records generate; {} more are other layouts", survey.capable, survey.complete_without_primitives, survey.other_layouts);
            for step in survey.greedy.iter().take(60) {
                eprintln!("{:>6} {:>6}  {}", step.unlocks, step.appears_on, step.feature);
            }
            }
            #[cfg(not(feature="cantrip"))]
            {let _=(input,output);return Err("Cantrip gaps require --features cantrip".into());}
        }
        [command, input, output] if command == "pretty-source" => {
            let cards = corpus::read_cards(Path::new(input))?;
            let mut out = corpus::Artifact::create(Path::new(output))?;
            let config = ron::ser::PrettyConfig::new().indentor("  ".into()).separate_tuple_members(true);
            for card in cards {
                writeln!(out, "------------------\n{}", ron::ser::to_string_pretty(&card, config.clone())?)?;
            }
            out.commit()?;
        }
        _ => return Err("usage: ast canonical-ir <card.ir.json>\n       ast transport-card-document <document.json> <mapped.json>\n       ast format-card-code <card.card>\n       ast read-card-document <card.card>\n       ast format-card-document <document.json>\n       ast migrate-ir <old-or-current.ir.json>\n       ast card-code <corpus.ronl> <card name>\n       ast read-card-code <card.card>\n       ast baseline <corpus.ronl> <manifest.json>\n       ast check-source <corpus.ronl> <manifest.json>\n       ast check <corpus.ronl> <report.json> [baseline.json]\n       ast pipeline-check <corpus.ronl> <report.json> [baseline.json]\n       ast pipeline-fixtures <corpus.ronl> <report.json> <subset.ronl>\n       ast pipeline-candidates <report.json> <source-enum> <candidates.json>\n       ast pipeline-record <corpus.ronl> <line> <intermediate.json>\n       ast pipeline-export <intermediate.json> <normalized.ronl>\n       ast check-cantrip <corpus.ronl> <report.json> [baseline.json] (requires --features cantrip)\n       ast export <corpus.ronl> <normalized.ronl>\n       ast export-unified <corpus.ronl> <unified.ronl>\n       ast primitive-draft <check-report> <source-type> <oracle.json> <drafts.json>\n       ast primitive-audit <check-report> <oracle.json>\n       ast render-back <corpus.ronl> <oracle.json> <reviewed.json>\n       ast cantrip <normalized.ronl> <output.rs>\n       ast cantrip-gaps <corpus.ronl> <survey.json> (requires --features cantrip)\n       ast cantrip-files <corpus.ronl> <oracle.json> <rows.jsonl> (requires --features cantrip)\n       ast cantrip-module <corpus.ronl> <card name>... (requires --features cantrip)\n       ast pretty-source <corpus.ronl> <output.ron>".into()),
    }
    Ok(())
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
