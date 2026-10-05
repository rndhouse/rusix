//! Lowering, code generation, isolated evaluation, and diagnostic translation.
pub mod ast;

pub mod diagnostic;

pub mod interop;

pub mod isolated;

pub mod nixos;

mod render;

use ast::{BinaryOp, Builtin, NixExpr, NixKind};
pub use diagnostic::{Diagnostic, DiagnosticKind, DiagnosticOrigin, OriginRole, Provenance};
pub use isolated::{Evaluation, NixSession};
pub use render::{Generated, SourceSpan, render};
use rusnix_ir::{Config, Node, ValueKind};

pub fn compile(config: &Config) -> Result<Generated, Box<Diagnostic>> {
    config
        .validate()
        .map_err(|error| Box::new(Diagnostic::validation(error.origin, error.message)))?;
    for assignment in &config.assignments {
        if let Some(reference) = option_reference(&assignment.value) {
            return Err(Diagnostic::validation(
                reference.origin.clone(),
                "symbolic NixOS option references require NixosModule lowering".into(),
            )
            .into());
        }
    }
    Ok(render(&lower(config)))
}

/// Find scoped dependencies without evaluating any values.
fn option_reference(node: &Node) -> Option<&Node> {
    match &node.kind {
        ValueKind::OptionReference(_) => Some(node),
        ValueKind::List(items) => items.iter().find_map(option_reference),
        ValueKind::AttrSet(fields) | ValueKind::OpaqueRecord(fields) => {
            fields.iter().find_map(|(_, node)| option_reference(node))
        }
        ValueKind::Apply(left, right) | ValueKind::Divide(left, right) => {
            option_reference(left).or_else(|| option_reference(right))
        }
        ValueKind::Select(value, _)
        | ValueKind::ToText(value)
        | ValueKind::StringPrefix { value, .. }
        | ValueKind::InRange { value, .. } => option_reference(value),
        ValueKind::Bool(_)
        | ValueKind::Int(_)
        | ValueKind::Float(_)
        | ValueKind::Null
        | ValueKind::String(_)
        | ValueKind::Reference(_) => None,
    }
}

fn lower(config: &Config) -> NixExpr {
    let assignments = config
        .assignments
        .iter()
        .map(|assignment| {
            let value = lower_value(&assignment.value);
            // Assignment and value keep separate identities, even for primitives.
            let annotated =
                NixExpr::attributed(NixKind::Group(Box::new(value)), assignment.origin.clone());
            (assignment.path_segments().to_vec(), annotated)
        })
        .collect();
    NixExpr::attributed(NixKind::AttrSet(assignments), config.origin.clone())
}

fn variable(name: &str) -> NixExpr {
    NixExpr::plain(NixKind::Variable(name.into()))
}

fn integer(value: i64) -> NixExpr {
    NixExpr::plain(NixKind::Int(value))
}

fn binary(op: BinaryOp, left: NixExpr, right: NixExpr) -> NixExpr {
    NixExpr::plain(NixKind::Binary(op, Box::new(left), Box::new(right)))
}

fn lower_value(node: &Node) -> NixExpr {
    let kind = match &node.kind {
        ValueKind::Bool(v) => NixKind::Bool(*v),
        ValueKind::Int(v) => NixKind::Int(*v),
        ValueKind::Float(v) => NixKind::Float(*v),
        ValueKind::Null => NixKind::Null,
        ValueKind::String(v) => NixKind::String(v.clone()),
        ValueKind::List(items) => NixKind::List(items.iter().map(lower_value).collect()),
        ValueKind::AttrSet(fields) | ValueKind::OpaqueRecord(fields) => NixKind::AttrSet(
            fields
                .iter()
                .map(|(name, node)| (vec![name.clone()], lower_value(node)))
                .collect(),
        ),
        ValueKind::Reference(reference) => return interop::lower_reference(reference),
        ValueKind::OptionReference(path) => {
            NixKind::Select(Box::new(variable("config")), path.parts().to_vec())
        }
        ValueKind::ToText(value) => NixKind::Call(Builtin::ToString, vec![lower_value(value)]),
        ValueKind::StringPrefix { prefix, value } => NixKind::Binary(
            BinaryOp::Add,
            Box::new(NixExpr::plain(NixKind::String(prefix.clone()))),
            Box::new(lower_value(value)),
        ),
        ValueKind::Apply(function, argument) => NixKind::Apply(
            Box::new(lower_value(function)),
            Box::new(lower_value(argument)),
        ),
        ValueKind::Select(value, path) => {
            return NixExpr::contextual(
                interop::select(lower_value(value), path).kind,
                node.origin.clone(),
            );
        }
        ValueKind::Divide(left, right) => {
            NixKind::Call(Builtin::Div, vec![lower_value(left), lower_value(right)])
        }
        ValueKind::InRange {
            value,
            min,
            max,
            message,
        } => {
            let condition = binary(
                BinaryOp::And,
                binary(
                    BinaryOp::GreaterEqual,
                    variable("__rusnix_range"),
                    integer(*min),
                ),
                binary(
                    BinaryOp::LessEqual,
                    variable("__rusnix_range"),
                    integer(*max),
                ),
            );
            let body = NixExpr::plain(NixKind::If(
                Box::new(condition),
                Box::new(variable("__rusnix_range")),
                Box::new(NixExpr::plain(NixKind::Call(
                    Builtin::Throw,
                    vec![NixExpr::plain(NixKind::String(message.clone()))],
                ))),
            ));
            NixKind::Let(
                "__rusnix_range".into(),
                Box::new(lower_value(value)),
                Box::new(body),
            )
        }
    };
    if matches!(
        node.kind,
        ValueKind::Divide(..)
            | ValueKind::InRange { .. }
            | ValueKind::Apply(..)
            | ValueKind::OptionReference(_)
            | ValueKind::ToText(_)
            | ValueKind::StringPrefix { .. }
    ) {
        NixExpr::contextual(kind, node.origin.clone())
    } else {
        NixExpr::attributed(kind, node.origin.clone())
    }
}
