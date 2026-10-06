mod layout;

use crate::ast::{BinaryOp, Builtin, NixExpr, NixKind};
use layout::{Doc, attributed, concat, group, hard_line, line, nest, soft, text};
use rusnix_ir::Origin;
use serde::{Deserialize, Serialize};

/// A range of generated Nix text linked to the Rust operation that created it.
/// `start` is inclusive and `end` exclusive. Nested ranges let a Nix error
/// position identify the most specific Rust expression and its surrounding
/// configuration path.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourceSpan {
    /// Inclusive UTF-8 byte offset from the start of [`Generated::source`].
    pub start: usize,
    /// Exclusive byte offset; the byte at `end` belongs outside this span.
    pub end: usize,
    /// Rust origin of the expression occupying this range.
    pub origin: Origin,
    /// Enclosing Rust operations, such as the containing field assignment.
    /// These explain the configuration path even without runtime error markers.
    #[serde(default)]
    pub enclosing: Vec<Origin>,
    /// Whether this range identifies an operation’s own failure without a runtime marker.
    /// Used when choosing between a generated failure position and an enclosing call;
    /// literal values and function definitions cannot displace the consuming operation.
    #[serde(default)]
    pub diagnostic_site: bool,
}

/// Nix source produced by the compiler, with its links back to Rust locations.
/// Save the source and spans together to translate later Nix error positions.
/// This is output to inspect or evaluate; normal configuration authors edit
/// Rust rather than this generated text.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Generated {
    /// Complete rendered expression, including selected runtime error contexts.
    /// Fine-grained origin comments are present only when explicitly requested.
    pub source: String,
    /// Text ranges linked to Rust operations and their enclosing configuration fields.
    pub spans: Vec<SourceSpan>,
}

/// Control optional aids for manually inspecting generated Nix.
/// Source maps and runtime diagnostic boundaries are preserved regardless of
/// these options. Each rendering produces spans for its own source text.
#[derive(Clone, Copy, Debug, Default)]
pub struct RenderOptions {
    /// Show `# rn-...` before each attributed expression for human inspection.
    /// Defaults to false. Diagnostics use source maps and selected runtime
    /// contexts, not these comments.
    pub origin_comments: bool,
}

impl Generated {
    /// Find the first source-map origin with this ID; reused expressions may have several spans.
    pub fn origin(&self, id: &str) -> Option<&Origin> {
        self.spans
            .iter()
            .find(|span| span.origin.id == id)
            .map(|span| &span.origin)
    }

    /// Find the narrowest origin at a one-based generated line and byte column.
    /// Returns `None` for invalid positions or compiler syntax with no attribution.
    pub fn at_position(&self, line: usize, column: usize) -> Option<&Origin> {
        self.span_at_position(line, column).map(|span| &span.origin)
    }

    /// Find the narrowest half-open span containing a one-based line and byte column.
    /// This preserves ancestry when runtime error contexts have disappeared.
    pub fn span_at_position(&self, line: usize, column: usize) -> Option<&SourceSpan> {
        if line == 0 || column == 0 {
            return None;
        }

        let start = if line == 1 {
            0
        } else {
            self.source.match_indices('\n').nth(line - 2)?.0 + 1
        };
        let line_end = self.source[start..]
            .find('\n')
            .map_or(self.source.len(), |n| start + n);
        let offset = start.checked_add(column - 1)?;
        if offset >= line_end {
            return None;
        }

        self.spans
            .iter()
            .filter(|span| span.start <= offset && offset < span.end)
            .min_by_key(|span| span.end - span.start)
    }
}

/// Render advanced backend syntax into escaped Nix source and attribution ranges.
/// Does not validate or evaluate the AST; ordinary callers should use [`crate::compile`]
/// or [`crate::nixos::compile_module`] to validate semantic IR first.
/// Layout uses a 100-character target and two-space indentation. Indivisible
/// literals can exceed that target; string contents are never reformatted.
pub fn render(ast: &NixExpr) -> Generated {
    render_with_options(ast, RenderOptions::default())
}

/// Render Nix with optional origin comments for manual inspection.
/// This uses the same renderer and runtime contexts as [`render`]; spans are
/// recomputed for this output rather than reused from another rendering.
pub fn render_with_options(ast: &NixExpr, options: RenderOptions) -> Generated {
    let document = expression(ast, options);
    layout::render(&document, 100)
}

