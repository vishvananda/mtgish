//! A generated card the way Cantrip's own files read, for a pull request.
//!
//! The generator writes for every card at once, so what it writes is regular and
//! wordy: a block around each read, each amount through a checked cast, one
//! player in a list and a loop over it. This pass reads that module as Rust and
//! says the same thing the way a card file says it, with the printed sentence
//! on each function, a name that says what it does, and the characteristics left
//! to the generated `oracle::` constant.
//!
//! Each rewrite here keeps what the code does. A module that does not parse
//! comes back as it went in.
use super::card_file::oracle_constant;
use quote::ToTokens;
use std::collections::{BTreeMap, BTreeSet};
use syn::visit::{self, Visit};
use syn::visit_mut::{self, VisitMut};
use syn::{parse_quote, Block, Expr, File, Item, Lit, Pat, Stmt};

/// Where a rendered card's characteristics come from.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Characteristics {
    /// Written out, for a crate with no content files.
    Inline,
    /// `..oracle::NAME`, for a card with its content file.
    Oracle,
}

/// `raw`, a generated module, as a card file. `printed` is the card's Oracle text.
pub fn render(raw: &str, name: &str, printed: &str, characteristics: Characteristics) -> Option<String> {
    let Ok(file) = syn::parse_str::<File>(raw) else {
        return Some(raw.to_string());
    };
    rendered(file, raw, name, printed, characteristics).map(written)
}

/// A card of two faces as one file, `raw` holding the second in `module`.
pub fn render_two_faces(raw: &str, module: &str, faces: [(&str, &str); 2]) -> Option<String> {
    render_faces(raw, &[module], &faces)
}

/// A card of several faces as one file, `raw` holding each in one of `modules`: its `CARD`
/// is named as the card reads it (`BACK`, or its own name), and they share one namespace.
/// `faces` are each face's name and Oracle text. The card itself may be neither face, as
/// a split card is. None where the module isn't there, or its name is no face's.
pub fn render_faces(raw: &str, modules: &[&str], faces: &[(&str, &str)]) -> Option<String> {
    let printed_name = |file: &File| {
        file.items.iter().find_map(|item| match item {
            Item::Static(s) if s.ident == "CARD" => match &*s.expr {
                Expr::Struct(card) => card.fields.iter().find_map(|f| match &f.expr {
                    Expr::Lit(lit) if text_of(&f.member) == "name" => match &lit.lit {
                        Lit::Str(name) => Some(name.value()),
                        _ => None,
                    },
                    _ => None,
                }),
                _ => None,
            },
            _ => None,
        })
    };
    let face_named = |name: &str| faces.iter().find(|face| face.0 == name).copied();
    let mut file = syn::parse_str::<File>(raw).ok()?;
    let mut parts: Vec<(&str, File, (&str, &str), String)> = Vec::new();
    for module in modules {
        let at = file
            .items
            .iter()
            .position(|item| matches!(item, Item::Mod(m) if m.ident == module))?;
        let Item::Mod(taken) = file.items.remove(at) else {
            return None;
        };
        let (_, items) = taken.content?;
        let items = items.into_iter().filter(|item| !matches!(item, Item::Use(_))).collect();
        let inner = File { shebang: None, attrs: Vec::new(), items };
        let face = face_named(&printed_name(&inner)?)?;
        let constant = match *module {
            "back" => "BACK".to_string(),
            _ => oracle_constant(face.0),
        };
        parts.push((module, inner, face, constant));
    }
    let name = printed_name(&file)?;
    let joined = faces.iter().map(|face| face.1).collect::<Vec<_>>().join("\n");
    let front = face_named(&name).unwrap_or((&name, &joined));
    let mut unparsed = prettyplease::unparse(&file);
    for (module, _, _, constant) in &parts {
        unparsed = unparsed.replace(&format!("{module}::CARD"), constant);
    }
    let oracle = Characteristics::Oracle;
    let mut merged = rendered(syn::parse_str(&unparsed).ok()?, &unparsed, front.0, front.1, oracle)?;
    for (module, inner, face, constant) in parts {
        let raw = format!("use cantrip_api::*;\n{}", prettyplease::unparse(&inner));
        // A prepare spell is no card and has no content file (722.3c).
        let characteristics = match module {
            "prepared" => Characteristics::Inline,
            _ => oracle,
        };
        let second = match rendered(syn::parse_str(&raw).ok()?, &raw, face.0, face.1, characteristics) {
            Some(file) => file,
            None => {
                let base: syn::Path = syn::parse_str(&format!("oracle::{}", oracle_constant(face.0))).ok()?;
                parse_quote!(pub static CARD: CardDef = CardDef { ..#base };)
            }
        };
        merged = merged_faces(merged, second, &constant)?;
    }
    Some(written(merged))
}

/// The second face's items before the first's, under one set of imports.
fn merged_faces(front: File, mut second: File, constant: &str) -> Option<File> {
    let name_of = |item: &Item| match item {
        Item::Fn(f) => Some(f.sig.ident.to_string()),
        Item::Static(s) => Some(s.ident.to_string()),
        Item::Const(c) => Some(c.ident.to_string()),
        _ => None,
    };
    // What a face's documentation says of a shared helper doesn't make it another one.
    let undocumented = |item: &Item| {
        let mut item = item.clone();
        match &mut item {
            Item::Fn(f) => f.attrs.clear(),
            Item::Static(s) => s.attrs.clear(),
            Item::Const(c) => c.attrs.clear(),
            _ => {}
        }
        text_of(&item)
    };
    // The same name for something else: the second face's takes a number.
    let mut renamed: BTreeMap<String, String> = second
        .items
        .iter()
        .filter_map(|item| name_of(item).map(|name| (name, undocumented(item))))
        .filter(|(name, body)| {
            !matches!(name.as_str(), "CARD" | "EXTRA")
                && front
                    .items
                    .iter()
                    .any(|item| name_of(item).as_ref() == Some(name) && undocumented(item) != *body)
        })
        .map(|(name, _)| {
            let new = format!("{name}_2");
            (name, new)
        })
        .collect();
    renamed.insert("CARD".to_string(), constant.to_string());
    rename(&mut second, &renamed);
    let (mut uses, mut mechanics, mut own, mut extra): (Vec<Item>, BTreeSet<String>, Vec<Item>, Vec<Expr>) =
        (Vec::new(), BTreeSet::new(), Vec::new(), Vec::new());
    let mut named: BTreeMap<String, String> = BTreeMap::new();
    let mut rest: Vec<Item> = Vec::new();
    for (i, face) in [second, front].into_iter().enumerate() {
        for mut item in face.items {
            if let Item::Use(_) = item {
                let line = text_of(&item).replace(' ', "");
                match line.strip_prefix("usecantrip_mechanics::") {
                    Some(leaves) => {
                        let leaves = leaves.trim_end_matches(';');
                        let leaves = leaves.strip_prefix('{').and_then(|l| l.strip_suffix('}')).unwrap_or(leaves);
                        mechanics.extend(leaves.split(',').filter(|l| !l.is_empty()).map(str::to_string));
                    }
                    None if uses.iter().any(|u| text_of(u).replace(' ', "") == line) => {}
                    None => uses.push(item),
                }
                continue;
            }
            if let Item::Static(s) = &mut item {
                if s.ident == "EXTRA" {
                    if let Expr::Reference(list) = &*s.expr {
                        if let Expr::Array(list) = &*list.expr {
                            for entry in &list.elems {
                                let through_a_module = text_of(entry).contains("::");
                                if !through_a_module && !extra.iter().any(|e| text_of(e) == text_of(entry)) {
                                    extra.push(entry.clone());
                                }
                            }
                        }
                    }
                    continue;
                }
                if i == 0 && s.ident != constant && text_of(&s.ty) == "CardDef" {
                    let ident = &s.ident;
                    let entry: Expr = parse_quote!(&#ident);
                    if !extra.iter().any(|e| text_of(e) == text_of(&entry)) {
                        extra.push(entry);
                    }
                }
            }
            if let Some(name) = name_of(&item) {
                let body = undocumented(&item);
                match named.get(&name) {
                    Some(seen) if *seen == body => continue,
                    Some(_) => return None,
                    None => {
                        named.insert(name, body);
                    }
                }
            }
            if i == 0 {
                own.push(item);
            } else {
                rest.push(item);
            }
        }
    }
    let mut items: Vec<Item> = Vec::new();
    let oracle = uses.iter().position(|u| text_of(u).contains("cantrip_oracle"));
    let leaves = mechanics.into_iter().collect::<Vec<_>>();
    let line = match leaves.as_slice() {
        [] => None,
        [one] => Some(format!("use cantrip_mechanics::{one};")),
        many => Some(format!("use cantrip_mechanics::{{{}}};", many.join(", "))),
    };
    let mut before = uses;
    if let Some(line) = line {
        let item: Item = syn::parse_str(&line).ok()?;
        before.insert(oracle.unwrap_or(before.len()), item);
    }
    items.extend(before);
    items.extend(own);
    items.extend(rest);
    if !extra.is_empty() {
        let list: syn::punctuated::Punctuated<Expr, syn::Token![,]> = extra.into_iter().collect();
        items.push(parse_quote!(pub static EXTRA: &[&CardDef] = &[#list];));
    }
    Some(File { shebang: None, attrs: Vec::new(), items })
}

/// A card's module as the file it will be, not yet written out.
fn rendered(
    mut file: File,
    raw: &str,
    name: &str,
    printed: &str,
    characteristics: Characteristics,
) -> Option<File> {
    let printed = &printed_as_cantrip(printed);
    let reads_data = raw.contains("item.data")
        || raw.contains("batch_by_data")
        || raw.contains("counted_by_amount")
        || raw.contains(".data[");
    let reads_subject = raw.contains("event_subject") || raw.contains("event_objects") || raw.contains("item.subject");
    let unread = matchers_whose_data_is_unread(&file);
    let reads_data = |function: &syn::ItemFn| reads_data && !unread.contains(&function.sig.ident.to_string());
    flatten_tokens(&mut file, &mut BTreeSet::new());
    use_shared_tokens(&mut file, name);
    for item in &mut file.items {
        strip_allows(item);
        if let Item::Fn(function) = item {
            let reads_data = reads_data(function);
            simplify_function(function, reads_data);
        }
    }
    use_shared_targets(&mut file);
    for item in &mut file.items {
        if let Item::Fn(function) = item {
            let reads_data = reads_data(function);
            simplify_function(function, reads_data);
        }
    }
    tidy_statics(&mut file);
    if !reads_subject {
        use_shared_triggers(&mut file);
    }
    use_shared_mana(&mut file);
    use_shared_effects(&mut file);
    drop_unreached(&mut file);
    drop_defaults(&mut file, characteristics);
    inline_target_lists(&mut file);
    say_printed_text(&mut file, printed);
    events_first(&mut file);
    ManaBySymbols.visit_file_mut(&mut file);
    if characteristics == Characteristics::Oracle
        && the_card(&mut file).is_some_and(|card| card.fields.is_empty())
        && file.items.iter().all(|item| !matches!(item, Item::Fn(_)))
    {
        return None;
    }
    let imports = shorten_paths(&mut file);
    let names = function_names(&file, printed);
    rename(&mut file, &names.renamed);
    document(&mut file, &names.quotes);
    document_card(&mut file, printed);
    let mut head = imports;
    if characteristics == Characteristics::Oracle {
        head.push("use cantrip_oracle as oracle;".to_string());
        set_oracle_base(&mut file, &oracle_constant(name));
    }
    file.items.splice(
        1.min(file.items.len())..1.min(file.items.len()),
        head.iter()
            .map(|line| syn::parse_str::<Item>(line).expect("an import")),
    );
    Some(file)
}

/// A rendered card as its text.
fn written(mut file: File) -> String {
    strip_literal_types(&mut file);
    let text = fields_on_their_own_lines(prettyplease::unparse(&file));
    spaced(let_else_on_one_line(&event_patterns_on_one_line(&text)))
}

/// The event functions whose abilities never read what they carry: `Fired::data` belongs to the ability it fires, not to the
/// file, and one ability reading it says nothing of the others. An event function goes in only if every `triggered` using it
/// has an effect that doesn't read `item.data`, by itself or through a function of the file, and no batching by data.
fn matchers_whose_data_is_unread(file: &File) -> BTreeSet<String> {
    let bodies: BTreeMap<String, String> = file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Fn(function) => Some((function.sig.ident.to_string(), text_of(function))),
            _ => None,
        })
        .collect();
    let says = |text: &str, name: &str| text.split(|c: char| !c.is_alphanumeric() && c != '_').any(|word| word == name);
    let mut reading: BTreeSet<&String> = bodies.iter().filter(|(_, body)| body.contains(". data [")).map(|(name, _)| name).collect();
    loop {
        let more: Vec<&String> = bodies
            .iter()
            .filter(|(name, body)| !reading.contains(name) && reading.iter().any(|read| says(body, read)))
            .map(|(name, _)| name)
            .collect();
        if more.is_empty() {
            break;
        }
        reading.extend(more);
    }
    struct Uses<'a> {
        used: BTreeMap<&'a str, bool>,
        bodies: &'a BTreeMap<String, String>,
        reading: &'a BTreeSet<&'a String>,
    }
    impl<'a> Uses<'a> {
        fn triggered(&mut self, call: &syn::ExprCall, batched: bool) {
            let (Some(matcher), Some(effect)) = (call.args.first(), call.args.iter().nth(1)) else { return };
            let (matcher, effect) = (text_of(matcher), text_of(effect));
            let words = |text: &str| text.split(|c: char| !c.is_alphanumeric() && c != '_').map(str::to_string).collect::<BTreeSet<_>>();
            let effect_words = words(&effect);
            let reads = batched || effect.contains(". data [") || self.reading.iter().any(|name| effect_words.contains(*name));
            for word in words(&matcher) {
                if let Some((name, _)) = self.bodies.get_key_value(&word) {
                    *self.used.entry(name).or_default() |= reads || self.reading.contains(name);
                }
            }
        }
    }
    impl<'ast> Visit<'ast> for Uses<'_> {
        fn visit_expr(&mut self, node: &'ast Expr) {
            let mut at = node;
            let mut batched = false;
            while let Expr::MethodCall(method) = at {
                let word = method.method.to_string();
                batched |= word == "batch_by_data" || word == "counted_by_amount";
                at = &method.receiver;
            }
            match at {
                Expr::Call(call) if text_of(&call.func).ends_with("triggered") => self.triggered(call, batched),
                _ => {}
            }
            visit::visit_expr(self, node);
        }
    }
    let mut uses = Uses { used: BTreeMap::new(), bodies: &bodies, reading: &reading };
    uses.visit_file(file);
    let unread = uses.used.iter().filter(|(_, reads)| !**reads).map(|(name, _)| name.to_string()).collect();
    unread
}

/// `if let Event::X { .. } = event { .. } None` says what to do with any other event, which a trigger on X's mask is never
/// handed, so the closing brace of the `if` is a line no game can run. The event goes first, on a line that always runs.
fn events_first(file: &mut File) {
    for item in &mut file.items {
        let Item::Fn(function) = item else { continue };
        let stmts = &mut function.block.stmts;
        let [.., Stmt::Expr(Expr::If(test), _), Stmt::Expr(tail, None)] = stmts.as_mut_slice() else { continue };
        let Expr::Let(guard) = &*test.cond else { continue };
        if ident_of(tail).as_deref() != Some("None") || test.else_branch.is_some() || ident_of(&guard.expr).as_deref() != Some("event") {
            continue;
        }
        let pattern = text_of(&guard.pat).replace(" :: ", "::").replace(" ,", ",");
        if "    ".len() + pattern.len() + " = event else { return None };".len() > 120 {
            continue;
        }
        let pat = &guard.pat;
        let first: Stmt = syn::parse_quote!(let #pat = event else { return None; };);
        let body = test.then_branch.stmts.clone();
        let at = stmts.len() - 2;
        stmts.splice(at..at + 1, std::iter::once(first).chain(body));
        // The body's last words may sit in a block of their own.
        let before = stmts.len() - 2;
        if let Stmt::Expr(Expr::Block(inner), _) = &stmts[before] {
            let inner = inner.block.stmts.clone();
            stmts.splice(before..before + 1, inner);
        }
        // A body that always returns leaves the `None` after it unreachable: its return is the function's value.
        if matches!(stmts.iter().rev().nth(1), Some(Stmt::Expr(Expr::Return(_), _))) {
            stmts.pop();
            tail_return(&mut function.block);
        }
    }
}

/// A cost as a card writes it, `mana("{3}{U}")`: a `ManaCost` spelled out has a line for each default, and a number in one is a
/// change a scenario is held to though the printed cost is all there is to say. Hybrid and Phyrexian symbols stay spelled out.
struct ManaBySymbols;

impl ManaBySymbols {
    fn number(expr: &Expr) -> Option<u32> {
        match expr {
            Expr::Lit(syn::ExprLit { lit: syn::Lit::Int(value), .. }) => value.base10_parse().ok(),
            _ => None,
        }
    }

    fn numbers(expr: &Expr) -> Option<Vec<u32>> {
        match expr {
            Expr::Array(array) => array.elems.iter().map(Self::number).collect(),
            Expr::Repeat(repeat) => Some(vec![Self::number(&repeat.expr)?; Self::number(&repeat.len)? as usize]),
            _ => None,
        }
    }

    fn printed(cost: &syn::ExprStruct) -> Option<String> {
        if cost.path.segments.last()?.ident != "ManaCost" {
            return None;
        }
        if let Some(rest) = &cost.rest {
            if !text_of(rest).replace(' ', "").ends_with("ManaCost::ZERO") {
                return None;
            }
        } else if cost.fields.len() != 6 {
            return None;
        }
        let (mut generic, mut x, mut symbols) = (0, 0, vec![0; 6]);
        for field in &cost.fields {
            let syn::Member::Named(name) = &field.member else { return None };
            match name.to_string().as_str() {
                "generic" => generic = Self::number(&field.expr)?,
                "x" => x = Self::number(&field.expr)?,
                "symbols" => symbols = Self::numbers(&field.expr)?,
                "hybrid" if !text_of(&field.expr).contains("Some") => {}
                "twobrid" | "phyrexian" if Self::numbers(&field.expr)?.iter().all(|&n| n == 0) => {}
                _ => return None,
            }
        }
        if symbols.len() != 6 || generic > 99 {
            return None;
        }
        let mut out = "{X}".repeat(x as usize);
        if generic > 0 {
            out += &format!("{{{generic}}}");
        }
        for (letter, count) in "WUBRGC".chars().zip(symbols) {
            out += &format!("{{{letter}}}").repeat(count as usize);
        }
        Some(out)
    }
}

impl VisitMut for ManaBySymbols {
    fn visit_expr_mut(&mut self, node: &mut Expr) {
        syn::visit_mut::visit_expr_mut(self, node);
        let Expr::Struct(cost) = node else { return };
        let Some(printed) = Self::printed(cost) else { return };
        *node = if printed.is_empty() {
            syn::parse_quote!(cantrip_api::ManaCost::ZERO)
        } else {
            syn::parse_quote!(cantrip_api::mana(#printed))
        };
    }
}

/// Oracle text with loyalty costs as `+1:`, not the data's `[+1]:`.
fn printed_as_cantrip(printed: &str) -> String {
    let lines: Vec<String> = printed
        .lines()
        .map(|line| {
            let cost = line
                .strip_prefix('[')
                .and_then(|rest| rest.split_once("]:"))
                .filter(|(cost, _)| cost.chars().all(|c| "+-\u{2212}X0123456789".contains(c)));
            match cost {
                Some((cost, rest)) => format!("{cost}:{rest}"),
                None => line.to_string(),
            }
        })
        .collect();
    lines.join("\n")
}

/// A blank line between the file's items, as a person leaves them.
fn spaced(source: String) -> String {
    let mut out = String::with_capacity(source.len() + 256);
    let mut before = "";
    for line in source.lines() {
        let opens = ["///", "fn ", "pub fn ", "static ", "pub static ", "const ", "pub const "]
            .iter()
            .any(|start| line.starts_with(start));
        if opens && !before.is_empty() && !before.starts_with("///") && !before.starts_with("use ") {
            out.push('\n');
        }
        if opens && before.starts_with("use ") {
            out.push('\n');
        }
        out.push_str(line);
        out.push('\n');
        before = line;
    }
    out
}

fn strip_allows(item: &mut Item) {
    let attributes = match item {
        Item::Fn(item) => &mut item.attrs,
        Item::Use(item) => &mut item.attrs,
        Item::Static(item) => &mut item.attrs,
        Item::Const(item) => &mut item.attrs,
        _ => return,
    };
    attributes.retain(|a| !a.path().is_ident("allow"));
}

fn text_of<T: ToTokens>(node: &T) -> String {
    node.to_token_stream().to_string()
}

fn ident_of(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Path(path) if path.qself.is_none() => path.path.get_ident().map(|i| i.to_string()),
        _ => None,
    }
}

/// How `name` is read in what was visited.
#[derive(Default)]
struct Uses<'a> {
    name: &'a str,
    reads: usize,
    in_macros: usize,
    rebound: bool,
}

/// How many times `name` stands in `tokens`; `x.name` is a field of `x` and doesn't count.
fn tokens_name(tokens: proc_macro2::TokenStream, name: &str) -> usize {
    let mut dots = 0;
    tokens
        .into_iter()
        .map(|token| {
            let count = match &token {
                proc_macro2::TokenTree::Ident(ident) => usize::from(dots != 1 && ident == name),
                proc_macro2::TokenTree::Group(group) => tokens_name(group.stream(), name),
                _ => 0,
            };
            let dot = matches!(&token, proc_macro2::TokenTree::Punct(p) if p.as_char() == '.');
            dots = if dot { dots + 1 } else { 0 };
            count
        })
        .sum()
}

/// Whether `pat` binds the variable `name`.
fn binds<T: ToTokens>(pat: &T, name: &str) -> bool {
    mentions(pat, name)
}

impl<'ast> Visit<'ast> for Uses<'_> {
    fn visit_expr_path(&mut self, node: &'ast syn::ExprPath) {
        if node.qself.is_none() && node.path.is_ident(self.name) {
            self.reads += 1;
        }
    }
    fn visit_macro(&mut self, node: &'ast syn::Macro) {
        if node.path.is_ident("matches") {
            if let Ok(parsed) = syn::parse2::<Matches>(node.tokens.clone()) {
                let mut inside = Uses {
                    name: self.name,
                    ..Uses::default()
                };
                inside.visit_expr(&parsed.value);
                if let (false, Some(guard)) = (binds(&parsed.pat, self.name), &parsed.guard) {
                    inside.visit_expr(guard);
                }
                self.in_macros += inside.reads + inside.in_macros;
                return;
            }
        }
        self.in_macros += tokens_name(node.tokens.clone(), self.name);
    }
    fn visit_pat_ident(&mut self, node: &'ast syn::PatIdent) {
        if node.ident == self.name {
            self.rebound = true;
        }
        visit::visit_pat_ident(self, node);
    }
    // A name bound again stands for something else from there on.
    fn visit_block(&mut self, node: &'ast Block) {
        self.visit_stmts(&node.stmts);
    }
    fn visit_expr_if(&mut self, node: &'ast syn::ExprIf) {
        if let Expr::Let(binding) = &*node.cond {
            if binds(&binding.pat, self.name) {
                self.rebound = true;
                self.visit_expr(&binding.expr);
                if let Some((_, otherwise)) = &node.else_branch {
                    self.visit_expr(otherwise);
                }
                return;
            }
        }
        visit::visit_expr_if(self, node);
    }
    fn visit_arm(&mut self, node: &'ast syn::Arm) {
        if binds(&node.pat, self.name) {
            self.rebound = true;
        } else {
            visit::visit_arm(self, node);
        }
    }
    fn visit_expr_closure(&mut self, node: &'ast syn::ExprClosure) {
        if node.inputs.iter().any(|input| binds(input, self.name)) {
            self.rebound = true;
        } else {
            visit::visit_expr_closure(self, node);
        }
    }
    fn visit_expr_for_loop(&mut self, node: &'ast syn::ExprForLoop) {
        self.visit_expr(&node.expr);
        if binds(&node.pat, self.name) {
            self.rebound = true;
        } else {
            self.visit_block(&node.body);
        }
    }
}

impl Uses<'_> {
    fn visit_stmts(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            self.visit_stmt(stmt);
            if matches!(stmt, Stmt::Local(local) if binds(&local.pat, self.name)) {
                self.rebound = true;
                break;
            }
        }
    }
    fn any(&self) -> bool {
        self.reads + self.in_macros > 0
    }
}

