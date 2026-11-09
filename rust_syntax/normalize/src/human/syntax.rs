//! A bounded data language: constructors, records, lists, scalar values and bindings.
use serde_json::{Number, Value};
#[derive(Clone, Debug)]
pub(super) enum Node {
    Scalar(Value),
    Name(String),
    List(Vec<Node>),
    Record(Option<String>, Vec<(String, Node)>),
    Call(String, Vec<Node>),
    Bind(String, Box<Node>),
}
struct Parser<'a> {
    text: &'a str,
    at: usize,
    nodes: usize,
}
impl Parser<'_> {
    fn space(&mut self) {
        loop {
            while self
                .text
                .as_bytes()
                .get(self.at)
                .is_some_and(u8::is_ascii_whitespace)
            {
                self.at += 1;
            }
            if self.text[self.at..].starts_with("//") {
                while self
                    .text
                    .as_bytes()
                    .get(self.at)
                    .is_some_and(|b| *b != b'\n')
                {
                    self.at += 1;
                }
            } else {
                break;
            }
        }
    }
    fn take(&mut self, c: u8) -> bool {
        self.space();
        if self.text.as_bytes().get(self.at) == Some(&c) {
            self.at += 1;
            true
        } else {
            false
        }
    }
    fn expect(&mut self, c: u8) -> Result<(), String> {
        if self.take(c) {
            Ok(())
        } else {
            Err(format!("expected '{}' at byte {}", c as char, self.at))
        }
    }
    fn word(&mut self) -> Result<String, String> {
        self.space();
        let start = self.at;
        while self
            .text
            .as_bytes()
            .get(self.at)
            .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_')
        {
            self.at += 1;
        }
        if start == self.at {
            return Err(format!("expected a name at byte {}", start));
        }
        Ok(self.text[start..self.at].into())
    }
    fn string(&mut self) -> Result<String, String> {
        self.space();
        let start = self.at;
        self.expect(b'"')?;
        let mut escaped = false;
        while let Some(&b) = self.text.as_bytes().get(self.at) {
            self.at += 1;
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                return serde_json::from_str(&self.text[start..self.at]).map_err(|e| e.to_string());
            }
        }
        Err("unterminated string".into())
    }
    fn fields(&mut self, name: Option<String>, depth: usize) -> Result<Node, String> {
        let mut fields = vec![];
        while !self.take(b'}') {
            let key = if self.text[self.at..].starts_with('"') {
                self.string()?
            } else {
                self.word()?
            };
            if fields.iter().any(|(k, _)| k == &key) {
                return Err(format!("duplicate field {key}"));
            }
            self.expect(b':')?;
            fields.push((key, self.node(depth + 1)?));
            if !self.take(b',') {
                self.expect(b'}')?;
                break;
            }
        }
        Ok(Node::Record(name, fields))
    }
    fn items(&mut self, end: u8, depth: usize) -> Result<Vec<Node>, String> {
        let mut out = vec![];
        while !self.take(end) {
            out.push(self.node(depth + 1)?);
            if !self.take(b',') {
                self.expect(end)?;
                break;
            }
        }
        Ok(out)
    }
    fn node(&mut self, depth: usize) -> Result<Node, String> {
        self.space();
        self.nodes += 1;
        if depth > 128 || self.nodes > 200_000 {
            return Err("card code exceeds depth/work limit".into());
        }
        if self.take(b'{') {
            return self.fields(None, depth);
        }
        if self.take(b'[') {
            return Ok(Node::List(self.items(b']', depth)?));
        }
        let Some(&b) = self.text.as_bytes().get(self.at) else {
            return Err("unexpected end of card code".into());
        };
        if b == b'"' {
            return Ok(Node::Scalar(Value::String(self.string()?)));
        }
        if b == b'-' || b.is_ascii_digit() {
            let start = self.at;
            self.at += 1;
            while self
                .text
                .as_bytes()
                .get(self.at)
                .is_some_and(u8::is_ascii_digit)
            {
                self.at += 1;
            }
            let n: Number = self.text[start..self.at]
                .parse()
                .map_err(|_| "integer outside i64/u64 range")?;
            return Ok(Node::Scalar(Value::Number(n)));
        }
        let name = self.word()?;
        if self.take(b'=') {
            return Ok(Node::Bind(name, Box::new(self.node(depth + 1)?)));
        }
        if self.take(b'{') {
            return self.fields(Some(name), depth);
        }
        if self.take(b'(') {
            return Ok(Node::Call(name, self.items(b')', depth)?));
        }
        Ok(match name.as_str() {
            "true" => Node::Scalar(Value::Bool(true)),
            "false" => Node::Scalar(Value::Bool(false)),
            "None" | "null" => Node::Scalar(Value::Null),
            _ => Node::Name(name),
        })
    }
}
pub(super) fn parse(text: &str) -> Result<Node, String> {
    if text.len() > 2 * 1024 * 1024 {
        return Err("card code exceeds 2 MiB".into());
    }
    let mut parser = Parser {
        text,
        at: 0,
        nodes: 0,
    };
    let value = parser.node(0)?;
    parser.space();
    if parser.at != text.len() {
        return Err(format!("unexpected text at byte {}", parser.at));
    }
    Ok(value)
}
pub(super) fn render(n: &Node, indent: usize) -> String {
    match n {
        Node::Scalar(Value::Null) => "None".into(),
        Node::Scalar(v) => v.to_string(),
        Node::Name(s) => s.clone(),
        Node::Bind(name, n) => format!("{name} = {}", render(n, indent)),
        Node::Call(name, items) => format!(
            "{name}({})",
            items
                .iter()
                .map(|x| render(x, indent))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Node::List(items) if items.is_empty() => "[]".into(),
        Node::List(items)
            if items
                .iter()
                .all(|n| matches!(n, Node::Scalar(_) | Node::Name(_))) =>
        {
            format!(
                "[{}]",
                items
                    .iter()
                    .map(|x| render(x, indent))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
        Node::List(items) => format!(
            "[\n{}\n{}]",
            items
                .iter()
                .map(|x| format!("{}{},", "  ".repeat(indent + 1), render(x, indent + 1)))
                .collect::<Vec<_>>()
                .join("\n"),
            "  ".repeat(indent)
        ),
        Node::Record(name, fields) => {
            let prefix = name.as_deref().unwrap_or("");
            if fields.is_empty() {
                return format!("{prefix} {{}}");
            }
            format!(
                "{prefix} {{\n{}\n{}}}",
                fields
                    .iter()
                    .map(|(k, v)| format!(
                        "{}{}: {},",
                        "  ".repeat(indent + 1),
                        k,
                        render(v, indent + 1)
                    ))
                    .collect::<Vec<_>>()
                    .join("\n"),
                "  ".repeat(indent)
            )
        }
    }
}