/// Preserve an expression's attribution until the final layout writes its bytes.
fn expression(expr: &NixExpr, options: RenderOptions) -> Doc<'_> {
    let mut body = syntax(&expr.kind, options);
    let Some(origin) = &expr.origin else {
        return body;
    };

    if expr.error_context {
        body = application(
            "(builtins.addErrorContext",
            vec![text(quote(&origin.id)), parens(body)],
        );
    }
    let diagnostic_site = !expr.error_context
        && matches!(
            expr.kind,
            NixKind::Call(..)
                | NixKind::Apply(..)
                | NixKind::Binary(..)
                | NixKind::If(..)
                | NixKind::Assert(..)
                | NixKind::Select(..)
                | NixKind::ArgumentSelect(..)
        );
    body = attributed(origin, diagnostic_site, body);

    if options.origin_comments {
        // Comments are an inspection aid, outside this expression's own span.
        concat(vec![text(format!("# {}", origin.id)), hard_line(), body])
    } else {
        body
    }
}

/// Keep existing parentheses while letting the child choose its layout.
fn parens(body: Doc<'_>) -> Doc<'_> {
    // Parentheses preserve syntax; only the expression inside owns indentation.
    concat(vec![text("("), body, text(")")])
}

fn join<'a>(parts: Vec<Doc<'a>>, separator: impl Fn() -> Doc<'a>) -> Doc<'a> {
    let mut joined = Vec::new();
    for (index, part) in parts.into_iter().enumerate() {
        if index > 0 {
            joined.push(separator());
        }
        joined.push(part);
    }

    concat(joined)
}

/// Collections stay inline when their entire group fits; otherwise each item
/// gets a line and may choose its own independent layout.
fn collection<'a>(open: &str, items: Vec<Doc<'a>>, close: &str) -> Doc<'a> {
    if items.is_empty() {
        return text(format!("{open} {close}"));
    }

    group(concat(vec![
        text(open),
        nest(concat(vec![line(), join(items, line)])),
        line(),
        text(close),
    ]))
}

fn application<'a>(head: &str, arguments: Vec<Doc<'a>>) -> Doc<'a> {
    let mut parts = vec![text(head)];
    for argument in arguments {
        parts.push(nest(concat(vec![line(), argument])));
    }
    parts.extend([soft(), text(")")]);

    group(concat(parts))
}

fn binding<'a>(key: String, value: Doc<'a>) -> Doc<'a> {
    group(concat(vec![
        text(format!("{key} =")),
        nest(concat(vec![line(), value])),
        text(";"),
    ]))
}