fn uses_in_stmts<'a>(stmts: &[Stmt], name: &'a str) -> Uses<'a> {
    let mut uses = Uses {
        name,
        ..Uses::default()
    };
    uses.visit_stmts(stmts);
    uses
}

fn mentions<T: ToTokens>(node: &T, name: &str) -> bool {
    tokens_name(node.to_token_stream(), name) > 0
}

/// Stands `with` in for the variable `name`.
struct Replace<'a> {
    name: &'a str,
    with: &'a Expr,
}

impl VisitMut for Replace<'_> {
    fn visit_expr_mut(&mut self, node: &mut Expr) {
        if ident_of(node).as_deref() == Some(self.name) {
            *node = self.with.clone();
        } else {
            visit_mut::visit_expr_mut(self, node);
        }
    }
    fn visit_field_value_mut(&mut self, node: &mut syn::FieldValue) {
        if node.colon_token.is_none() && ident_of(&node.expr).as_deref() == Some(self.name) {
            node.colon_token = Some(Default::default());
        }
        visit_mut::visit_field_value_mut(self, node);
    }
}

/// A value that reads the same wherever it stands: no game read, nothing computed.
fn is_constant(expr: &Expr) -> bool {
    match expr {
        Expr::Lit(_) => true,
        Expr::Path(path) => {
            let segments = &path.path.segments;
            segments.len() > 1
                || segments
                    .last()
                    .is_some_and(|s| s.ident.to_string().starts_with(char::is_uppercase))
        }
        Expr::Field(field) => {
            matches!(ident_of(&field.base).as_deref(), Some("item" | "asker"))
                && matches!(&field.member, syn::Member::Named(_))
        }
        Expr::Index(index) => {
            matches!(&*index.index, Expr::Lit(_)) && text_of(&index.expr) == "item . data"
        }
        Expr::Paren(paren) => is_constant(&paren.expr),
        Expr::Cast(cast) => is_constant(&cast.expr),
        Expr::Unary(unary) => matches!(unary.op, syn::UnOp::Neg(_)) && is_constant(&unary.expr),
        Expr::Call(call) => {
            matches!(&*call.func, Expr::Path(path) if path.path.segments.last().is_some_and(
                |s| s.ident.to_string().starts_with(char::is_uppercase)))
                && call.args.iter().all(is_constant)
        }
        _ => false,
    }
}

/// Whether a statement of `expr` alone does nothing, so can go. `is_pure` is about the
/// game and what a value can be left out for; here it must also be known that the
/// statement is not called for what it does to something else (`chars.set(..);`).
fn is_idle(expr: &Expr) -> bool {
    if !is_pure(expr) {
        return false;
    }
    match expr {
        Expr::Lit(_) | Expr::Path(_) | Expr::Field(_) | Expr::Tuple(_) | Expr::Array(_) => true,
        Expr::Paren(paren) => is_idle(&paren.expr),
        Expr::Reference(reference) => is_idle(&reference.expr),
        Expr::Cast(cast) => is_idle(&cast.expr),
        Expr::Unary(unary) => is_idle(&unary.expr),
        Expr::Binary(binary) => is_idle(&binary.left) && is_idle(&binary.right),
        // Reads to the end of a chain: nothing before it can be for its effect, as
        // the chain starts from a value and `is_pure` passed it.
        Expr::MethodCall(call) => {
            let read = text_of(expr);
            matches!(
                call.method.to_string().as_str(),
                "collect" | "clone" | "len" | "is_empty" | "contains" | "sum" | "count" | "any" | "all" | "first" | "last"
            ) || read.starts_with("g . state ()")
                || read.starts_with("g . history ()")
                || (call.method == "abs" && is_idle(&call.receiver))
        }
        Expr::Call(call) if text_of(&call.func).ends_with(":: from") => call.args.iter().all(is_idle),
        _ => false,
    }
}

/// Methods of a local list that change it.
const CHANGING_METHODS: &[&str] = &[
    "push", "extend", "extend_from_slice", "append", "insert", "remove", "pop", "retain", "sort", "sort_by",
    "sort_by_key", "sort_unstable", "dedup", "clear", "truncate", "drain", "swap", "reverse", "push_back", "next",
    "replace",
];

/// Whether evaluating `expr` can be left out: it never touches the game but to read,
/// changes nothing of its own and leaves nothing it is in.
fn is_pure(expr: &Expr) -> bool {
    if ident_of(expr).as_deref() == Some("g") {
        return true;
    }
    struct Touches(bool);
    impl<'ast> Visit<'ast> for Touches {
        fn visit_expr_path(&mut self, node: &'ast syn::ExprPath) {
            if node.path.is_ident("g") {
                self.0 = true;
            }
        }
        fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
            let read = text_of(&node.func);
            if !matches!(
                read.as_str(),
                "announced" | "targets :: object" | "targets :: player"
            ) && !read.ends_with("targets :: object")
                && !read.ends_with("targets :: player")
            {
                visit::visit_expr_call(self, node);
            }
        }
        fn visit_block(&mut self, node: &'ast Block) {
            if !matches!(node.stmts.first(), Some(stmt) if is_query_binding(stmt)) {
                visit::visit_block(self, node);
            }
        }
        fn visit_macro(&mut self, _: &'ast syn::Macro) {
            self.0 = true;
        }
        // Changing a local is doing something, and so is leaving.
        fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
            self.0 |= CHANGING_METHODS.contains(&node.method.to_string().as_str());
            let reads = matches!(
                node.method.to_string().as_str(),
                "state" | "chars" | "controller" | "obj" | "words" | "mana_value" | "history" | "target_legal"
                    | "card_id" | "is_on_battlefield" | "can_sacrifice"
            );
            if reads && text_of(&node.receiver) == "g" {
                node.args.iter().for_each(|a| self.visit_expr(a));
            } else {
                visit::visit_expr_method_call(self, node);
            }
        }
        fn visit_expr_reference(&mut self, node: &'ast syn::ExprReference) {
            self.0 |= node.mutability.is_some();
            visit::visit_expr_reference(self, node);
        }
        fn visit_expr_return(&mut self, _: &'ast syn::ExprReturn) {
            self.0 = true;
        }
        fn visit_expr_break(&mut self, _: &'ast syn::ExprBreak) {
            self.0 = true;
        }
        fn visit_expr_continue(&mut self, _: &'ast syn::ExprContinue) {
            self.0 = true;
        }
        fn visit_expr_try(&mut self, _: &'ast syn::ExprTry) {
            self.0 = true;
        }
        fn visit_expr_assign(&mut self, _: &'ast syn::ExprAssign) {
            self.0 = true;
        }
        fn visit_expr_binary(&mut self, node: &'ast syn::ExprBinary) {
            self.0 |= is_assignment(&node.op);
            visit::visit_expr_binary(self, node);
        }
    }
    let mut touches = Touches(false);
    touches.visit_expr(expr);
    !touches.0
}

/// `let q: &dyn Query = g;`
fn is_query_binding(stmt: &Stmt) -> bool {
    text_of(stmt) == "let q : & dyn Query = g ;"
}

fn eval(expr: &Expr) -> Option<i128> {
    match expr {
        Expr::Lit(lit) => match &lit.lit {
            Lit::Int(int) => int.base10_parse().ok(),
            _ => None,
        },
        Expr::Paren(paren) => eval(&paren.expr),
        Expr::Group(group) => eval(&group.expr),
        Expr::Unary(unary) if matches!(unary.op, syn::UnOp::Neg(_)) => Some(-eval(&unary.expr)?),
        Expr::Cast(cast) => {
            let value = eval(&cast.expr)?;
            let (low, high) = match text_of(&cast.ty).as_str() {
                "u8" => (0, u8::MAX as i128),
                "u16" => (0, u16::MAX as i128),
                "u32" => (0, u32::MAX as i128),
                "usize" | "u64" => (0, u64::MAX as i128),
                "i8" => (i8::MIN as i128, i8::MAX as i128),
                "i16" => (i16::MIN as i128, i16::MAX as i128),
                "i32" => (i32::MIN as i128, i32::MAX as i128),
                "i64" => (i64::MIN as i128, i64::MAX as i128),
                _ => return None,
            };
            (low <= value && value <= high).then_some(value)
        }
        Expr::MethodCall(call) if call.args.len() == 1 => {
            let (value, other) = (eval(&call.receiver)?, eval(&call.args[0])?);
            match call.method.to_string().as_str() {
                "max" => Some(value.max(other)),
                "min" => Some(value.min(other)),
                // Below zero it depends on the type, which this doesn't know.
                "saturating_sub" if value >= other => Some(value - other),
                _ => None,
            }
        }
        Expr::Binary(binary) => {
            let (left, right) = (eval(&binary.left)?, eval(&binary.right)?);
            match binary.op {
                syn::BinOp::Add(_) => left.checked_add(right),
                syn::BinOp::Sub(_) => left.checked_sub(right),
                syn::BinOp::Mul(_) => left.checked_mul(right),
                _ => None,
            }
        }
        _ => None,
    }
}

