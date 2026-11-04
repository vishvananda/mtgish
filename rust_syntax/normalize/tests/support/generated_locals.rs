use proc_macro2::{Group, Ident, TokenStream, TokenTree};
fn normalize(tokens: TokenStream) -> TokenStream {
    tokens
        .into_iter()
        .map(|token| match token {
            TokenTree::Group(group) => {
                let mut new = Group::new(group.delimiter(), normalize(group.stream()));
                new.set_span(group.span());
                TokenTree::Group(new)
            }
            TokenTree::Ident(ident) => {
                let name = ident.to_string();
                let mut parts: Vec<_> = name.split('_').map(str::to_owned).collect();
                if matches!(
                    parts.first().map(String::as_str),
                    Some("carried" | "selected" | "paid" | "hoisted" | "objects" | "moved" | "player")
                ) && parts
                    .get(1)
                    .is_some_and(|part| part.chars().all(|c| c.is_ascii_digit()))
                {
                    parts[1] = "LOCAL".into();
                    TokenTree::Ident(Ident::new(&parts.join("_"), ident.span()))
                } else {
                    TokenTree::Ident(ident)
                }
            }
            other => other,
        })
        .collect()
}
/// Compare scratch-local spelling only; preserve source bindings, target indices,
/// carried payload slots, literals, API calls, and every other token.
pub fn contains(code: &str, fragment: &str) -> bool {
    if code.contains(fragment) {
        return true;
    }
    match (code.parse::<TokenStream>(), fragment.parse::<TokenStream>()) {
        (Ok(code), Ok(fragment)) => normalize(code)
            .to_string()
            .contains(&normalize(fragment).to_string()),
        _ => code.contains(fragment),
    }
}