fn syntax(kind: &NixKind, options: RenderOptions) -> Doc<'_> {
    let child = |expr| expression(expr, options);
    match kind {
        NixKind::Bool(v) => text(if *v { "true" } else { "false" }),
        NixKind::Int(i64::MIN) => text("(-9223372036854775807 - 1)"),
        NixKind::Int(v) if *v < 0 => text(format!("({v})")),
        NixKind::Int(v) => text(v.to_string()),
        NixKind::Float(v) => {
            // Preserve floating types and Rust's shortest round-trip precision.
            let mut literal = format!("{v:?}");
            if let Some(exponent) = literal.find('e')
                && !literal[..exponent].contains('.')
            {
                literal.insert_str(exponent, ".0");
            }
            text(format!("({literal})"))
        }
        NixKind::Null => text("null"),
        NixKind::String(v) => text(quote(v)),
        NixKind::Path(v) | NixKind::Variable(v) => text(v.clone()),
        NixKind::Select(value, path) => {
            let mut parts = vec![parens(child(value))];
            parts.extend(path.iter().map(|part| text(format!(".{}", quote(part)))));
            concat(parts)
        }
        NixKind::ArgumentSelect(value, path) => {
            if path.is_empty() {
                return child(value);
            }

            let mut parts = vec![parens(child(value))];
            parts.extend(path.iter().map(|part| {
                let attribute = if attribute_identifier(part) {
                    part.clone()
                } else {
                    quote(part)
                };
                text(format!(".{attribute}"))
            }));
            concat(parts)
        }
        NixKind::Lambda(argument, body) => group(concat(vec![
            text(format!("({argument}:")),
            nest(concat(vec![line(), child(body)])),
            soft(),
            text(")"),
        ])),
        NixKind::Function(arguments, body) => {
            let mut parameters: Vec<_> = arguments.iter().map(|name| text(name.clone())).collect();
            if parameters.is_empty() {
                // Preserve the backend constructor's existing tokens even for
                // this invalid compiler-owned empty pattern.
                parameters.push(text(""));
            }
            parameters.push(text("..."));
            function(parameters, child(body))
        }
        NixKind::ArgumentFunction(arguments, body) => {
            let parameters = arguments
                .iter()
                .map(|(name, default)| match default {
                    None => text(name.clone()),
                    Some(value) => group(concat(vec![
                        text(format!("{name} ?")),
                        nest(concat(vec![line(), parens(child(value))])),
                    ])),
                })
                .collect();
            function(parameters, child(body))
        }
        NixKind::Group(value) => parens(child(value)),
        NixKind::List(items) => collection("[", items.iter().map(child).collect(), "]"),
        NixKind::AttrSet(bindings) => collection(
            "{",
            bindings
                .iter()
                .map(|(path, value)| {
                    let key = path
                        .iter()
                        .map(|part| quote(part))
                        .collect::<Vec<_>>()
                        .join(".");
                    binding(key, child(value))
                })
                .collect(),
            "}",
        ),
        NixKind::Call(builtin, args) => application(
            match builtin {
                Builtin::Div => "(builtins.div",
                Builtin::Throw => "(builtins.throw",
                Builtin::Import => "(builtins.import",
                Builtin::GetAttr => "(builtins.getAttr",
                Builtin::ToPath => "(builtins.toPath",
                Builtin::ToString => "(builtins.toString",
            },
            args.iter().map(|arg| parens(child(arg))).collect(),
        ),
        NixKind::Binary(op, left, right) => {
            let operator = match op {
                BinaryOp::Equal => "==",
                BinaryOp::GreaterEqual => ">=",
                BinaryOp::LessEqual => "<=",
                BinaryOp::And => "&&",
                BinaryOp::Add => "+",
                BinaryOp::AttrMerge => "//",
            };
            group(concat(vec![
                text("("),
                child(left),
                nest(concat(vec![
                    line(),
                    text(format!("{operator} ")),
                    child(right),
                ])),
                soft(),
                text(")"),
            ]))
        }
        NixKind::If(condition, yes, no) => group(concat(vec![
            text("(if "),
            child(condition),
            text(" then"),
            nest(concat(vec![line(), child(yes)])),
            line(),
            text("else"),
            nest(concat(vec![line(), child(no)])),
            soft(),
            text(")"),
        ])),
        NixKind::Assert(condition, value) => group(concat(vec![
            text("(assert "),
            child(condition),
            text(";"),
            nest(concat(vec![line(), child(value)])),
            soft(),
            text(")"),
        ])),
        NixKind::Let(name, value, body) => group(concat(vec![
            text("(let"),
            nest(concat(vec![line(), binding(name.clone(), child(value))])),
            line(),
            text("in"),
            nest(concat(vec![line(), child(body)])),
            soft(),
            text(")"),
        ])),
        NixKind::Apply(function, argument) => group(concat(vec![
            text("("),
            parens(child(function)),
            nest(concat(vec![line(), parens(child(argument))])),
            soft(),
            text(")"),
        ])),
    }
}

fn function<'a>(parameters: Vec<Doc<'a>>, body: Doc<'a>) -> Doc<'a> {
    let pattern = if parameters.is_empty() {
        text("{  }:")
    } else {
        group(concat(vec![
            text("{"),
            nest(concat(vec![
                line(),
                join(parameters, || concat(vec![text(","), line()])),
            ])),
            line(),
            text("}:"),
        ]))
    };
    group(concat(vec![
        text("("),
        pattern,
        nest(concat(vec![line(), body])),
        soft(),
        text(")"),
    ]))
}

/// Quoting is required for literal punctuation, interpolation and reserved words.
fn attribute_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || "_-'".contains(c))
        && ![
            "if", "then", "else", "assert", "with", "let", "in", "rec", "inherit", "or",
        ]
        .contains(&name)
}

/// Nix string literals, including escaping interpolation; JSON escaping alone
/// would wrongly leave `${...}` executable and use unsupported `\u` escapes.
pub(crate) fn quote(value: &str) -> String {
    let mut out = String::from("\"");
    let mut chars = value.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '$' if chars.peek() == Some(&'{') => out.push_str("\\$"),
            _ => out.push(c),
        }
    }

    out.push('"');
    out
}

#[cfg(test)]
mod tests;