fn int(value: i128) -> Expr {
    let literal = proc_macro2::Literal::i128_unsuffixed(value.abs());
    if value < 0 {
        parse_quote!(-#literal)
    } else {
        parse_quote!(#literal)
    }
}

/// `value` with the type a cast gave it, so that a method read on it still finds one.
fn typed_int(value: i128, ty: &str) -> Expr {
    let literal: proc_macro2::TokenStream = format!("{}{ty}", value.abs()).parse().expect("a literal");
    if value < 0 {
        parse_quote!(-#literal)
    } else {
        parse_quote!(#literal)
    }
}

/// Literal types an argument's own parameter already says.
fn strip_literal_types(file: &mut File) {
    struct Strip;
    fn plain(expr: &mut Expr) {
        let is_suffixed = matches!(expr, Expr::Lit(lit) if matches!(&lit.lit, Lit::Int(int) if !int.suffix().is_empty()));
        if is_suffixed {
            if let Some(value) = eval(expr) {
                *expr = int(value);
            }
        }
    }
    impl VisitMut for Strip {
        fn visit_expr_call_mut(&mut self, node: &mut syn::ExprCall) {
            visit_mut::visit_expr_call_mut(self, node);
            if !text_of(&node.func).ends_with(":: from") {
                node.args.iter_mut().for_each(plain);
            }
        }
        fn visit_expr_method_call_mut(&mut self, node: &mut syn::ExprMethodCall) {
            visit_mut::visit_expr_method_call_mut(self, node);
            node.args.iter_mut().for_each(plain);
        }
        fn visit_expr_binary_mut(&mut self, node: &mut syn::ExprBinary) {
            visit_mut::visit_expr_binary_mut(self, node);
            plain(&mut node.right);
        }
        fn visit_field_value_mut(&mut self, node: &mut syn::FieldValue) {
            visit_mut::visit_field_value_mut(self, node);
            plain(&mut node.expr);
        }
    }
    Strip.visit_file_mut(file);
}

fn strip_parens(expr: &mut Expr) {
    while let Expr::Paren(paren) = expr {
        *expr = (*paren.expr).clone();
    }
}

/// Parentheses that say nothing where the expression stands.
fn strip_atom_parens(expr: &mut Expr) {
    if let Expr::Paren(paren) = expr {
        if matches!(
            &*paren.expr,
            Expr::Lit(_)
                | Expr::Path(_)
                | Expr::Field(_)
                | Expr::MethodCall(_)
                | Expr::Call(_)
                | Expr::Index(_)
                | Expr::Macro(_)
                | Expr::Paren(_)
        ) {
            strip_parens(expr);
        }
    }
}

/// `matches!(value, pat if guard)`.
struct Matches {
    value: Expr,
    pat: Pat,
    guard: Option<Expr>,
}

impl syn::parse::Parse for Matches {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let value = input.parse()?;
        input.parse::<syn::Token![,]>()?;
        let pat = Pat::parse_multi_with_leading_vert(input)?;
        let guard = if input.parse::<Option<syn::Token![if]>>()?.is_some() {
            Some(input.parse()?)
        } else {
            None
        };
        input.parse::<Option<syn::Token![,]>>()?;
        Ok(Matches { value, pat, guard })
    }
}

impl Matches {
    fn into_tokens(self) -> proc_macro2::TokenStream {
        let (value, pat) = (&self.value, &self.pat);
        match &self.guard {
            Some(guard) => quote::quote!(#value, #pat if #guard),
            None => quote::quote!(#value, #pat),
        }
    }
}

/// The value of a condition that has one whatever the game holds.
fn truth(expr: &Expr) -> Option<bool> {
    match expr {
        Expr::Lit(syn::ExprLit { lit: Lit::Bool(value), .. }) => Some(value.value),
        Expr::Paren(paren) => truth(&paren.expr),
        Expr::Group(group) => truth(&group.expr),
        Expr::Unary(unary) if matches!(unary.op, syn::UnOp::Not(_)) => truth(&unary.expr).map(|value| !value),
        Expr::Binary(binary) => {
            let (left, right) = (eval(&binary.left), eval(&binary.right));
            if let (Some(left), Some(right)) = (left, right) {
                return match binary.op {
                    syn::BinOp::Eq(_) => Some(left == right),
                    syn::BinOp::Ne(_) => Some(left != right),
                    syn::BinOp::Lt(_) => Some(left < right),
                    syn::BinOp::Le(_) => Some(left <= right),
                    syn::BinOp::Gt(_) => Some(left > right),
                    syn::BinOp::Ge(_) => Some(left >= right),
                    _ => None,
                };
            }
            match (binary.op, truth(&binary.left), truth(&binary.right)) {
                (syn::BinOp::And(_), Some(true), Some(value)) | (syn::BinOp::And(_), Some(value), Some(true)) => Some(value),
                (syn::BinOp::And(_), Some(false), _) => Some(false),
                (syn::BinOp::Or(_), Some(false), Some(value)) | (syn::BinOp::Or(_), Some(value), Some(false)) => Some(value),
                (syn::BinOp::Or(_), Some(true), _) => Some(true),
                _ => None,
            }
        }
        _ => None,
    }
}

/// What a condition that is settled leaves of its expression, and an iterator's filter
/// that keeps everything.
fn constant_condition(node: &Expr) -> Option<Expr> {
    match node {
        Expr::Binary(binary) if truth(node).is_some() && !matches!(binary.op, syn::BinOp::And(_) | syn::BinOp::Or(_)) => {
            let value = truth(node)?;
            Some(parse_quote!(#value))
        }
        Expr::Binary(binary) if matches!(binary.op, syn::BinOp::And(_) | syn::BinOp::Or(_)) => {
            let value = truth(node)?;
            Some(parse_quote!(#value))
        }
        Expr::If(branch) => match truth(&branch.cond)? {
            true => {
                let block = branch.then_branch.clone();
                Some(parse_quote!(#block))
            }
            false => match &branch.else_branch {
                Some((_, otherwise)) => Some((**otherwise).clone()),
                None => Some(parse_quote!({})),
            },
        },
        Expr::MethodCall(call) if call.method == "filter" && call.args.len() == 1 => match &call.args[0] {
            Expr::Closure(keep) if truth(&keep.body) == Some(true) => Some((*call.receiver).clone()),
            _ => None,
        },
        _ => None,
    }
}

/// Rewrites of one expression, whatever surrounds it.
struct Tidy {
    item_asker: bool,
    reads_data: bool,
}

impl VisitMut for Tidy {
    fn visit_expr_mut(&mut self, node: &mut Expr) {
        if let Expr::MethodCall(call) = node {
            if eval(&call.receiver).is_some() && eval(node).is_none() {
                if let Expr::MethodCall(call) = node {
                    call.args.iter_mut().for_each(|a| {
                        self.visit_expr_mut(a);
                        strip_parens(a);
                    });
                    strip_atom_parens(&mut call.receiver);
                }
                return;
            }
        }
        if let Expr::Call(call) = node {
            if text_of(&call.func).ends_with(":: from") && call.args.iter().any(|a| eval(a).is_some()) {
                return;
            }
        }
        visit_mut::visit_expr_mut(self, node);
        if let Some(value) = eval(node) {
            match node {
                Expr::Cast(cast) => {
                    let ty = text_of(&cast.ty);
                    *node = typed_int(value, &ty);
                    return;
                }
                Expr::Lit(lit) if matches!(&lit.lit, Lit::Int(int) if int.suffix() == "i64") => {
                    *node = int(value);
                    return;
                }
                Expr::Lit(_) => {}
                _ => {
                    *node = int(value);
                    return;
                }
            }
        }
        if let Some(folded) = constant_condition(node) {
            *node = folded;
            return;
        }
        match node {
            Expr::MethodCall(call) => {
                let method = call.method.to_string();
                let replaced: Option<Expr> = match method.as_str() {
                    // `Some(x).expect("a reference that always names a player")`
                    "expect" => some_of(&call.receiver),
                    // `Some(item.object).map_or(0, |o: ObjectId| ..)`: a default for an object
                    // that is always there, which no game reaches.
                    "map_or" if call.args.len() == 2 => some_of(&call.receiver)
                        .filter(|x| matches!(x, Expr::Path(_) | Expr::Field(_)))
                        .and_then(|x| applied(&call.args[1], &x)),
                    // A list collected to be walked at once
                    "into_iter" => match &*call.receiver {
                        Expr::MethodCall(collect) if collect.method == "collect" => {
                            match &*collect.receiver {
                                Expr::MethodCall(walk) if walk.method == "into_iter" => {
                                    Some((*collect.receiver).clone())
                                }
                                _ => None,
                            }
                        }
                        _ => None,
                    },
                    "targets" if text_of(&call.args) == "& []" => Some((*call.receiver).clone()),
                    "flat_map" if text_of(&call.args).ends_with("q . state () . opponents (p)") => {
                        match &*call.receiver {
                            Expr::MethodCall(walk) if walk.method == "into_iter" => {
                                some_of(&walk.receiver).map(|one| parse_quote!(q.state().opponents(#one)))
                            }
                            _ => None,
                        }
                    }
                    "bottom_in_random_order" if text_of(&call.receiver) == "g" => {
                        let (player, cards) = (&call.args[0], &call.args[1]);
                        Some(parse_quote!(
                            cantrip_mechanics::library::rest_to_bottom_in_random_order(g, #player, #cards)
                        ))
                    }
                    "and" if text_of(&call.receiver) == "Cost :: NONE" => Some(call.args[0].clone()),
                    "and" if text_of(&call.receiver).starts_with("Cost :: mana (ManaCost {") && is_zero_cost(&call.receiver) => {
                        Some(call.args[0].clone())
                    }
                    "and" if text_of(&call.args[0]).starts_with("Cost :: mana (ManaCost {") && is_zero_cost(&call.args[0]) => {
                        Some((*call.receiver).clone())
                    }
                    _ => None,
                };
                if let Some(replaced) = replaced {
                    *node = replaced;
                }
            }
            // `x.clamp(0, 65535) as u16`: a cast, not a limit a rule sets. So is
            // `n.min(i16::MAX.into()) as i16`, which `Number::amount` writes of a value never negative.
            Expr::Cast(cast) => {
                if let Expr::MethodCall(call) = &*cast.expr {
                    if call.method == "clamp" {
                        let value = (*call.receiver).clone();
                        let ty = cast.ty.clone();
                        let floor = eval(&call.args[0]) == Some(0);
                        let atom = matches!(
                            value,
                            Expr::Lit(_) | Expr::Path(_) | Expr::Field(_) | Expr::MethodCall(_)
                                | Expr::Call(_) | Expr::Index(_) | Expr::Paren(_)
                        );
                        *node = match (floor, atom) {
                            (true, true) => parse_quote!(#value.max(0) as #ty),
                            (true, false) => parse_quote!((#value).max(0) as #ty),
                            (false, true) => parse_quote!(#value as #ty),
                            (false, false) => parse_quote!((#value) as #ty),
                        };
                    } else if call.method == "min"
                        && call.args.len() == 1
                        && text_of(&call.args[0]) == format!("{} :: MAX . into ()", text_of(&cast.ty))
                    {
                        let mut value = (*call.receiver).clone();
                        strip_parens(&mut value);
                        let ty = cast.ty.clone();
                        *node = match matches!(
                            value,
                            Expr::Lit(_) | Expr::Path(_) | Expr::Field(_) | Expr::MethodCall(_)
                                | Expr::Call(_) | Expr::Index(_)
                        ) {
                            true => parse_quote!(#value as #ty),
                            false => parse_quote!((#value) as #ty),
                        };
                    }
                }
            }
            Expr::Call(call) if text_of(&call.func) == "Keywords :: of" && text_of(&call.args) == "& []" => {
                *node = parse_quote!(Keywords::NONE);
            }
            Expr::Call(call) if text_of(&call.func).ends_with("choose :: cards") && call.args.len() == 7 => {
                let (g, player, source, purpose) = (&call.args[0], &call.args[1], &call.args[2], &call.args[3]);
                let (among, count) = (&call.args[4], counted(&call.args[5], &call.args[6]));
                *node = parse_quote!(cantrip_mechanics::choose::select(
                    #g, #player, #source, #purpose, #among,
                    cantrip_mechanics::select::Selection::new(#count)
                ));
            }
            Expr::Call(call) if text_of(&call.func).ends_with("choose :: cards_from") && call.args.len() == 8 => {
                let (g, player, source, purpose) = (&call.args[0], &call.args[1], &call.args[2], &call.args[3]);
                let (shown, eligible) = (&call.args[4], &call.args[5]);
                let count = counted(&call.args[6], &call.args[7]);
                *node = parse_quote!(cantrip_mechanics::choose::select(
                    #g, #player, #source, #purpose, (#shown).clone(),
                    cantrip_mechanics::select::Selection::listed(#count, &#shown, #eligible)
                ));
            }
            Expr::Call(call) if text_of(&call.func).ends_with("choose :: card") && call.args.len() == 5 => {
                let (g, player, source, purpose) = (&call.args[0], &call.args[1], &call.args[2], &call.args[3]);
                let among = &call.args[4];
                *node = parse_quote!(cantrip_mechanics::choose::select(
                    #g, #player, #source, #purpose, #among,
                    cantrip_mechanics::select::Selection::new(cantrip_mechanics::select::Count::AsManyAsCan(1))
                ).map(|chosen| chosen.first().copied()));
            }
            Expr::Call(call) if text_of(&call.func).ends_with("choose :: card_from") && call.args.len() == 6 => {
                let (g, player, source, purpose) = (&call.args[0], &call.args[1], &call.args[2], &call.args[3]);
                let (shown, eligible) = (&call.args[4], &call.args[5]);
                *node = parse_quote!(cantrip_mechanics::choose::select(
                    #g, #player, #source, #purpose, (#shown).clone(),
                    cantrip_mechanics::select::Selection::listed(
                        cantrip_mechanics::select::Count::AsManyAsCan(1), &#shown, #eligible
                    )
                ).map(|chosen| chosen.first().copied()));
            }
            Expr::Call(call) if text_of(&call.func) == "Cost :: mana" && call.args.len() == 1 => {
                if let Some(printed) = printed_cost(&call.args[0]) {
                    *node = parse_quote!(cost(#printed));
                }
            }
            Expr::Struct(cost) if text_of(cost) == "Cost { tap : true , .. Cost :: NONE }" => {
                *node = parse_quote!(TAP);
            }
            // `{ let q: &dyn Query = g; value }` where the value reads nothing through `q`
            Expr::Block(block) if block.label.is_none() => {
                let stmts = &block.block.stmts;
                let tail = match stmts.as_slice() {
                    [only] => Some(only),
                    [first, tail] if is_query_binding(first) && !mentions(tail, "q") => Some(tail),
                    _ => None,
                };
                if let Some(Stmt::Expr(value, None)) = tail {
                    *node = value.clone();
                    strip_parens(node);
                }
            }
            // `x && true`
            Expr::Binary(binary) if matches!(binary.op, syn::BinOp::And(_)) => {
                if text_of(&binary.right) == "true" {
                    *node = (*binary.left).clone();
                } else if text_of(&binary.left) == "true" {
                    *node = (*binary.right).clone();
                }
            }
            // `Some(a) == Some(b)`
            Expr::Binary(binary) if matches!(binary.op, syn::BinOp::Eq(_)) => {
                if let (Some(left), Some(right)) = (some_of(&binary.left), some_of(&binary.right)) {
                    *binary.left = left;
                    *binary.right = right;
                }
            }
            // `asker.controller` when the asker is only this item's
            Expr::Field(field) if self.item_asker && ident_of(&field.base).as_deref() == Some("asker") => {
                match text_of(&field.member).as_str() {
                    "controller" => *node = parse_quote!(item.controller),
                    "source" => *node = parse_quote!(item.object),
                    _ => {}
                }
            }
            // `EventMask(EventMask::SPELL_CAST.0)`
            Expr::Call(call) if text_of(&call.func) == "EventMask" && call.args.len() == 1 => {
                if let Expr::Field(field) = &call.args[0] {
                    if text_of(&field.member) == "0" {
                        *node = (*field.base).clone();
                    }
                }
            }
            // `Fired { subject, data }` when nothing reads the data
            Expr::Struct(fired) if !self.reads_data && text_of(&fired.path) == "Fired" && fired.fields.len() == 2 => {
                let subject = fired
                    .fields
                    .iter()
                    .find(|f| text_of(&f.member) == "subject")
                    .map(|f| f.expr.clone());
                if let Some(subject) = subject {
                    *node = parse_quote!(Fired { subject: #subject, data: [0; 2] });
                }
            }
            Expr::Call(call) if text_of(&call.func) == "Some" && call.args.len() == 1 => {
                if let Expr::Struct(fired) = &call.args[0] {
                    if text_of(&fired.path) == "Fired" && text_of(fired).ends_with("data : [0 ; 2] }") {
                        if let Some(subject) = fired.fields.first().map(|f| f.expr.clone()) {
                            *node = parse_quote!(Fired::about(#subject));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    fn visit_local_init_mut(&mut self, node: &mut syn::LocalInit) {
        visit_mut::visit_local_init_mut(self, node);
        strip_parens(&mut node.expr);
    }
    fn visit_expr_if_mut(&mut self, node: &mut syn::ExprIf) {
        visit_mut::visit_expr_if_mut(self, node);
        strip_parens(&mut node.cond);
        if let Some((_, otherwise)) = &node.else_branch {
            if let Expr::If(branch) = &**otherwise {
                if truth(&branch.cond) == Some(false) {
                    node.else_branch = branch.else_branch.clone();
                }
            }
        }
    }
    fn visit_macro_mut(&mut self, node: &mut syn::Macro) {
        if node.path.is_ident("matches") {
            if let Ok(mut parsed) = syn::parse2::<Matches>(node.tokens.clone()) {
                if parsed.guard.as_ref().and_then(truth) == Some(true) {
                    parsed.guard = None;
                }
                self.visit_expr_mut(&mut parsed.value);
                if let Some(guard) = &mut parsed.guard {
                    self.visit_expr_mut(guard);
                    strip_parens(guard);
                }
                if parsed.guard.as_ref().and_then(truth) == Some(true) {
                    parsed.guard = None;
                }
                let guard = parsed.guard.as_ref().map(text_of).unwrap_or_default();
                drop_unread_bindings(&mut parsed.pat, |name| text_reads(&guard, name));
                node.tokens = parsed.into_tokens();
            }
        } else if node.path.is_ident("vec") {
            let parser = syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated;
            if let Ok(mut items) = syn::parse::Parser::parse2(parser, node.tokens.clone()) {
                items.iter_mut().for_each(|item| self.visit_expr_mut(item));
                node.tokens = quote::quote!(#items);
            }
        }
    }
    fn visit_arm_mut(&mut self, node: &mut syn::Arm) {
        visit_mut::visit_arm_mut(self, node);
        strip_parens(&mut node.body);
    }
    fn visit_expr_match_mut(&mut self, node: &mut syn::ExprMatch) {
        visit_mut::visit_expr_match_mut(self, node);
        strip_parens(&mut node.expr);
    }
    fn visit_expr_call_mut(&mut self, node: &mut syn::ExprCall) {
        visit_mut::visit_expr_call_mut(self, node);
        node.args.iter_mut().for_each(strip_parens);
    }
    fn visit_expr_method_call_mut(&mut self, node: &mut syn::ExprMethodCall) {
        visit_mut::visit_expr_method_call_mut(self, node);
        node.args.iter_mut().for_each(strip_parens);
        strip_atom_parens(&mut node.receiver);
    }
    fn visit_expr_cast_mut(&mut self, node: &mut syn::ExprCast) {
        visit_mut::visit_expr_cast_mut(self, node);
        strip_atom_parens(&mut node.expr);
    }
    fn visit_expr_array_mut(&mut self, node: &mut syn::ExprArray) {
        visit_mut::visit_expr_array_mut(self, node);
        node.elems.iter_mut().for_each(strip_parens);
    }
    fn visit_expr_tuple_mut(&mut self, node: &mut syn::ExprTuple) {
        visit_mut::visit_expr_tuple_mut(self, node);
        node.elems.iter_mut().for_each(strip_parens);
    }
    fn visit_expr_closure_mut(&mut self, node: &mut syn::ExprClosure) {
        visit_mut::visit_expr_closure_mut(self, node);
        strip_parens(&mut node.body);
    }
    fn visit_field_value_mut(&mut self, node: &mut syn::FieldValue) {
        visit_mut::visit_field_value_mut(self, node);
        strip_parens(&mut node.expr);
    }
    fn visit_expr_return_mut(&mut self, node: &mut syn::ExprReturn) {
        visit_mut::visit_expr_return_mut(self, node);
        if let Some(value) = &mut node.expr {
            strip_parens(value);
        }
    }
    fn visit_block_mut(&mut self, node: &mut Block) {
        visit_mut::visit_block_mut(self, node);
        if let Some(Stmt::Expr(tail, None)) = node.stmts.last_mut() {
            strip_parens(tail);
        }
    }
}

/// `Count` of `count` cards, or up to `count`, as `choose::cards` took them.
fn counted(count: &Expr, up_to: &Expr) -> Expr {
    match text_of(up_to).as_str() {
        "true" => parse_quote!(cantrip_mechanics::select::Count::UpTo(#count)),
        "false" => parse_quote!(cantrip_mechanics::select::Count::AsManyAsCan(#count)),
        _ => parse_quote!(if #up_to {
            cantrip_mechanics::select::Count::UpTo(#count)
        } else {
            cantrip_mechanics::select::Count::AsManyAsCan(#count)
        }),
    }
}

/// A mana cost literal as it is printed, for `cost("{2}{W}")`. Only plain symbols.
fn printed_cost(cost: &Expr) -> Option<String> {
    let Expr::Struct(cost) = cost else { return None };
    let field = |name: &str| {
        cost.fields
            .iter()
            .find(|f| text_of(&f.member) == name)
            .map(|f| f.expr.clone())
    };
    let plain = field("hybrid").is_some_and(|h| !text_of(&h).contains("Some"))
        && field("twobrid").is_some_and(|t| text_of(&t) == "[0 , 0 , 0 , 0 , 0]")
        && field("phyrexian").is_some_and(|t| text_of(&t) == "[0 , 0 , 0 , 0 , 0]");
    if !plain {
        return None;
    }
    let generic = eval(&field("generic")?)?;
    let x = eval(&field("x")?)?;
    let Expr::Array(symbols) = field("symbols")? else {
        return None;
    };
    let mut out = "{X}".repeat(x as usize);
    if generic > 0 {
        out.push_str(&format!("{{{generic}}}"));
    }
    for (symbol, count) in ["W", "U", "B", "R", "G", "C"].iter().zip(symbols.elems.iter()) {
        out.push_str(&format!("{{{symbol}}}").repeat(eval(count)? as usize));
    }
    (!out.is_empty()).then_some(out)
}

fn is_zero_cost(cost: &Expr) -> bool {
    let cost = text_of(cost);
    cost.contains("generic : 0 ,")
        && cost.contains("x : 0 ,")
        && cost.contains("symbols : [0 , 0 , 0 , 0 , 0 , 0]")
        && cost.contains("hybrid : [None , None , None , None , None , None]")
        && cost.contains("twobrid : [0 , 0 , 0 , 0 , 0]")
        && cost.contains("phyrexian : [0 , 0 , 0 , 0 , 0]")
}

/// `{ let o: T = x; body }`: the closure `|o: T| body` applied to `x`. None for a closure
/// whose body could leave it (`return`, `?`), or that takes its argument by a pattern.
fn applied(closure: &Expr, value: &Expr) -> Option<Expr> {
    let Expr::Closure(closure) = closure else {
        return None;
    };
    let body = text_of(&closure.body);
    if closure.inputs.len() != 1 || body.contains("return") || body.contains('?') {
        return None;
    }
    let input = &closure.inputs[0];
    match input {
        Pat::Ident(_) => {}
        Pat::Type(typed) if matches!(&*typed.pat, Pat::Ident(_)) => {}
        _ => return None,
    }
    Some(match &*closure.body {
        Expr::Block(block) => {
            let statements = &block.block.stmts;
            parse_quote!({ let #input = #value; #(#statements)* })
        }
        other => parse_quote!({ let #input = #value; #other }),
    })
}

/// `x` of `Some(x)`.
fn some_of(expr: &Expr) -> Option<Expr> {
    match expr {
        Expr::Paren(paren) => some_of(&paren.expr),
        Expr::Call(call) if text_of(&call.func) == "Some" && call.args.len() == 1 => {
            Some(call.args[0].clone())
        }
        _ => None,
    }
}

/// `x` of `Some(x).into_iter().collect::<Vec<_>>()`, one value in a list.
fn only_in_list(expr: &Expr) -> Option<Expr> {
    let Expr::MethodCall(collect) = expr else {
        return None;
    };
    if collect.method != "collect" {
        return None;
    }
    let Expr::MethodCall(into_iter) = &*collect.receiver else {
        return None;
    };
    if into_iter.method != "into_iter" {
        return None;
    }
    some_of(&into_iter.receiver)
}

/// The name a `let` binds, with no `mut` and no destructuring.
fn plain_binding(local: &syn::Local) -> Option<String> {
    let pat = match &local.pat {
        Pat::Type(typed) => &*typed.pat,
        other => other,
    };
    match pat {
        Pat::Ident(ident)
            if ident.mutability.is_none() && ident.by_ref.is_none() && ident.subpat.is_none() =>
        {
            Some(ident.ident.to_string())
        }
        _ => None,
    }
}

/// The variable a `for` over `name` binds: `for x in name` or `for &x in &name`.
fn loop_over<'a>(looped: &'a syn::ExprForLoop, name: &str) -> Option<&'a syn::PatIdent> {
    let over = match &*looped.expr {
        Expr::Reference(reference) => &*reference.expr,
        other => other,
    };
    if ident_of(over).as_deref() != Some(name) {
        return None;
    }
    let pat = match &*looped.pat {
        Pat::Reference(reference) => &*reference.pat,
        other => other,
    };
    match pat {
        Pat::Ident(ident) => Some(ident),
        _ => None,
    }
}

fn breaks_out(block: &Block) -> bool {
    struct Finds(bool);
    impl<'ast> Visit<'ast> for Finds {
        fn visit_expr_break(&mut self, _: &'ast syn::ExprBreak) {
            self.0 = true;
        }
        fn visit_expr_continue(&mut self, _: &'ast syn::ExprContinue) {
            self.0 = true;
        }
    }
    let mut finds = Finds(false);
    finds.visit_block(block);
    finds.0
}

/// `body` in place of a statement, in its own braces if it binds anything that
/// could collide with what follows it.
fn spliced(body: Block, after: &[Stmt]) -> Vec<Stmt> {
    let collides = body.stmts.iter().any(|stmt| match stmt {
        Stmt::Local(local) => {
            let names = text_of(&local.pat);
            after.iter().any(|later| {
                names
                    .split(|c: char| !c.is_alphanumeric() && c != '_')
                    .any(|name| !name.is_empty() && mentions(later, name))
            })
        }
        _ => false,
    });
    if collides {
        vec![Stmt::Expr(
            Expr::Block(syn::ExprBlock {
                attrs: vec![],
                label: None,
                block: body,
            }),
            None,
        )]
    } else {
        body.stmts
    }
}

/// The first `x.expect(..)` a statement evaluates itself, as `named`: the `x` is
/// returned. Nested blocks and closures keep theirs, for their own statements.
struct TakeExpect<'a> {
    named: &'a syn::Ident,
    taken: Option<Expr>,
}

impl VisitMut for TakeExpect<'_> {
    fn visit_block_mut(&mut self, _: &mut Block) {}
    fn visit_expr_closure_mut(&mut self, _: &mut syn::ExprClosure) {}
    fn visit_expr_mut(&mut self, node: &mut Expr) {
        if self.taken.is_some() {
            return;
        }
        if let Expr::MethodCall(call) = node {
            if call.method == "expect" {
                self.taken = Some((*call.receiver).clone());
                let named = self.named;
                *node = parse_quote!(#named);
                return;
            }
        }
        visit_mut::visit_expr_mut(self, node);
    }
}

/// The variable an expression changes in place: `x`, `x[0]`, `x.y`, `*x`.
fn root_of(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Index(index) => root_of(&index.expr),
        Expr::Field(field) => root_of(&field.base),
        Expr::Paren(paren) => root_of(&paren.expr),
        Expr::Unary(unary) => root_of(&unary.expr),
        other => ident_of(other),
    }
}

fn is_assignment(op: &syn::BinOp) -> bool {
    matches!(
        op,
        syn::BinOp::AddAssign(_)
            | syn::BinOp::SubAssign(_)
            | syn::BinOp::MulAssign(_)
            | syn::BinOp::DivAssign(_)
            | syn::BinOp::RemAssign(_)
            | syn::BinOp::BitOrAssign(_)
            | syn::BinOp::BitAndAssign(_)
            | syn::BinOp::BitXorAssign(_)
            | syn::BinOp::ShlAssign(_)
            | syn::BinOp::ShrAssign(_)
    )
}

/// Methods that leave what they are called on as it was.
const READING_METHODS: &[&str] = &[
    "iter", "len", "is_empty", "contains", "clone", "get", "first", "last", "into_iter", "to_vec", "copied", "cloned",
    "is_some", "is_none", "player", "object",
];

/// Whether `name` is assigned, borrowed mutably, or has anything but a reading method
/// called on it.
fn is_changed(stmts: &[Stmt], name: &str) -> bool {
    struct Finds<'a>(&'a str, bool);
    impl<'ast> Visit<'ast> for Finds<'_> {
        fn visit_expr_assign(&mut self, node: &'ast syn::ExprAssign) {
            self.1 |= root_of(&node.left).as_deref() == Some(self.0);
            visit::visit_expr_assign(self, node);
        }
        fn visit_expr_binary(&mut self, node: &'ast syn::ExprBinary) {
            self.1 |= is_assignment(&node.op) && root_of(&node.left).as_deref() == Some(self.0);
            visit::visit_expr_binary(self, node);
        }
        fn visit_expr_reference(&mut self, node: &'ast syn::ExprReference) {
            self.1 |= node.mutability.is_some() && root_of(&node.expr).as_deref() == Some(self.0);
            visit::visit_expr_reference(self, node);
        }
        fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
            self.1 |= root_of(&node.receiver).as_deref() == Some(self.0)
                && !READING_METHODS.contains(&node.method.to_string().as_str());
            visit::visit_expr_method_call(self, node);
        }
        fn visit_macro(&mut self, node: &'ast syn::Macro) {
            let reads = node.path.is_ident("vec") || node.path.is_ident("matches");
            self.1 |= !reads && tokens_name(node.tokens.clone(), self.0) > 0;
        }
    }
    let mut finds = Finds(name, false);
    stmts.iter().for_each(|stmt| finds.visit_stmt(stmt));
    finds.1
}

/// How many times `name` is assigned, and how many other times it is read.
fn assignments(stmts: &[Stmt], name: &str) -> (usize, usize) {
    struct Counts<'a>(&'a str, usize, usize);
    impl<'ast> Visit<'ast> for Counts<'_> {
        fn visit_expr_assign(&mut self, node: &'ast syn::ExprAssign) {
            if ident_of(&node.left).as_deref() == Some(self.0) {
                self.1 += 1;
                self.visit_expr(&node.right);
            } else {
                visit::visit_expr_assign(self, node);
            }
        }
        fn visit_expr_binary(&mut self, node: &'ast syn::ExprBinary) {
            if is_assignment(&node.op) && ident_of(&node.left).as_deref() == Some(self.0) {
                self.1 += 1;
                self.visit_expr(&node.right);
            } else {
                visit::visit_expr_binary(self, node);
            }
        }
        fn visit_expr_path(&mut self, node: &'ast syn::ExprPath) {
            self.2 += usize::from(node.qself.is_none() && node.path.is_ident(self.0));
        }
        fn visit_macro(&mut self, node: &'ast syn::Macro) {
            self.2 += tokens_name(node.tokens.clone(), self.0);
        }
    }
    let mut counts = Counts(name, 0, 0);
    stmts.iter().for_each(|stmt| counts.visit_stmt(stmt));
    (counts.1, counts.2)
}

/// Each assignment to `name` as what was assigned.
struct OnlyTheValue<'a>(&'a str);

impl VisitMut for OnlyTheValue<'_> {
    fn visit_expr_mut(&mut self, node: &mut Expr) {
        visit_mut::visit_expr_mut(self, node);
        let value = match node {
            Expr::Assign(assign) if ident_of(&assign.left).as_deref() == Some(self.0) => (*assign.right).clone(),
            Expr::Binary(binary) if is_assignment(&binary.op) && ident_of(&binary.left).as_deref() == Some(self.0) => {
                (*binary.right).clone()
            }
            _ => return,
        };
        *node = value;
    }
}

/// `let (a, b) = ..` without what nothing reads. True if it changed anything.
fn drop_unread_elements(block: &mut Block, i: usize) -> bool {
    let (before, later) = block.stmts.split_at_mut(i + 1);
    let Some(Stmt::Local(local)) = before.last_mut() else {
        return false;
    };
    let Some(init) = local.init.as_mut().filter(|init| init.diverge.is_none()) else {
        return false;
    };
    let typed = matches!(local.pat, Pat::Type(_));
    let tuple = match &mut local.pat {
        Pat::Type(typed) => &mut *typed.pat,
        other => other,
    };
    let Pat::Tuple(tuple) = tuple else {
        return false;
    };
    let unread: Vec<bool> = tuple
        .elems
        .iter()
        .map(|elem| match elem {
            Pat::Ident(ident) if ident.subpat.is_none() => {
                let name = ident.ident.to_string();
                let uses = uses_in_stmts(later, &name);
                uses.reads + uses.in_macros == 0
            }
            _ => false,
        })
        .collect();
    if !unread.contains(&true) {
        return false;
    }
    let pairs_up = !typed && matches!(&*init.expr, Expr::Tuple(values) if values.elems.len() == tuple.elems.len());
    if pairs_up {
        let Expr::Tuple(values) = &mut *init.expr else {
            return false;
        };
        let keep: Vec<bool> = values.elems.iter().zip(&unread).map(|(value, unread)| !*unread || !is_pure(value)).collect();
        if keep.contains(&false) {
            let elems: Vec<Pat> = tuple.elems.iter().cloned().zip(&keep).filter(|(_, keep)| **keep).map(|(p, _)| p).collect();
            let kept: Vec<Expr> = values.elems.iter().cloned().zip(&keep).filter(|(_, keep)| **keep).map(|(v, _)| v).collect();
            match (elems.as_slice(), kept.as_slice()) {
                ([], _) => {
                    block.stmts.remove(i);
                    return true;
                }
                ([pat], [value]) => {
                    let (pat, value) = (pat.clone(), value.clone());
                    *init.expr = value;
                    local.pat = pat;
                }
                _ => {
                    tuple.elems = elems.into_iter().collect();
                    values.elems = kept.into_iter().collect();
                }
            }
            return true;
        }
    }
    for (elem, unread) in tuple.elems.iter_mut().zip(&unread) {
        if *unread {
            *elem = wild();
        }
    }
    true
}

/// Rewrites of a block's statements in order. Returns whether anything changed.
fn simplify_stmts(block: &mut Block, returns_result: bool) -> bool {
    let mut changed = false;
    let mut i = 0;
    while i < block.stmts.len() {
        let tail = i + 1 == block.stmts.len();
        // `let color = chosen.expect(..);`: the engine answered, so no card crashes on it.
        if let Stmt::Local(local) = &block.stmts[i] {
            if let (Some(name), Some(init)) = (plain_binding(local), &local.init) {
                if let Expr::MethodCall(call) = &*init.expr {
                    if call.method == "expect" {
                        let (variable, option) = (syn::Ident::new(&name, proc_macro2::Span::call_site()), &call.receiver);
                        let done = if returns_result { quote::quote!(return Ok(());) } else { quote::quote!(return;) };
                        block.stmts[i] = parse_quote!(let Some(#variable) = #option else { #done };);
                        changed = true;
                    }
                }
            }
        }
        // The same, inside a read.
        if let Stmt::Local(local) = &block.stmts[i] {
            if let (Some(name), Some(init)) = (plain_binding(local), &local.init) {
                if let Expr::Block(read) = &*init.expr {
                    if let Some(Stmt::Expr(Expr::MethodCall(call), None)) = read.block.stmts.last() {
                        if call.method == "expect" && read.block.stmts.len() == 2 {
                            let (variable, option) = (syn::Ident::new(&name, proc_macro2::Span::call_site()), &call.receiver);
                            let first = &read.block.stmts[0];
                            let done = if returns_result { quote::quote!(return Ok(());) } else { quote::quote!(return;) };
                            block.stmts[i] = parse_quote!(let Some(#variable) = ({ #first #option }) else { #done };);
                            changed = true;
                        }
                    }
                }
            }
        }
        // `f(chosen.expect(..))`: a card never crashes, so it stops where it can't go on.
        let named = syn::Ident::new(&format!("named_{i}"), proc_macro2::Span::call_site());
        let mut take = TakeExpect {
            named: &named,
            taken: None,
        };
        take.visit_stmt_mut(&mut block.stmts[i]);
        if let Some(option) = take.taken {
            let done = if returns_result { quote::quote!(return Ok(());) } else { quote::quote!(return;) };
            block.stmts.insert(i, parse_quote!(let Some(#named) = #option else { #done };));
            changed = true;
            continue;
        }
        if drop_unread_elements(block, i) {
            changed = true;
            continue;
        }
        // `let mut x` that nothing changes, or that is only ever assigned.
        let (before, later) = block.stmts.split_at_mut(i + 1);
        if let Some(Stmt::Local(local)) = before.last_mut() {
            let pat = match &mut local.pat {
                Pat::Type(typed) => &mut *typed.pat,
                other => other,
            };
            if let Pat::Ident(ident) = pat {
                if ident.mutability.is_some() && ident.subpat.is_none() {
                    let name = ident.ident.to_string();
                    let (assigned, read) = assignments(later, &name);
                    if assigned > 0 && read == 0 {
                        block.stmts.remove(i);
                        let mut value = OnlyTheValue(&name);
                        for stmt in &mut block.stmts[i..] {
                            value.visit_stmt_mut(stmt);
                        }
                        changed = true;
                        continue;
                    }
                    if !is_changed(later, &name) {
                        ident.mutability = None;
                        changed = true;
                    }
                    // The first thing done with it is to set it: that is where it starts.
                    let first = later.iter().position(|stmt| mentions(stmt, &name));
                    if let Some(Stmt::Expr(Expr::Assign(assign), Some(_))) = first.map(|at| &later[at]) {
                        if ident_of(&assign.left).as_deref() == Some(&name) && !mentions(&assign.right, &name) {
                            let value = (*assign.right).clone();
                            let at = i + 1 + first.expect("the statement just matched");
                            let mut start = local.clone();
                            start.init.as_mut().expect("a `let` with a value").expr = Box::new(value);
                            block.stmts[at] = Stmt::Local(start);
                            block.stmts.remove(i);
                            changed = true;
                            continue;
                        }
                    }
                }
            }
        }
        // A list never added to, given to the one verb that takes it.
        if let Stmt::Local(local) = &block.stmts[i] {
            if let (Some(name), Some(init)) = (plain_binding(local), &local.init) {
                if text_of(&init.expr) == "Vec :: new ()" {
                    let verb = format!("g . add_counters_together (& {name}) ;");
                    let at = block.stmts[i + 1..].iter().position(|stmt| text_of(stmt) == verb);
                    if let (Some(at), 1) = (at, uses_in_stmts(&block.stmts[i + 1..], &name).reads) {
                        block.stmts.remove(i + 1 + at);
                        block.stmts.remove(i);
                        changed = true;
                        continue;
                    }
                }
            }
        }
        // `let z = seen; if let Proposal::ZoneChange(z) = proposal { .. }`: the name is taken back.
        if let (Stmt::Local(local), Some(Stmt::Expr(Expr::If(branch), _))) = (&block.stmts[i], block.stmts.get(i + 1)) {
            if let (Some(name), Some(init)) = (plain_binding(local), &local.init) {
                if let Expr::Let(binding) = &*branch.cond {
                    if is_pure(&init.expr)
                        && mentions(&binding.pat, &name)
                        && !mentions(&binding.expr, &name)
                        && branch.else_branch.is_none()
                        && !block.stmts[i + 2..].iter().any(|stmt| mentions(stmt, &name))
                    {
                        block.stmts.remove(i);
                        changed = true;
                        continue;
                    }
                }
            }
        }
        // After a `return`, nothing runs.
        if matches!(&block.stmts[i], Stmt::Expr(Expr::Return(_), _)) && !tail {
            block.stmts.truncate(i + 1);
            changed = true;
            continue;
        }
        // `done += g.damage_together(&x).iter().map(..).sum();`, the total no one wanted.
        if let Stmt::Expr(Expr::MethodCall(sum), Some(_)) = &block.stmts[i] {
            if sum.method == "sum" {
                if let Expr::MethodCall(mapped) = &*sum.receiver {
                    if let (true, Expr::MethodCall(iter)) = (mapped.method == "map", &*mapped.receiver) {
                        if iter.method == "iter" {
                            let value = (*iter.receiver).clone();
                            block.stmts[i] = Stmt::Expr(value, Some(Default::default()));
                            changed = true;
                            continue;
                        }
                    }
                }
            }
        }
        // `let Some(x) = .. else { .. }` of an `x` nothing reads.
        let (before, later) = block.stmts.split_at_mut(i + 1);
        if let Some(Stmt::Local(local)) = before.last_mut() {
            if local.init.as_ref().is_some_and(|init| init.diverge.is_some()) {
                let unread = text_of(&local.pat);
                drop_unread_bindings(&mut local.pat, |name| uses_in_stmts(later, name).any());
                changed |= text_of(&local.pat) != unread;
            }
        }
        // `i64::from(x);`: what the conversion was asked for, nothing reads it.
        if let Stmt::Expr(Expr::Call(call), Some(_)) = &block.stmts[i] {
            if text_of(&call.func).ends_with(":: from") && call.args.len() == 1 {
                let value = call.args[0].clone();
                block.stmts[i] = parse_quote!(let _ = #value;);
                changed = true;
                continue;
            }
        }
        // An expression no one reads, with nothing to do.
        if let Stmt::Expr(expr, semi) = &block.stmts[i] {
            if (semi.is_some() || !tail) && is_idle(expr) {
                block.stmts.remove(i);
                changed = true;
                continue;
            }
        }
        // A block standing as a statement: no value, and no braces if nothing needs them.
        if let Stmt::Expr(Expr::Block(inner), semi) = &mut block.stmts[i] {
            if inner.label.is_none() && inner.attrs.is_empty() && (semi.is_some() || !tail) {
                if let Some(Stmt::Expr(value, None)) = inner.block.stmts.last() {
                    if is_idle(value) {
                        inner.block.stmts.pop();
                        changed = true;
                    }
                }
                if !matches!(inner.block.stmts.last(), Some(Stmt::Expr(_, None))) {
                    let body = inner.block.clone();
                    let replacement = spliced(body, &block.stmts[i + 1..]);
                    if !matches!(replacement.as_slice(), [Stmt::Expr(Expr::Block(_), _)]) {
                        block.stmts.splice(i..=i, replacement);
                        changed = true;
                        continue;
                    }
                }
            }
        }
        // A `let`
        if let Stmt::Local(local) = &block.stmts[i] {
            if let (Some(name), Some(init)) = (plain_binding(local), &local.init) {
                if init.diverge.is_none() {
                    let value = (*init.expr).clone();
                    let later = &block.stmts[i + 1..];
                    let uses = uses_in_stmts(later, &name);
                    if uses.reads + uses.in_macros == 0 {
                        let keep = (!is_pure(&value)).then(|| {
                            if text_of(&value).ends_with(". collect ()") {
                                parse_quote!(let _: Vec<_> = #value;)
                            } else {
                                Stmt::Expr(value.clone(), Some(Default::default()))
                            }
                        });
                        block.stmts.splice(i..=i, keep);
                        changed = true;
                        continue;
                    }
                    if !uses.rebound && uses.in_macros == 0 && is_constant(&value) {
                        block.stmts.remove(i);
                        let mut replace = Replace {
                            name: &name,
                            with: &value,
                        };
                        for stmt in &mut block.stmts[i..] {
                            replace.visit_stmt_mut(stmt);
                        }
                        changed = true;
                        continue;
                    }
                    if let Some(only) = only_in_list(&value).filter(is_constant) {
                        if !uses.rebound && uses.in_macros == 0 {
                            if unroll_loops(block, i, &name, &only) {
                                changed = true;
                                continue;
                            }
                        }
                    }
                }
            }
        }
        // `for player in Some(x).into_iter().collect::<Vec<_>>() { ... }`
        if let Stmt::Expr(Expr::ForLoop(looped), _) = &block.stmts[i] {
            if let (Some(only), Pat::Ident(variable)) = (only_in_list(&looped.expr), &*looped.pat) {
                if is_constant(&only) && !breaks_out(&looped.body) {
                    let name = variable.ident.to_string();
                    let mut body = looped.body.clone();
                    let mut replace = Replace {
                        name: &name,
                        with: &only,
                    };
                    replace.visit_block_mut(&mut body);
                    let replacement = spliced(body, &block.stmts[i + 1..]);
                    block.stmts.splice(i..=i, replacement);
                    changed = true;
                    continue;
                }
            }
        }
        // `if let Some(x) = Some(y) { ... }`
        if let Stmt::Expr(Expr::If(branch), semi) = &block.stmts[i] {
            if branch.else_branch.is_none() {
                if let Expr::Let(binding) = &*branch.cond {
                    if let (Some(value), Pat::TupleStruct(pattern)) =
                        (some_of(&binding.expr), &*binding.pat)
                    {
                        if text_of(&pattern.path) == "Some"
                            && pattern.elems.len() == 1
                            && is_constant(&value)
                        {
                            if let Pat::Ident(variable) = &pattern.elems[0] {
                                let name = variable.ident.to_string();
                                let mut body = branch.then_branch.clone();
                                let mut replace = Replace {
                                    name: &name,
                                    with: &value,
                                };
                                replace.visit_block_mut(&mut body);
                                let _ = semi;
                                let replacement = spliced(body, &block.stmts[i + 1..]);
                                block.stmts.splice(i..=i, replacement);
                                changed = true;
                                continue;
                            }
                        }
                    }
                }
            }
        }
        // The last sentence's end: the state triggers are judged as the item finishes (CR 603.8).
        if !tail && text_of(&block.stmts[i]) == "g . end_sentence () ;" {
            let rest: Vec<_> = block.stmts[i + 1..].iter().map(text_of).collect();
            if rest.iter().all(|s| s == "g . end_sentence () ;" || s == "Ok (())") {
                block.stmts.remove(i);
                changed = true;
                continue;
            }
        }
        i += 1;
    }
    changed
}

/// Each `for` over the one-value list `name` of statement `at`, as its body with the
/// value in place. False, changing nothing, if the list has any other use.
fn unroll_loops(block: &mut Block, at: usize, name: &str, only: &Expr) -> bool {
    let loops: usize = block.stmts[at + 1..]
        .iter()
        .map(|stmt| match stmt {
            Stmt::Expr(Expr::ForLoop(looped), _) if loop_over(looped, name).is_some() => {
                usize::from(!breaks_out(&looped.body) && !mentions(&looped.body, name))
            }
            _ => 0,
        })
        .sum();
    if loops == 0 || loops != uses_in_stmts(&block.stmts[at + 1..], name).reads {
        return false;
    }
    block.stmts.remove(at);
    let mut i = at;
    while i < block.stmts.len() {
        let Stmt::Expr(Expr::ForLoop(looped), _) = &block.stmts[i] else {
            i += 1;
            continue;
        };
        let Some(variable) = loop_over(looped, name) else {
            i += 1;
            continue;
        };
        let variable = variable.ident.to_string();
        let mut body = looped.body.clone();
        let mut replace = Replace {
            name: &variable,
            with: only,
        };
        replace.visit_block_mut(&mut body);
        let replacement = spliced(body, &block.stmts[i + 1..]);
        block.stmts.splice(i..=i, replacement);
    }
    true
}

struct Blocks {
    changed: bool,
    returns_result: bool,
}

impl VisitMut for Blocks {
    fn visit_block_mut(&mut self, node: &mut Block) {
        visit_mut::visit_block_mut(self, node);
        self.changed |= simplify_stmts(node, self.returns_result);
    }
    fn visit_macro_mut(&mut self, node: &mut syn::Macro) {
        if node.path.is_ident("matches") {
            if let Ok(mut parsed) = syn::parse2::<Matches>(node.tokens.clone()) {
                self.visit_expr_mut(&mut parsed.value);
                if let Some(guard) = &mut parsed.guard {
                    self.visit_expr_mut(guard);
                }
                node.tokens = parsed.into_tokens();
            }
        }
    }
    fn visit_expr_for_loop_mut(&mut self, node: &mut syn::ExprForLoop) {
        visit_mut::visit_expr_for_loop_mut(self, node);
        if let (Pat::Tuple(pair), Expr::MethodCall(call)) = (&*node.pat, &*node.expr) {
            if let (true, Some(Pat::Ident(index)), Some(value)) =
                (call.method == "enumerate", pair.elems.first(), pair.elems.get(1))
            {
                if pair.elems.len() == 2 && !mentions(&node.body, &index.ident.to_string()) {
                    let (value, over) = (value.clone(), (*call.receiver).clone());
                    *node.pat = value;
                    *node.expr = over;
                    self.changed = true;
                }
            }
        }
    }
    fn visit_expr_closure_mut(&mut self, node: &mut syn::ExprClosure) {
        let returns_result = std::mem::replace(&mut self.returns_result, false);
        visit_mut::visit_expr_closure_mut(self, node);
        self.returns_result = returns_result;
        for input in &mut node.inputs {
            let variable = match input {
                Pat::Type(typed) => &mut *typed.pat,
                other => other,
            };
            // `|&id|`, as a closure's argument, is the same variable.
            let name = match variable {
                Pat::Ident(ident) => Some(ident.ident.to_string()),
                Pat::Reference(reference) => match &*reference.pat {
                    Pat::Ident(ident) => Some(ident.ident.to_string()),
                    _ => None,
                },
                _ => None,
            };
            if let Some(name) = name {
                let mut uses = Uses {
                    name: &name,
                    ..Uses::default()
                };
                uses.visit_expr(&node.body);
                if !uses.any() {
                    *variable = wild();
                }
            }
        }
    }
    fn visit_expr_if_mut(&mut self, node: &mut syn::ExprIf) {
        visit_mut::visit_expr_if_mut(self, node);
        if let Expr::Let(binding) = &mut *node.cond {
            drop_unread_fields(&mut binding.pat, &node.then_branch);
        }
    }
    fn visit_arm_mut(&mut self, node: &mut syn::Arm) {
        visit_mut::visit_arm_mut(self, node);
        drop_unread_bindings(&mut node.pat, |name| {
            let mut uses = Uses {
                name,
                ..Uses::default()
            };
            uses.visit_expr(&node.body);
            if let Some((_, guard)) = &node.guard {
                uses.visit_expr(guard);
            }
            uses.any()
        });
    }
}

fn wild() -> Pat {
    Pat::Wild(syn::PatWild {
        attrs: vec![],
        underscore_token: Default::default(),
    })
}

fn drop_unread_fields(pat: &mut Pat, body: &Block) {
    drop_unread_bindings(pat, |name| {
        let mut uses = Uses {
            name,
            ..Uses::default()
        };
        uses.visit_block(body);
        uses.any()
    });
}

/// Whether the variable `name` stands in this printed text. `i64 :: from` and `x . from`
/// name no variable `from`; `.. from` does.
fn text_reads(body: &str, name: &str) -> bool {
    body.match_indices(name).any(|(at, _)| {
        let (before, after) = (&body[..at], &body[at + name.len()..]);
        let is_word = |c: char| c.is_alphanumeric() || c == '_';
        !before.ends_with(is_word)
            && !after.starts_with(is_word)
            && !before.ends_with(":: ")
            && (!before.ends_with(". ") || before.ends_with(". . "))
    })
}

fn drop_unread_bindings(pat: &mut Pat, reads: impl Fn(&str) -> bool) {
    let reads = |name: &syn::Ident| reads(&name.to_string());
    match pat {
        Pat::Struct(fields) => {
            let kept: Vec<_> = fields
                .fields
                .iter()
                .filter(|f| match &*f.pat {
                    Pat::Ident(ident) => reads(&ident.ident),
                    _ => true,
                })
                .cloned()
                .collect();
            if kept.len() < fields.fields.len() && fields.rest.is_none() {
                fields.rest = Some(syn::PatRest {
                    attrs: vec![],
                    dot2_token: Default::default(),
                });
            }
            fields.fields = kept.into_iter().collect();
        }
        Pat::TupleStruct(parts) => {
            for part in &mut parts.elems {
                if let Pat::Ident(ident) = part {
                    if !reads(&ident.ident) {
                        *part = wild();
                    }
                }
            }
        }
        _ => {}
    }
}

/// A function's last statement `return x;` as `x`.
/// `if let Some(x) = e { .. } Ok(())` at the end of a function is `let Some(x) = e else { return Ok(()) }; .. Ok(())`: the
/// same program, but where `e` is always something (a trigger's object subject, a target the spell resolved for) the `if
/// let`'s closing brace is a line no game runs, and Cantrip's coverage gate asks for every line.
fn guard_at_the_end(block: &mut Block) {
    let n = block.stmts.len();
    if n < 2 || text_of(&block.stmts[n - 1]) != "Ok (())" {
        return;
    }
    let Stmt::Expr(Expr::If(branch), _) = &block.stmts[n - 2] else {
        return;
    };
    if branch.else_branch.is_some() {
        return;
    }
    let Expr::Let(binding) = &*branch.cond else {
        return;
    };
    let Pat::TupleStruct(pattern) = &*binding.pat else {
        return;
    };
    if text_of(&pattern.path) != "Some" || pattern.elems.len() != 1 {
        return;
    }
    let (pat, value) = (&binding.pat, &binding.expr);
    // `let .. = { .. } else` is no Rust: an initializer of a `let .. else` can't end in a brace.
    if text_of(value).ends_with('}') {
        return;
    }
    let guard: Stmt = parse_quote!(let #pat = #value else { return Ok(()) };);
    let mut body = branch.then_branch.stmts.clone();
    if let Some(Stmt::Expr(_, semi @ None)) = body.last_mut() {
        *semi = Some(Default::default());
    }
    block.stmts.truncate(n - 2);
    block.stmts.push(guard);
    block.stmts.extend(body);
    block.stmts.push(Stmt::Expr(parse_quote!(Ok(())), None));
}

fn tail_return(block: &mut Block) {
    if let Some(Stmt::Expr(Expr::Return(value), _)) = block.stmts.last() {
        if let Some(value) = value.expr.clone() {
            let last = block.stmts.len() - 1;
            block.stmts[last] = Stmt::Expr(*value, None);
        }
    }
}

/// "Each opponent loses 1 life" is one event for all of them (CR 119.3), which the game has a verb for: `let players =
/// <the opponents>; for player in players { g.lose_life(player, n); }` says it one player at a time.
struct LifeLostTogether;

impl LifeLostTogether {
    fn joined(list: &Stmt, each: &Stmt, later: &[Stmt]) -> Option<Stmt> {
        let Stmt::Local(local) = list else { return None };
        let Pat::Ident(name) = &local.pat else { return None };
        let name = name.ident.to_string();
        let init = local.init.as_ref()?;
        let list_text: String = text_of(&init.expr).split_whitespace().collect();
        if !list_text.ends_with("q.state().opponents(item.controller).collect::<Vec<PlayerId>>()}") {
            return None;
        }
        let Stmt::Expr(Expr::ForLoop(each), _) = each else { return None };
        let Pat::Ident(player) = &*each.pat else { return None };
        if ident_of(&each.expr)? != name {
            return None;
        }
        let [Stmt::Expr(Expr::MethodCall(call), _)] = each.body.stmts.as_slice() else {
            return None;
        };
        if text_of(&call.receiver) != "g" || call.method != "lose_life" || call.args.len() != 2 {
            return None;
        }
        let player = player.ident.to_string();
        if text_of(&call.args[0]) != player {
            return None;
        }
        let amount = &call.args[1];
        let mut in_amount = Uses {
            name: &player,
            ..Uses::default()
        };
        in_amount.visit_expr(amount);
        let mut after = Uses {
            name: &name,
            ..Uses::default()
        };
        later.iter().for_each(|stmt| after.visit_stmt(stmt));
        if in_amount.any() || after.any() {
            return None;
        }
        let text = format!("g.each_opponent_loses_life(item.controller, {})", text_of(amount));
        Some(Stmt::Expr(syn::parse_str(&text).ok()?, Some(Default::default())))
    }
}

impl VisitMut for LifeLostTogether {
    fn visit_block_mut(&mut self, block: &mut Block) {
        visit_mut::visit_block_mut(self, block);
        let mut at = 0;
        while at + 1 < block.stmts.len() {
            if let Some(joined) = Self::joined(&block.stmts[at], &block.stmts[at + 1], &block.stmts[at + 2..]) {
                block.stmts.splice(at..at + 2, [joined]);
            }
            at += 1;
        }
    }
}

fn simplify_function(function: &mut syn::ItemFn, reads_data: bool) {
    let item_asker = function
        .block
        .stmts
        .iter()
        .any(|s| text_of(s).starts_with("let asker = & Asker :: new (item . object , item . controller)"));
    let mut tidy = Tidy {
        item_asker,
        reads_data,
    };
    for _ in 0..12 {
        tidy.visit_item_fn_mut(function);
        let mut blocks = Blocks {
            changed: false,
            returns_result: text_of(&function.sig.output).contains("Result"),
        };
        blocks.visit_block_mut(&mut function.block);
        if !blocks.changed {
            break;
        }
    }
    LifeLostTogether.visit_block_mut(&mut function.block);
    if text_of(&function.sig.output).contains("Result") {
        guard_at_the_end(&mut function.block);
    }
    tail_return(&mut function.block);
    for input in &mut function.sig.inputs {
        if let syn::FnArg::Typed(typed) = input {
            if let Pat::Ident(ident) = &*typed.pat {
                let name = ident.ident.to_string();
                let mut uses = Uses {
                    name: &name,
                    ..Uses::default()
                };
                uses.visit_block(&function.block);
                if !uses.any() {
                    *typed.pat = wild();
                }
            }
        }
    }
}

/// `static ..._TARGETS` by the function that reads them, and whether each is one exact target.
fn single_targets(file: &File) -> BTreeMap<String, Vec<bool>> {
    let mut found = BTreeMap::new();
    for item in &file.items {
        if let Item::Static(item) = item {
            let name = item.ident.to_string();
            if let (Some(stem), Expr::Reference(list)) = (name.strip_suffix("_TARGETS"), &*item.expr) {
                if let Expr::Array(specs) = &*list.expr {
                    let single = specs
                        .elems
                        .iter()
                        .map(|spec| !matches!(spec, Expr::MethodCall(_)))
                        .collect();
                    found.insert(stem.to_lowercase(), single);
                }
            }
        }
    }
    found
}

/// The announced-target reads of a function with `targets::object`, wherever its
/// target slot is certain: each earlier spec is one exact target.
fn use_shared_targets(file: &mut File) {
    let specs = single_targets(file);
    for item in &mut file.items {
        let Item::Fn(function) = item else { continue };
        let Some(singles) = specs.get(&function.sig.ident.to_string()) else {
            continue;
        };
        let lone = singles.len() == 1 && singles[0];
        one_target_reads(&mut function.block, singles, lone);
    }
}

/// Which of the shared readers says exactly what `read` does, if one does. Anything
/// more, a filter or a walk of what the target owns, is the card's and stays.
fn one_target_kind(read: &str, announced: &str) -> Option<&'static str> {
    let read = read
        .strip_prefix("{ let q : & dyn Query = g ; ")
        .and_then(|read| read.strip_suffix(" }"))
        .unwrap_or(read);
    let not_moved = " . filter (| & o | q . state () . obj (o) . became . is_none ())";
    match read.strip_prefix(announced)? {
        " . and_then (| t : Target | t . object ()) . into_iter () . collect :: < Vec < _ > > ()" => Some("object"),
        " . and_then (| t : Target | t . player ()) . into_iter () . collect :: < Vec < PlayerId > > ()" => Some("player"),
        // A target that moved is no longer the target (CR 400.7), and `targets::object`
        // reads only what is still legal.
        other if other
            == format!(" . and_then (| t : Target | t . object ()) . into_iter (){not_moved} . collect :: < Vec < ObjectId > > ()") =>
        {
            Some("object")
        }
        _ => None,
    }
}

fn one_target_reads(block: &mut Block, singles: &[bool], lone: bool) {
    for spec in 0..singles.len() {
        if !singles[..=spec].iter().all(|single| *single) {
            return;
        }
        let announced = format!("t_{spec}_raw . get (0) . copied () . filter (| t | t_{spec} . contains (t))");
        for i in 0..block.stmts.len() {
            let Stmt::Local(local) = &block.stmts[i] else {
                continue;
            };
            let Some(name) = plain_binding(local) else {
                continue;
            };
            let Some(init) = &local.init else { continue };
            let read = text_of(&init.expr);
            let Some(kind) = one_target_kind(&read, &announced) else {
                continue;
            };
            // Whether a target is legal is settled as the item starts to resolve (CR 608.2b),
            // and `targets::object` asks again: a sentence before this one can have given
            // the target hexproof, or protection.
            let undisturbed = block.stmts[..i].iter().all(|stmt| match stmt {
                Stmt::Local(local) => local.init.as_ref().is_some_and(|init| is_pure(&init.expr)),
                _ => false,
            });
            if !undisturbed {
                continue;
            }
            // Stopping is right only if all that is left needs the target: the other half
            // of "target opponent loses 2 life and you gain 2 life" happens anyway when
            // the spell has another mode to resolve (CR 608.2b).
            let lone = lone
                && block.stmts[i + 1..].iter().all(|stmt| {
                    mentions(stmt, &name) || matches!(text_of(stmt).as_str(), "Ok (())" | "g . end_sentence () ;")
                });
            let later = &block.stmts[i + 1..];
            let uses = uses_in_stmts(later, &name);
            let loops: usize = later.iter().map(|s| loops_in(s, &name)).sum();
            let lists: usize = later.iter().map(|s| borrows_in(s, &name)).sum();
            if uses.reads == 0
                || uses.reads != loops + lists
                || uses.in_macros > 0
                || uses.rebound
                || (lists > 0 && !lone)
            {
                continue;
            }
            let slot = proc_macro2::Literal::usize_unsuffixed(spec);
            let shared: syn::Path = syn::parse_str(&format!("cantrip_mechanics::targets::{kind}")).expect("a path");
            let variable = syn::Ident::new(&name, proc_macro2::Span::call_site());
            let stmt: Stmt = if lone {
                parse_quote!(let Some(#variable) = #shared(g, item, #slot) else { return Ok(()); };)
            } else {
                parse_quote!(let #variable = #shared(g, item, #slot);)
            };
            block.stmts[i] = stmt;
            if lone {
                let value: Expr = parse_quote!(#variable);
                let mut unroll = LoopsOver {
                    name: &name,
                    value: &value,
                };
                unroll.visit_block_mut(block);
                for stmt in &mut block.stmts[i + 1..] {
                    OneListed(&name).visit_stmt_mut(stmt);
                }
                let wanted = if kind == "object" { "target" } else { "player" };
                let (before, after) = (&block.stmts[..i], &block.stmts[i + 1..]);
                if !before.iter().chain(after).any(|stmt| mentions(stmt, wanted)) {
                    let named: Expr = syn::parse_str(wanted).expect("a name");
                    let mut replace = Replace {
                        name: &name,
                        with: &named,
                    };
                    for stmt in &mut block.stmts[i + 1..] {
                        replace.visit_stmt_mut(stmt);
                    }
                    let named = syn::Ident::new(wanted, proc_macro2::Span::call_site());
                    block.stmts[i] = parse_quote!(let Some(#named) = #shared(g, item, #slot) else { return Ok(()); };);
                }
            } else {
                OptionLoops(&name).visit_block_mut(block);
            }
        }
    }
}

/// `&name` as an argument, the list a verb takes.
fn borrows_in(stmt: &Stmt, name: &str) -> usize {
    struct Counts<'a>(&'a str, usize);
    impl<'ast> Visit<'ast> for Counts<'_> {
        fn visit_expr_for_loop(&mut self, node: &'ast syn::ExprForLoop) {
            self.visit_block(&node.body);
        }
        fn visit_expr_reference(&mut self, node: &'ast syn::ExprReference) {
            if ident_of(&node.expr).as_deref() == Some(self.0) {
                self.1 += 1;
            }
            visit::visit_expr_reference(self, node);
        }
    }
    let mut counts = Counts(name, 0);
    counts.visit_stmt(stmt);
    counts.1
}

/// `&name`, one object, as the list `&[name]`.
struct OneListed<'a>(&'a str);

impl VisitMut for OneListed<'_> {
    fn visit_expr_mut(&mut self, node: &mut Expr) {
        if let Expr::Reference(reference) = node {
            if ident_of(&reference.expr).as_deref() == Some(self.0) {
                let inner = reference.expr.clone();
                *node = parse_quote!(&[#inner]);
                return;
            }
        }
        visit_mut::visit_expr_mut(self, node);
    }
}

fn loops_in(stmt: &Stmt, name: &str) -> usize {
    struct Counts<'a>(&'a str, usize);
    impl<'ast> Visit<'ast> for Counts<'_> {
        fn visit_expr_for_loop(&mut self, node: &'ast syn::ExprForLoop) {
            if loop_over(node, self.0).is_some() {
                self.1 += 1;
            }
            visit::visit_expr_for_loop(self, node);
        }
    }
    let mut counts = Counts(name, 0);
    counts.visit_stmt(stmt);
    counts.1
}

/// Each `for` over the `Option` `name` as an `if let`.
struct OptionLoops<'a>(&'a str);

impl VisitMut for OptionLoops<'_> {
    fn visit_expr_mut(&mut self, node: &mut Expr) {
        visit_mut::visit_expr_mut(self, node);
        if let Expr::ForLoop(looped) = node {
            if let Some(variable) = loop_over(looped, self.0) {
                if !breaks_out(&looped.body) {
                    let (variable, body) = (variable.ident.clone(), looped.body.clone());
                    let over = syn::Ident::new(self.0, proc_macro2::Span::call_site());
                    *node = parse_quote!(if let Some(#variable) = #over #body);
                }
            }
        }
    }
}

/// Each `for` over the one object `name`, anywhere below, as its body.
struct LoopsOver<'a> {
    name: &'a str,
    value: &'a Expr,
}

impl VisitMut for LoopsOver<'_> {
    /// A closure whose whole body is the loop has no block to splice into.
    fn visit_expr_closure_mut(&mut self, node: &mut syn::ExprClosure) {
        visit_mut::visit_expr_closure_mut(self, node);
        let Expr::ForLoop(looped) = &*node.body else {
            return;
        };
        let Some(variable) = loop_over(looped, self.name) else {
            return;
        };
        let variable = variable.ident.to_string();
        let mut body = looped.body.clone();
        let mut replace = Replace {
            name: &variable,
            with: self.value,
        };
        replace.visit_block_mut(&mut body);
        *node.body = Expr::Block(syn::ExprBlock {
            attrs: vec![],
            label: None,
            block: body,
        });
    }
    fn visit_block_mut(&mut self, node: &mut Block) {
        visit_mut::visit_block_mut(self, node);
        let mut i = 0;
        while i < node.stmts.len() {
            if let Stmt::Expr(Expr::ForLoop(looped), _) = &node.stmts[i] {
                if let Some(variable) = loop_over(looped, self.name) {
                    let variable = variable.ident.to_string();
                    let mut body = looped.body.clone();
                    let mut replace = Replace {
                        name: &variable,
                        with: self.value,
                    };
                    replace.visit_block_mut(&mut body);
                    let replacement = spliced(body, &node.stmts[i + 1..]);
                    node.stmts.splice(i..=i, replacement);
                    continue;
                }
            }
            i += 1;
        }
    }
}

/// The items named by something that stands.
fn drop_unreached(file: &mut File) {
    fn named(item: &Item) -> Option<String> {
        match item {
            Item::Fn(item) => Some(item.sig.ident.to_string()),
            Item::Static(item) => Some(item.ident.to_string()),
            Item::Const(item) => Some(item.ident.to_string()),
            _ => None,
        }
    }
    let public = |item: &Item| {
        matches!(item, Item::Static(s) if matches!(s.vis, syn::Visibility::Public(_)))
            || matches!(item, Item::Fn(f) if matches!(f.vis, syn::Visibility::Public(_)))
            || matches!(item, Item::Const(c) if matches!(c.vis, syn::Visibility::Public(_)))
    };
    let mut kept: BTreeSet<usize> = file
        .items
        .iter()
        .enumerate()
        .filter(|(_, item)| named(item).is_none() || public(item))
        .map(|(i, _)| i)
        .collect();
    loop {
        let mut grew = false;
        for (i, item) in file.items.iter().enumerate() {
            if kept.contains(&i) {
                continue;
            }
            let name = named(item).unwrap_or_default();
            let reached = file
                .items
                .iter()
                .enumerate()
                .any(|(j, other)| kept.contains(&j) && mentions(other, &name));
            if reached {
                kept.insert(i);
                grew = true;
            }
        }
        if !grew {
            break;
        }
    }
    let mut i = 0;
    file.items.retain(|_| {
        i += 1;
        kept.contains(&(i - 1))
    });
}

/// What the generated `oracle::` constant says, so a file need not.
const CHARACTERISTICS: &[&str] = &[
    "name", "token_words", "oracle", "cost", "has_cost", "types", "subtypes", "supertypes", "colors",
    "power", "toughness", "loyalty", "defense", "keywords",
];

/// What a file's statics and constants are defined as.
fn definitions(file: &mut File) -> impl Iterator<Item = &mut Expr> {
    file.items.iter_mut().filter_map(|item| match item {
        Item::Static(item) => Some(&mut *item.expr),
        Item::Const(item) => Some(&mut *item.expr),
        _ => None,
    })
}

/// The definition that says what the card is: `CARD`, or the `const` it spreads, as a
/// bestow card spreads the face its creature and its Aura share (CR 702.103b).
fn the_card(file: &mut File) -> Option<&mut syn::ExprStruct> {
    let spread = file.items.iter().find_map(|item| match item {
        Item::Static(item) if item.ident == "CARD" => match &*item.expr {
            Expr::Struct(card) => card.rest.as_deref().and_then(ident_of),
            _ => None,
        },
        _ => None,
    });
    let spread = spread.filter(|name| {
        file.items
            .iter()
            .any(|item| matches!(item, Item::Const(c) if c.ident == name))
    });
    file.items.iter_mut().find_map(|item| match item {
        Item::Static(item) if spread.is_none() && item.ident == "CARD" => match &mut *item.expr {
            Expr::Struct(card) => Some(card),
            _ => None,
        },
        Item::Const(item) if spread.as_ref().is_some_and(|name| item.ident == name) => match &mut *item.expr {
            Expr::Struct(card) => Some(card),
            _ => None,
        },
        _ => None,
    })
}

/// Keywords the generated constant is given, `KEYWORDS` of cantrip-oracle's build script.
const ORACLE_KEYWORDS: &[&str] = &[
    "Flying", "Haste", "Vigilance", "Trample", "FirstStrike", "DoubleStrike", "Deathtouch", "Lifelink", "Reach",
    "Defender", "Flash", "Hexproof", "Indestructible", "Menace", "Infect", "Shadow", "Shroud", "CantBeBlocked",
    "CantBeCountered", "CantSpendManaToCast", "CastFromYourGraveyard", "Daybound", "Nightbound", "Changeling",
    "Decayed", "CantBeBlockedByMoreThanOne", "Islandwalk", "Plainswalk", "Swampwalk", "Mountainwalk", "Forestwalk",
    "Banding", "CrewsAsThoughPowerTwoGreater", "SaddlesAsThoughPowerTwoGreater", "Horsemanship", "Fear", "Intimidate",
    "Skulk", "NonbasicLandwalk", "LegendaryLandwalk", "Wither", "Phasing", "CantBeCopied",
];

fn has_keyword_beyond_oracle(keywords: &Expr) -> bool {
    let text = text_of(keywords);
    text.split("Keyword :: ")
        .skip(1)
        .any(|rest| !ORACLE_KEYWORDS.contains(&rest.split(|c: char| !c.is_alphanumeric()).next().unwrap_or_default()))
}

/// A field that says what `CardDef::BLANK` says.
fn is_default_field(value: &Expr) -> bool {
    matches!(
        text_of(value).as_str(),
        "None" | "false" | "0" | "& []" | "ColorSet (0)" | "Keywords :: of (& [])"
    )
}

fn drop_defaults(file: &mut File, characteristics: Characteristics) {
    if let Some(card) = the_card(file) {
        // A `*` power is 0 where the card's own definition no longer applies (CR 208.2b),
        // and Cantrip's content holds no number for it.
        let defines_size = text_of(card).contains("characteristic_defining (Layer :: PtDefine");
        let kept: Vec<_> = card
            .fields
            .iter()
            .filter(|field| {
                let name = text_of(&field.member);
                let stays = (name == "keywords" && has_keyword_beyond_oracle(&field.expr))
                    || (defines_size && matches!(name.as_str(), "power" | "toughness"));
                (name == "has_cost" || !is_default_field(&field.expr))
                    && !(characteristics == Characteristics::Oracle
                        && CHARACTERISTICS.contains(&name.as_str())
                        && !stays)
            })
            .cloned()
            .collect();
        card.fields = kept.into_iter().collect();
    }
    file.items.retain(|item| {
        !matches!(item, Item::Static(s) if s.ident == "EXTRA" && text_of(&s.expr) == "& []")
            || characteristics == Characteristics::Inline
    });
}

fn set_oracle_base(file: &mut File, constant: &str) {
    if let Some(card) = the_card(file) {
        let base: syn::Path = syn::parse_str(&format!("oracle::{constant}")).expect("a constant");
        card.rest = Some(Box::new(parse_quote!(#base)));
    }
}

/// A list of target specs named once, where it is read, and the card's lasting
/// rules, a table of their own only so they can be a hole.
fn inline_target_lists(file: &mut File) {
    let lists: Vec<(String, Expr)> = file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Static(item)
                if item.ident.to_string().ends_with("_TARGETS") || item.ident == "LASTING_RULES" =>
            {
                Some((item.ident.to_string(), (*item.expr).clone()))
            }
            _ => None,
        })
        .collect();
    for (name, list) in lists {
        let total: usize = file
            .items
            .iter()
            .map(|item| tokens_name(item.to_token_stream(), &name))
            .sum();
        if total != 2 {
            continue;
        }
        file.items
            .retain(|item| !matches!(item, Item::Static(s) if s.ident == name));
        let mut replace = Replace {
            name: &name,
            with: &list,
        };
        for item in &mut file.items {
            replace.visit_item_mut(item);
        }
    }
}

/// `cantrip_api::x` as `x`, and `cantrip_mechanics::a::b` as `a::b`, with what must be imported.
fn shorten_paths(file: &mut File) -> Vec<String> {
    struct Shorten {
        modules: BTreeSet<String>,
        names: BTreeSet<String>,
    }
    impl VisitMut for Shorten {
        fn visit_item_mod_mut(&mut self, _: &mut syn::ItemMod) {}
        fn visit_path_mut(&mut self, node: &mut syn::Path) {
            visit_mut::visit_path_mut(self, node);
            let first = node.segments.first().map(|s| s.ident.to_string());
            let keep = match (first.as_deref(), node.segments.len()) {
                (Some("cantrip_api"), 2) => 1,
                (Some("cantrip_api"), _) if node.segments[1].ident.to_string().starts_with(char::is_uppercase) => 1,
                (Some("cantrip_mechanics"), n) if n > 2 => {
                    self.modules.insert(node.segments[1].ident.to_string());
                    1
                }
                (Some("cantrip_mechanics"), 2) => {
                    self.names.insert(node.segments[1].ident.to_string());
                    1
                }
                _ => return,
            };
            let segments: Vec<_> = node.segments.iter().skip(keep).cloned().collect();
            node.segments = segments.into_iter().collect();
            node.leading_colon = None;
        }
    }
    let mut shorten = Shorten {
        modules: BTreeSet::new(),
        names: BTreeSet::new(),
    };
    shorten.visit_file_mut(file);
    let names: Vec<_> = shorten.modules.into_iter().chain(shorten.names).collect();
    match names.as_slice() {
        [] => vec![],
        [one] => vec![format!("use cantrip_mechanics::{one};")],
        many => vec![format!("use cantrip_mechanics::{{{}}};", many.join(", "))],
    }
}

/// The printed-text argument of each ability a card lists, with the functions under it.
struct Abilities {
    of_function: BTreeMap<String, String>,
}

struct Constructors {
    found: Vec<(String, BTreeSet<String>)>,
    functions: BTreeSet<String>,
}

/// The printed text a constructor call was given.
fn printed_of(call: &syn::ExprCall) -> Option<String> {
    let name = text_of(&call.func);
    if !matches!(
        name.as_str(),
        "triggered" | "ability" | "mana_ability" | "static_ability" | "activated" | "loyalty_ability"
            | "replacement" | "prevention" | "chapter"
    ) {
        return None;
    }
    call.args.iter().rev().find_map(|arg| match arg {
        Expr::Lit(syn::ExprLit { lit: Lit::Str(text), .. }) => Some(text.value()),
        _ => None,
    })
}

impl Constructors {
    fn found_in<T: ToTokens>(&mut self, printed: String, node: &T) {
        let called = self.functions.iter().filter(|f| mentions(node, f)).cloned().collect();
        self.found.push((printed, called));
    }
}

impl<'ast> Visit<'ast> for Constructors {
    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let Some(printed) = printed_of(node) {
            self.found_in(printed, node);
        }
        visit::visit_expr_call(self, node);
    }
    /// `ability(..).targets(&[target(f)])`: what the chain names is the ability's.
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let mut root = &*node.receiver;
        while let Expr::MethodCall(call) = root {
            root = &call.receiver;
        }
        if let Expr::Call(call) = root {
            if let Some(printed) = printed_of(call) {
                self.found_in(printed, node);
            }
        }
        visit::visit_expr_method_call(self, node);
    }
}

fn abilities_of(file: &File) -> Abilities {
    let functions: BTreeSet<String> = file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Fn(item) => Some(item.sig.ident.to_string()),
            _ => None,
        })
        .collect();
    let mut constructors = Constructors {
        found: vec![],
        functions: functions.clone(),
    };
    for item in &file.items {
        match item {
            Item::Static(item) => constructors.visit_item_static(item),
            Item::Const(item) => constructors.visit_item_const(item),
            _ => {}
        }
    }
    let mut of_function = BTreeMap::new();
    for (printed, called) in constructors.found {
        for function in called {
            of_function.entry(function).or_insert_with(|| printed.clone());
        }
    }
    // What those call, down the file.
    loop {
        let mut grew = false;
        for item in &file.items {
            let Item::Fn(item) = item else { continue };
            let Some(printed) = of_function.get(&item.sig.ident.to_string()).cloned() else {
                continue;
            };
            for function in &functions {
                if !of_function.contains_key(function) && mentions(&item.block, function) {
                    of_function.insert(function.clone(), printed.clone());
                    grew = true;
                }
            }
        }
        if !grew {
            break;
        }
    }
    Abilities { of_function }
}

const QUIET: &[&str] = &[
    "a", "an", "the", "this", "that", "these", "those", "of", "to", "until", "end", "turn", "your", "you", "and",
    "it", "its", "on", "at", "for", "with", "from", "or", "may", "then", "each", "is", "are",
];

fn without_reminder(text: &str) -> String {
    let mut out = String::new();
    let mut depth = 0;
    for c in text.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

/// What an ability does: the words after its cost or trigger condition.
fn effect_clause(printed: &str) -> String {
    let plain = without_reminder(printed);
    let first = plain.lines().next().unwrap_or_default().trim();
    if let Some((_, after)) = first.split_once(": ") {
        return after.to_string();
    }
    if ["When", "Whenever", "At "].iter().any(|w| first.starts_with(w)) {
        if let Some((_, after)) = first.split_once(", ") {
            return after.to_string();
        }
    }
    first.to_string()
}

/// A trigger's condition, word for word.
fn trigger_quote(printed: &str) -> String {
    printed.split(", ").next().unwrap_or(printed).trim().to_string()
}

/// A trigger's condition, as printed.
fn trigger_clause(printed: &str) -> String {
    let plain = without_reminder(printed);
    let first = plain.lines().next().unwrap_or_default().trim();
    first.split(", ").next().unwrap_or(first).to_string()
}

fn slug(words: &str, limit: usize) -> String {
    let lowered: String = words
        .chars()
        .map(|c| match c {
            '\u{2019}' | '\'' => '\0',
            c if c.is_ascii_alphanumeric() => c.to_ascii_lowercase(),
            c if c.is_alphabetic() => '\0',
            _ => ' ',
        })
        .filter(|c| *c != '\0')
        .collect();
    let kept: Vec<&str> = lowered
        .split_whitespace()
        .filter(|w| !QUIET.contains(w))
        .take(limit)
        .collect();
    let joined = kept.join("_");
    match joined.as_str() {
        "" => "effect".to_string(),
        s if s.starts_with(|c: char| c.is_ascii_digit()) => format!("n_{s}"),
        s if syn::parse_str::<syn::Ident>(s).is_err() || s == "self" || s == "super" => format!("{s}_effect"),
        _ => joined,
    }
}

struct Names {
    renamed: BTreeMap<String, String>,
    quotes: BTreeMap<String, (String, Role)>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    Effect,
    Trigger,
    Target,
    Static,
    /// The set a rule on an open set is asked of (CR 611.2c).
    Lasting,
    /// The damage a prevention effect applies to (CR 615.1).
    Prevention,
    Mana,
}

/// `token_1_ability_0`, lifted out of its module, as `ability_0`.
fn own_name(lifted: &str) -> &str {
    let mut rest = lifted;
    while let Some((kind, after)) = rest.split_once('_') {
        let Some((number, after)) = after.split_once('_') else {
            break;
        };
        if !matches!(kind, "token" | "emblem") || number.parse::<u32>().is_err() {
            break;
        }
        rest = after;
    }
    rest
}

/// A line that is a keyword and nothing a spell does: "Foretell {1}{R}{R}", or a cost in a sentence of its own,
/// "Flashback—{1}{G}, Pay 3 life.". "Choose one —" opens what the spell does.
fn is_keyword_line(line: &str) -> bool {
    let plain = without_reminder(line);
    let plain = plain.trim();
    let sentence = plain.ends_with('.') || plain.ends_with('—') || plain.ends_with(':');
    !sentence || plain.split_whitespace().next().is_some_and(|word| word.contains('—'))
}

/// What a spell does: its last unbroken run of lines that are not keywords, which can come after them as well as before.
fn spell_text(printed: &str) -> String {
    let lines: Vec<&str> = printed.lines().map(str::trim).filter(|line| !line.is_empty()).collect();
    let Some(end) = lines.iter().rposition(|line| !is_keyword_line(line)) else {
        return lines.last().copied().unwrap_or_default().to_string();
    };
    let start = lines[..end].iter().rposition(|line| is_keyword_line(line)).map_or(0, |at| at + 1);
    lines[start..=end].join(" ")
}

fn function_names(file: &File, printed: &str) -> Names {
    let abilities = abilities_of(file);
    let spell_text = spell_text(printed);
    let mut renamed = BTreeMap::new();
    let mut quotes = BTreeMap::new();
    let mut taken: BTreeSet<String> = BTreeSet::new();
    for item in &file.items {
        let Item::Fn(function) = item else { continue };
        let old = function.sig.ident.to_string();
        let text = abilities.of_function.get(&old).filter(|text| !text.trim().is_empty()).cloned();
        let source = text.clone().unwrap_or_else(|| spell_text.clone());
        let parts: Vec<&str> = own_name(&old).split('_').collect();
        let role = if old.ends_with("_mana") {
            Role::Mana
        } else if old.contains("event") {
            Role::Trigger
        } else if old.contains("target") {
            Role::Target
        } else if old.starts_with("lasting_rule") {
            Role::Lasting
        } else if old.ends_with("_applies") && text_of(&function.sig).contains("DamageEvent") {
            Role::Prevention
        } else if ["affects", "applies", "apply", "rule"].iter().any(|w| old.contains(w)) {
            Role::Static
        } else {
            Role::Effect
        };
        let quote = match role {
            Role::Trigger => trigger_quote(&source),
            _ => source.lines().next().unwrap_or_default().trim().to_string(),
        };
        if parts.first() != Some(&"ability") {
            quotes.insert(old, (quote, role));
            continue;
        }
        let rest: Vec<&str> = parts.iter().skip(1).copied().skip_while(|p| p.parse::<u32>().is_ok()).collect();
        let rest = rest.join("_");
        let base = match role {
            Role::Trigger => format!("when_{}", slug(&trigger_clause(&source).replacen("Whenever", "", 1).replacen("When", "", 1), 5)),
            _ => slug(&effect_clause(&source), 5),
        };
        let wanted = match rest.as_str() {
            "" | "event" => base,
            rest => format!("{base}_{rest}"),
        };
        let mut unique = wanted.clone();
        let mut n = 2;
        while taken.contains(&unique) || RESERVED_NAMES.contains(&unique.as_str()) {
            unique = format!("{wanted}_{n}");
            n += 1;
        }
        taken.insert(unique.clone());
        quotes.insert(unique.clone(), (quote, role));
        renamed.insert(old, unique);
    }
    Names { renamed, quotes }
}

/// Names a card's own function would hide that its code still calls.
const RESERVED_NAMES: &[&str] = &["target", "spell", "triggered", "ability", "add", "when", "when_asked", "instruction"];

fn rename(file: &mut File, names: &BTreeMap<String, String>) {
    struct Rename<'a>(&'a BTreeMap<String, String>);
    impl VisitMut for Rename<'_> {
        fn visit_ident_mut(&mut self, node: &mut syn::Ident) {
            if let Some(new) = self.0.get(&node.to_string()) {
                *node = syn::Ident::new(new, node.span());
            }
        }
        fn visit_macro_mut(&mut self, node: &mut syn::Macro) {
            node.tokens = renamed_tokens(node.tokens.clone(), self.0);
        }
    }
    fn renamed_tokens(tokens: proc_macro2::TokenStream, names: &BTreeMap<String, String>) -> proc_macro2::TokenStream {
        tokens
            .into_iter()
            .map(|token| match token {
                proc_macro2::TokenTree::Ident(ident) => match names.get(&ident.to_string()) {
                    Some(new) => proc_macro2::TokenTree::Ident(proc_macro2::Ident::new(new, ident.span())),
                    None => proc_macro2::TokenTree::Ident(ident),
                },
                proc_macro2::TokenTree::Group(group) => {
                    let mut new = proc_macro2::Group::new(group.delimiter(), renamed_tokens(group.stream(), names));
                    new.set_span(group.span());
                    proc_macro2::TokenTree::Group(new)
                }
                other => other,
            })
            .collect()
    }
    Rename(names).visit_file_mut(file);
}

/// The rule behind what a function does, for its doc comment.
fn rules_of(function: &syn::ItemFn, role: Role) -> String {
    const CALLS: &[(&str, &str)] = &[
        ("add_effect", "611.2a"),
        ("add_lasting_rule", "611.2c"),
        ("damage", "120.3"),
        ("destroy", "701.8a"),
        ("draw", "121.1"),
        ("exile", "406.3"),
        ("sacrifice", "701.21a"),
        ("discard", "701.9a"),
        ("gain_life", "119.3"),
        ("lose_life", "119.3"),
        ("create_token", "111.1"),
        ("add_counters", "122.1"),
        ("return_to_hand", "400.7"),
        ("shuffle", "701.24a"),
        ("reveal", "701.20a"),
        ("mill", "701.17a"),
        ("tap", "701.26a"),
        ("untap", "502.3"),
        ("attach", "701.3a"),
        ("counter", "701.5a"),
        ("fight", "701.14a"),
        ("may_pay", "118.1"),
        ("add_mana", "106.3"),
    ];
    let mut cited: Vec<&str> = vec![match role {
        Role::Trigger => "603.2",
        Role::Target => "115.1",
        Role::Static => "604.1",
        Role::Lasting => "611.2c",
        Role::Prevention => "615.1",
        Role::Mana => "605.3a",
        Role::Effect => "608.2",
    }];
    let body = text_of(&function.block);
    for (call, rule) in CALLS {
        if cited.len() < 3 && body.contains(&format!(". {call}")) && !cited.contains(rule) {
            cited.push(rule);
        }
    }
    format!("CR {}", cited.join(", "))
}

fn document(file: &mut File, quotes: &BTreeMap<String, (String, Role)>) {
    for item in &mut file.items {
        let Item::Fn(function) = item else { continue };
        let Some((quote, role)) = quotes.get(&function.sig.ident.to_string()).cloned() else {
            continue;
        };
        let line = format!(" \"{quote}\" ({}).", rules_of(function, role));
        function.attrs.insert(0, parse_quote!(#[doc = #line]));
    }
}

/// A file with no function of its own says on the card what it answers.
fn document_card(file: &mut File, printed: &str) {
    let has_function = file.items.iter().any(|item| matches!(item, Item::Fn(_)));
    let Some(Item::Static(card)) = file
        .items
        .iter_mut()
        .find(|item| matches!(item, Item::Static(s) if s.ident == "CARD"))
    else {
        return;
    };
    if !has_function {
        let first = printed.lines().next().unwrap_or_default().trim();
        let line = format!(" \"{first}\" (CR 604.1).");
        card.attrs.insert(0, parse_quote!(#[doc = #line]));
    }
}

/// `CardDef { a: b, ..oracle::X }` always spread, so each field is a line a tool can read.
fn fields_on_their_own_lines(source: String) -> String {
    let mut out = String::with_capacity(source.len() + 64);
    for line in source.lines() {
        let spread = line.strip_prefix("pub static CARD: CardDef = CardDef { ").and_then(|r| r.strip_suffix(" };"));
        match spread {
            Some(fields) => {
                out.push_str("pub static CARD: CardDef = CardDef {\n");
                match fields.rsplit_once(", ..") {
                    Some((head, rest)) => out.push_str(&format!("    {head},\n    ..{rest}\n")),
                    None => out.push_str(&format!("    {fields}\n")),
                }
                out.push_str("};\n");
            }
            None => {
                out.push_str(line);
                out.push('\n');
            }
        }
    }
    out
}

/// The expression-level rewrites over what is not a function: the abilities a card lists.
fn tidy_statics(file: &mut File) {
    struct BestowCost;
    impl VisitMut for BestowCost {
        fn visit_field_value_mut(&mut self, node: &mut syn::FieldValue) {
            if text_of(&node.member) == "bestow_cost" {
                if let Some(printed) = printed_cost(&node.expr) {
                    node.expr = parse_quote!(cantrip_api::mana(#printed));
                }
            }
            visit_mut::visit_field_value_mut(self, node);
        }
    }
    for expr in definitions(file) {
        BestowCost.visit_expr_mut(expr);
    }
    let mut tidy = Tidy {
        item_asker: false,
        reads_data: true,
    };
    for expr in definitions(file) {
        tidy.visit_expr_mut(expr);
    }
}

/// The trigger conditions `cantrip_mechanics::triggers` names: an event function with
/// this body, its name in the file and what stands for it.
const SHARED_TRIGGERS: &[(&str, &str, &str)] = &[
    (
        "ZONE_CHANGE",
        "if let Event :: ZoneChange { old , from , to , .. } = event { let id = * old ; let subject = Target :: Object (id) ; if to . is (ZoneKind :: Graveyard) && from . is (ZoneKind :: Battlefield) && asker . source == id { return Fired :: about (subject) ; } } None",
        "THIS_DIES",
    ),
    (
        "ZONE_CHANGE",
        "if let Event :: ZoneChange { old , from , .. } = event { let id = * old ; let subject = Target :: Object (id) ; if from . is (ZoneKind :: Battlefield) && asker . source == id { return Fired :: about (subject) ; } } None",
        "THIS_LEAVES",
    ),
    (
        "STEP_BEGAN",
        "if let Event :: StepBegan { step } = event { let subject = Target :: Player (q . state () . turn . active) ; if matches ! (* step , Step :: Upkeep) && asker . controller == q . state () . turn . active { return Fired :: about (subject) ; } } None",
        "YOUR_UPKEEP",
    ),
    (
        "STEP_BEGAN",
        "if let Event :: StepBegan { step } = event { let subject = Target :: Player (q . state () . turn . active) ; if matches ! (* step , Step :: End) && asker . controller == q . state () . turn . active { return Fired :: about (subject) ; } } None",
        "YOUR_END_STEP",
    ),
    (
        "STEP_BEGAN",
        "if let Event :: StepBegan { step } = event { let subject = Target :: Player (q . state () . turn . active) ; if matches ! (* step , Step :: BeginCombat) && asker . controller == q . state () . turn . active { return Fired :: about (subject) ; } } None",
        "BEGINNING_OF_YOUR_COMBAT",
    ),
    (
        "ZONE_CHANGE",
        "if let Event :: ZoneChange { new , to , .. } = event { let id = * new ; let subject = Target :: Object (id) ; if to . is (ZoneKind :: Battlefield) && asker . source == id { return Fired :: about (subject) ; } } None",
        "THIS_ENTERS",
    ),
    (
        "ATTACKS",
        "if let Event :: Attacks { attacker , .. } = event { let id = * attacker ; let subject = Target :: Object (id) ; if asker . source == id { return Fired :: about (subject) ; } } None",
        "THIS_ATTACKS",
    ),
];

/// What `cantrip_mechanics::effects` calls an effect whose whole body is this, its
/// variables' numbers aside.
const SHARED_EFFECTS: &[(&str, &str)] = &[
    (
        "let Some (target) = cantrip_mechanics :: targets :: object (g , item , 0) else { return Ok (()) ; } ; g . destroy_all (& [target]) ; Ok (())",
        "destroy_first_target",
    ),
    (
        "let Some (target) = cantrip_mechanics :: targets :: object (g , item , 0) else { return Ok (()) ; } ; g . return_to_hand (& [target]) ; Ok (())",
        "return_first_target_to_hand",
    ),
    (
        "let objects = Some (item . object) . into_iter () . collect :: < Vec < ObjectId > > () ; g . return_to_hand (& objects) ; Ok (())",
        "return_this_to_hand",
    ),
    (
        "let mut counters : Vec < (ObjectId , CounterKind , i16) > = Vec :: new () ; for t in Some (item . object) . into_iter () . map (Target :: Object) . collect :: < Vec < Target > > () { if let Target :: Object (object) = t { counters . push ((object , CounterKind :: PlusOne , 1i16)) ; } } g . add_counters_together (& counters) ; Ok (())",
        "plus_one_counter_on_this",
    ),
    (
        "let mut counters : Vec < (ObjectId , CounterKind , i16) > = Vec :: new () ; for t in Some (item . object) . into_iter () . map (Target :: Object) . collect :: < Vec < Target > > () { if let Target :: Object (object) = t { counters . push ((object , CounterKind :: PlusOne , 2i16)) ; } } g . add_counters_together (& counters) ; Ok (())",
        "two_plus_one_counters_on_this",
    ),
];

/// `objects_0` as `objects`, so one table says a body however the generator numbered it.
fn without_numbers(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('_') {
        let (before, after) = rest.split_at(at);
        out.push_str(before);
        let digits = after[1..].bytes().take_while(u8::is_ascii_digit).count();
        let ends = !after[1 + digits..].starts_with(|c: char| c.is_alphanumeric() || c == '_');
        if digits > 0 && ends {
            rest = &after[1 + digits..];
        } else {
            out.push('_');
            rest = &after[1..];
        }
    }
    out.push_str(rest);
    out
}

/// A function that is a shared effect over again, as the shared effect.
fn use_shared_effects(file: &mut File) {
    let mut named: BTreeMap<String, Expr> = BTreeMap::new();
    for item in &file.items {
        let Item::Fn(function) = item else { continue };
        if text_of(&function.sig.inputs) != "g : & mut dyn Game , item : & StackItem" {
            continue;
        }
        let body = without_numbers(&body_text(&function.block));
        if let Some((_, effect)) = SHARED_EFFECTS.iter().find(|(shared, _)| *shared == body) {
            let path = format!("cantrip_mechanics::effects::{effect}");
            named.insert(function.sig.ident.to_string(), syn::parse_str(&path).expect("a path"));
        }
    }
    struct Shared<'a>(&'a BTreeMap<String, Expr>);
    impl VisitMut for Shared<'_> {
        fn visit_expr_mut(&mut self, node: &mut Expr) {
            if let Some(effect) = ident_of(node).and_then(|name| self.0.get(&name)) {
                *node = effect.clone();
                return;
            }
            visit_mut::visit_expr_mut(self, node);
        }
    }
    let mut shared = Shared(&named);
    for expr in definitions(file) {
        shared.visit_expr_mut(expr);
    }
}

fn body_text(block: &Block) -> String {
    block.stmts.iter().map(text_of).collect::<Vec<_>>().join(" ")
}

fn use_shared_triggers(file: &mut File) {
    let mut named: BTreeMap<String, Expr> = BTreeMap::new();
    // A shared trigger fires about nothing, so an effect that reads what its own
    // trigger was about keeps that trigger (Jadeheart Attendant's "the exiled card
    // used to craft it", of the permanent that entered).
    let mut reads_subject: BTreeSet<String> = BTreeSet::new();
    for item in &file.items {
        let Item::Fn(function) = item else { continue };
        let body = body_text(&function.block);
        if body.contains("item . subject") {
            reads_subject.insert(function.sig.ident.to_string());
        }
        if let Some((mask, _, shared)) = SHARED_TRIGGERS.iter().find(|(_, known, _)| *known == body) {
            let path: syn::Path = syn::parse_str(&format!("cantrip_mechanics::triggers::{shared}")).expect("a path");
            named.insert(
                format!("when_asked (EventMask :: {mask} , {})", function.sig.ident),
                parse_quote!(#path),
            );
        }
    }
    struct Replace<'a>(&'a BTreeMap<String, Expr>, &'a BTreeSet<String>);
    impl VisitMut for Replace<'_> {
        fn visit_expr_mut(&mut self, node: &mut Expr) {
            if let Expr::Call(call) = &*node {
                let effect = call.args.iter().nth(1).and_then(ident_of);
                if ident_of(&call.func).as_deref() == Some("triggered")
                    && effect.is_some_and(|effect| self.1.contains(&effect))
                {
                    return;
                }
            }
            visit_mut::visit_expr_mut(self, node);
            if let Some(shared) = self.0.get(&text_of(node)) {
                *node = shared.clone();
            }
        }
    }
    let mut replace = Replace(&named, &reads_subject);
    for expr in definitions(file) {
        replace.visit_expr_mut(expr);
    }
}

/// `add_by(f)` where `f` only adds what `add`, `add_n`, `add_each`, `add_one_of` or
/// `add_any_color` already say.
fn use_shared_mana(file: &mut File) {
    let mut bodies: BTreeMap<String, String> = BTreeMap::new();
    for item in &file.items {
        if let Item::Fn(function) = item {
            bodies.insert(function.sig.ident.to_string(), body_text(&function.block));
        }
    }
    struct Shared<'a>(&'a BTreeMap<String, String>);
    impl Shared<'_> {
        /// What `f` adds, if it adds fixed mana and nothing else.
        fn fixed(&self, function: &str, pool: &[i128; 6]) -> bool {
            let Some(body) = self.0.get(function) else { return false };
            let mut expected = String::new();
            for (symbol, count) in ["W", "U", "B", "R", "G", "C"].iter().zip(pool) {
                if *count > 0 {
                    expected.push_str(&format!("g . add_mana (asker . controller , Mana :: {symbol} , {count}) ; "));
                }
            }
            expected.push_str("Ok (())");
            *body == expected
        }
    }
    impl VisitMut for Shared<'_> {
        fn visit_expr_mut(&mut self, node: &mut Expr) {
            visit_mut::visit_expr_mut(self, node);
            let replaced: Option<Expr> = match &*node {
                // add_by(f).choose_color().colors(ColorSet(n)).yields_any(k)
                Expr::MethodCall(call) if call.method == "yields_any" => (|| {
                    let chain = text_of(&call.receiver);
                    let (function, rest) = chain.strip_prefix("add_by (")?.split_once(") . choose_color () . colors (ColorSet (")?;
                    let colors: u8 = rest.strip_suffix("))")?.trim().parse().ok()?;
                    let amount = &call.args[0];
                    let expected = format!(
                        "let Some (color) = chosen else {{ return Ok (()) ; }} ; g . add_mana (asker . controller , color . mana () , {}) ; Ok (())",
                        text_of(amount)
                    );
                    let typed = expected.replace(&format!(", {})", text_of(amount)), &format!(", {}u32)", text_of(amount)));
                    let body = self.0.get(function.trim());
                    if body != Some(&expected) && body != Some(&typed) {
                        return None;
                    }
                    if colors == 31 {
                        return Some(parse_quote!(add_any_color(#amount)));
                    }
                    if eval(amount) != Some(1) {
                        return None;
                    }
                    let names = ["White", "Blue", "Black", "Red", "Green"];
                    let listed: Vec<Expr> = (0..5)
                        .filter(|bit| colors & (1 << bit) != 0)
                        .map(|bit| syn::parse_str(&format!("Color::{}", names[bit])).expect("a color"))
                        .collect();
                    Some(parse_quote!(add_one_of(&[#(#listed),*])))
                })(),
                // Adds { pool: ManaPool([..]), ..add_by(f) }
                Expr::Struct(adds) if text_of(&adds.path) == "Adds" && adds.fields.len() == 1 => (|| {
                    let rest = adds.rest.as_ref()?;
                    let function = text_of(rest);
                    let function = function.strip_prefix("add_by (")?.strip_suffix(')')?.trim().to_string();
                    let Expr::Call(pool) = &adds.fields[0].expr else { return None };
                    let Expr::Array(symbols) = pool.args.first()? else { return None };
                    let mut counts = [0i128; 6];
                    for (at, symbol) in symbols.elems.iter().enumerate() {
                        *counts.get_mut(at)? = eval(symbol)?;
                    }
                    if !self.fixed(&function, &counts) {
                        return None;
                    }
                    let names = ["W", "U", "B", "R", "G", "C"];
                    let mana = |at: usize| -> Expr { syn::parse_str(&format!("Mana::{}", names[at])).expect("mana") };
                    let held: Vec<usize> = (0..6).filter(|at| counts[*at] > 0).collect();
                    match held.as_slice() {
                        [one] if counts[*one] == 1 => {
                            let mana = mana(*one);
                            Some(parse_quote!(add(#mana)))
                        }
                        [one] => {
                            let (mana, amount) = (mana(*one), int(counts[*one]));
                            Some(parse_quote!(add_n(#mana, #amount)))
                        }
                        many if many.iter().all(|at| counts[*at] == 1) => {
                            let listed: Vec<Expr> = many.iter().map(|at| mana(*at)).collect();
                            Some(parse_quote!(add_each(&[#(#listed),*])))
                        }
                        _ => None,
                    }
                })(),
                _ => None,
            };
            if let Some(replaced) = replaced {
                *node = replaced;
            }
        }
    }
    let mut shared = Shared(&bodies);
    for expr in definitions(file) {
        shared.visit_expr_mut(expr);
    }
}

/// The predefined tokens with a definition of their own in `cantrip_mechanics::tokens`.
const SHARED_TOKENS: &[(&str, &str)] = &[
    ("Treasure", "TREASURE"),
    ("Food", "FOOD"),
    ("Clue", "CLUE"),
    ("Blood", "BLOOD"),
    ("Powerstone", "POWERSTONE"),
    ("Map", "MAP"),
    ("Gold", "GOLD"),
    ("Mutagen", "MUTAGEN"),
    ("Lander", "LANDER"),
    ("Heartwood", "HEARTWOOD"),
];

/// The words of `token!(Elf Warrior)` a token's definition is named by.
fn token_words(card: &syn::ExprStruct) -> Option<String> {
    let name = card.fields.iter().find(|f| text_of(&f.member) == "name")?;
    let text = text_of(&name.expr);
    let words = text.strip_prefix("token ! (")?.strip_suffix(") . name")?;
    Some(words.trim().to_string())
}

/// Each `pub mod token_N` and `pub mod emblem_N` as items of the file: a token is a
/// static beside the card (`..token!(Words)`), an emblem a static `EMBLEM`, their
/// functions beside the card's. What each module became is returned by its name.
/// A token Cantrip shares, or that the commit before the card will share, is named, not defined again. Which is read from
/// `shared_tokens.tsv`, `card<TAB>the token's static here<TAB>its static in Cantrip`, which `tools/shared_tokens.py` writes
/// by comparing every card's own definitions: run with `MTGISH_LOCAL_TOKENS` set for that, so that each card has them.
fn use_shared_tokens(file: &mut File, card: &str) {
    static SHARED: std::sync::OnceLock<BTreeMap<(&str, &str), &str>> = std::sync::OnceLock::new();
    let shared = SHARED.get_or_init(|| {
        if std::env::var_os("MTGISH_LOCAL_TOKENS").is_some() {
            return BTreeMap::new();
        }
        let rows = include_str!("shared_tokens.tsv").lines();
        rows.filter_map(|row| {
            let mut fields = row.split('\t');
            Some(((fields.next()?, fields.next()?), fields.next()?))
        })
        .collect()
    });
    let mut named: BTreeMap<String, Expr> = BTreeMap::new();
    file.items.retain(|item| {
        let Item::Static(token) = item else { return true };
        let name = token.ident.to_string();
        let Some(path) = shared.get(&(card, name.as_str())).map(|path| path.to_string()) else {
            return true;
        };
        named.insert(name, syn::parse_str(&format!("cantrip_mechanics::tokens::{path}")).expect("a path"));
        false
    });
    struct Named<'a>(&'a BTreeMap<String, Expr>);
    impl VisitMut for Named<'_> {
        fn visit_expr_mut(&mut self, node: &mut Expr) {
            if let Some(path) = ident_of(node).and_then(|name| self.0.get(&name)) {
                *node = path.clone();
                return;
            }
            visit_mut::visit_expr_mut(self, node);
        }
    }
    if !named.is_empty() {
        Named(&named).visit_file_mut(file);
    }
}

/// `else { return Ok(()) };` stays on the line it guards, as in Cantrip's cards. Split, its `return` is a line of its own that
/// no scenario runs on an ability with one target: a target gone from it means the ability never resolves.
fn let_else_on_one_line(text: &str) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut i = 0;
    while i < lines.len() {
        let guards = i + 2 < lines.len()
            && lines[i].trim_end().ends_with(" else {")
            && matches!(lines[i + 1].trim(), "return Ok(());" | "return None;")
            && lines[i + 2].trim() == "};";
        let leave = if guards { lines[i + 1].trim().trim_end_matches(';') } else { "" };
        if guards && lines[i].len() + leave.len() + " };".len() + 1 <= 120 {
            out.push(format!("{} {leave} }};", lines[i].trim_end()));
            i += 3;
        } else {
            out.push(lines[i].to_string());
            i += 1;
        }
    }
    out.join("\n")
}

/// An event's pattern on one line where it fits: split by the formatter's narrower margin, each binding is a line that only
/// counts as run when the arm is taken, which on some events no game reaches.
fn event_patterns_on_one_line(text: &str) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    let field = |line: &str| {
        let line = line.trim();
        let name = line.strip_suffix(',').unwrap_or(line);
        name == ".." || name.chars().all(|c| c.is_alphanumeric() || "_: &".contains(c)) && line.ends_with(',')
    };
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut i = 0;
    while i < lines.len() {
        let head = lines[i].trim_start();
        let opens = lines[i].ends_with(" {") && (head.starts_with("Event::") || head.starts_with("if let Event::") || head.starts_with("let Event::") || head.starts_with("let Event::"));
        let end = opens.then(|| (i + 1..lines.len()).find(|&at| !field(lines[at]))).flatten();
        if let Some(end) = end.filter(|&end| end > i + 1 && lines[end].trim_start().starts_with('}')) {
            let fields: Vec<&str> = lines[i + 1..end].iter().map(|line| line.trim()).collect();
            let fields = fields.join(" ");
            let joined = format!("{} {} {}", lines[i], fields.strip_suffix(',').unwrap_or(&fields), lines[end].trim_start());
            if joined.len() <= 120 {
                out.push(joined);
                i = end + 1;
                continue;
            }
        }
        out.push(lines[i].to_string());
        i += 1;
    }
    out.join("\n")
}

fn flatten_tokens(file: &mut File, taken: &mut BTreeSet<String>) -> BTreeMap<String, syn::Path> {
    let mut became: BTreeMap<String, syn::Path> = BTreeMap::new();
    let mut i = 0;
    while i < file.items.len() {
        let Item::Mod(module) = &mut file.items[i] else {
            i += 1;
            continue;
        };
        let prefix = module.ident.to_string();
        let emblem = prefix.starts_with("emblem_");
        if !emblem && !prefix.starts_with("token_") {
            i += 1;
            continue;
        }
        let Some((_, items)) = module.content.take() else {
            i += 1;
            continue;
        };
        let mut inner = File {
            shebang: None,
            attrs: vec![],
            items,
        };
        let nested = flatten_tokens(&mut inner, taken);
        let card = inner.items.iter().find_map(|item| match item {
            Item::Static(item) if item.ident == "CARD" => match &*item.expr {
                Expr::Struct(card) => Some(card.clone()),
                _ => None,
            },
            _ => None,
        });
        let words = card.as_ref().and_then(token_words);
        let shared = words
            .as_deref()
            .and_then(|w| SHARED_TOKENS.iter().find(|(name, _)| *name == w).map(|(_, c)| *c));
        let replacement: syn::Path = if let Some(shared) = shared {
            syn::parse_str(&format!("cantrip_mechanics::tokens::{shared}")).expect("a path")
        } else {
            let wanted = match words.as_deref() {
                Some(words) if !words.is_empty() => words.to_uppercase().replace(' ', "_"),
                _ if emblem => "EMBLEM".to_string(),
                _ => card.as_ref().and_then(constant_of_name).unwrap_or_else(|| prefix.to_uppercase()),
            };
            let mut unique = wanted.clone();
            let mut n = 2;
            while taken.contains(&unique) {
                unique = format!("{wanted}_{n}");
                n += 1;
            }
            taken.insert(unique.clone());
            syn::parse_str(&unique).expect("a name")
        };
        file.items.remove(i);
        let lifted_statics: BTreeSet<String> = nested.values().map(text_of).collect();
        let mut names: BTreeMap<String, String> = BTreeMap::new();
        for item in &inner.items {
            match item {
                Item::Fn(item) => {
                    names.insert(item.sig.ident.to_string(), format!("{prefix}_{}", item.sig.ident));
                }
                Item::Static(item) if item.ident != "CARD" && !lifted_statics.contains(&item.ident.to_string()) => {
                    names.insert(item.ident.to_string(), format!("{}_{}", prefix.to_uppercase(), item.ident));
                }
                Item::Const(item) => {
                    names.insert(item.ident.to_string(), format!("{}_{}", prefix.to_uppercase(), item.ident));
                }
                _ => {}
            }
        }
        struct Prefix<'a>(&'a BTreeMap<String, String>, &'a syn::Path);
        impl VisitMut for Prefix<'_> {
            fn visit_ident_mut(&mut self, node: &mut syn::Ident) {
                if let Some(new) = self.0.get(&node.to_string()) {
                    *node = syn::Ident::new(new, node.span());
                }
            }
            fn visit_path_mut(&mut self, node: &mut syn::Path) {
                if node.is_ident("CARD") {
                    *node = self.1.clone();
                    return;
                }
                if node.segments.first().is_some_and(|s| s.ident == "super") {
                    let segments: Vec<_> = node.segments.iter().skip(1).cloned().collect();
                    node.segments = segments.into_iter().collect();
                }
                visit_mut::visit_path_mut(self, node);
            }
        }
        let mut lifted = Vec::new();
        for mut item in inner.items {
            if matches!(item, Item::Use(_)) {
                continue;
            }
            Prefix(&names, &replacement).visit_item_mut(&mut item);
            if let Item::Static(token) = &mut item {
                if token.ident == replacement.segments.last().expect("a name").ident
                    && replacement.get_ident().is_some()
                    || token.ident == "CARD"
                {
                    if shared.is_some() {
                        continue;
                    }
                    let name = replacement.get_ident().cloned().expect("a static's name");
                    token.ident = name;
                    if let Expr::Struct(card) = &mut *token.expr {
                        spread_token(card, emblem);
                    }
                } else {
                    token.vis = syn::Visibility::Inherited;
                }
            } else if shared.is_some() {
                continue;
            }
            lifted.push(item);
        }
        struct Refer<'a> {
            module: &'a str,
            card: &'a syn::Path,
            names: &'a BTreeMap<String, String>,
            nested: &'a BTreeMap<String, syn::Path>,
        }
        impl VisitMut for Refer<'_> {
            fn visit_path_mut(&mut self, node: &mut syn::Path) {
                let segments: Vec<String> = node.segments.iter().map(|s| s.ident.to_string()).collect();
                if segments.first().map(String::as_str) == Some(self.module) {
                    match &segments[1..] {
                        [last] if last == "CARD" => {
                            *node = self.card.clone();
                            return;
                        }
                        [last] => {
                            if let Some(new) = self.names.get(last) {
                                *node = syn::parse_str(new).expect("a name");
                                return;
                            }
                        }
                        [inner @ .., last] if last == "CARD" => {
                            if let Some(path) = self.nested.get(&inner.join("::")) {
                                *node = path.clone();
                                return;
                            }
                        }
                        _ => {}
                    }
                }
                visit_mut::visit_path_mut(self, node);
            }
        }
        let mut refer = Refer {
            module: &prefix,
            card: &replacement,
            names: &names,
            nested: &nested,
        };
        for item in &mut file.items {
            refer.visit_item_mut(item);
        }
        for item in &mut lifted {
            refer.visit_item_mut(item);
        }
        let count = lifted.len();
        file.items.splice(i..i, lifted);
        i += count;
        became.extend(nested.into_iter().map(|(path, card)| (format!("{prefix}::{path}"), card)));
        became.insert(prefix, replacement);
    }
    became
}

/// `CADET` for a token whose definition says `name: "Cadet"`.
fn constant_of_name(card: &syn::ExprStruct) -> Option<String> {
    let name = card.fields.iter().find(|f| text_of(&f.member) == "name")?;
    let Expr::Lit(syn::ExprLit { lit: Lit::Str(name), .. }) = &name.expr else {
        return None;
    };
    let constant: String = name
        .value()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == ' ')
        .map(|c| if c == ' ' { '_' } else { c.to_ascii_uppercase() })
        .collect();
    constant.starts_with(|c: char| c.is_ascii_uppercase()).then_some(constant)
}

/// A token's definition, leaving to `token!` what it says.
fn spread_token(card: &mut syn::ExprStruct, emblem: bool) {
    let Some(words) = token_words(card) else {
        // A token named on the card says everything else, as an emblem says next to nothing.
        let plain = ["SubtypeSet :: of (& [])", "TypeSet :: of (& [])", "& []", "ManaCost :: ZERO"];
        let kept: Vec<_> = card
            .fields
            .iter()
            .filter(|field| {
                let name = text_of(&field.member);
                let value = text_of(&field.expr);
                let blank = is_default_field(&field.expr)
                    || plain.contains(&value.as_str())
                    || value == "Supertypes { basic : false , legendary : false , snow : false }";
                name == "name" || name == "has_cost" || (!blank && !(emblem && name == "oracle"))
            })
            .cloned()
            .collect();
        card.fields = kept.into_iter().collect();
        return;
    };
    let macro_words: proc_macro2::TokenStream = words.parse().expect("subtype words");
    let spoken = ["name", "token_words", "subtypes", "oracle", "cost", "has_cost", "supertypes"];
    let kept: Vec<_> = card
        .fields
        .iter()
        .filter(|field| {
            let name = text_of(&field.member);
            let plain_supertypes = text_of(&field.expr) == "Supertypes { basic : false , legendary : false , snow : false }";
            !is_default_field(&field.expr)
                && (!spoken.contains(&name.as_str())
                    || (name == "supertypes" && !plain_supertypes))
        })
        .cloned()
        .collect();
    card.fields = kept.into_iter().collect();
    card.rest = Some(Box::new(parse_quote!(token!(#macro_words))));
}

/// Text compared the way Cantrip's own check compares it.
fn loose(text: &str) -> String {
    let text = text
        .replace(['\u{2019}', '\u{2018}'], "'")
        .replace(['\u{201c}', '\u{201d}'], "\"")
        .replace(['\u{2212}', '\u{2014}'], "-");
    text.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// How many of `text`'s words `line` has.
fn overlap(text: &str, line: &str) -> usize {
    let words: BTreeSet<&str> = line.split_whitespace().collect();
    text.split_whitespace().filter(|w| words.contains(w)).count()
}

/// An ability's text that is not on the card, as the printed line it most resembles. The
/// generator says a line it could not place in its own words, and the card must say what
/// is printed; no line is said twice while another is left.
fn say_printed_text(file: &mut File, printed: &str) {
    struct Say<'a> {
        lines: Vec<&'a str>,
        oracle: String,
        used: BTreeSet<usize>,
    }
    impl Say<'_> {
        fn is_printed(&self, text: &str) -> bool {
            let text = loose(text);
            let text = text.trim_end_matches(['.', ',', ';']);
            // "Equip {2} or {B}" is two abilities, each with its own price.
            self.oracle.contains(text)
                || self.lines.iter().filter_map(|line| super::costs::worded_apart(line)).any(|(one, two)| {
                    loose(&one).trim_end_matches(['.', ',', ';']) == text
                        || loose(&two).trim_end_matches(['.', ',', ';']) == text
                })
        }
    }
    impl VisitMut for Say<'_> {
        fn visit_expr_call_mut(&mut self, node: &mut syn::ExprCall) {
            visit_mut::visit_expr_call_mut(self, node);
            if !matches!(
                text_of(&node.func).as_str(),
                "triggered" | "ability" | "mana_ability" | "static_ability" | "activated" | "loyalty_ability"
                    | "replacement" | "prevention"
            ) {
                return;
            }
            let Some(Expr::Lit(syn::ExprLit { lit: Lit::Str(text), .. })) = node.args.last_mut() else {
                return;
            };
            let said = text.value();
            if self.is_printed(&said) || self.lines.is_empty() {
                return;
            }
            let wanted = loose(&said);
            let best = (0..self.lines.len())
                .max_by_key(|&at| (overlap(&wanted, &loose(self.lines[at])), !self.used.contains(&at), usize::MAX - at))
                .expect("a line");
            self.used.insert(best);
            *text = syn::LitStr::new(self.lines[best], text.span());
        }
    }
    let lines: Vec<&str> = printed.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let mut say = Say {
        oracle: loose(printed),
        lines,
        used: BTreeSet::new(),
    };
    for expr in definitions(file) {
        say.visit_expr_mut(expr);
    }
}
