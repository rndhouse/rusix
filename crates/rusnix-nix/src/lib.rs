//! Generate Nix source from Rust configuration and evaluate it in isolation.
//!
//! Nix evaluates configuration expressions and describes software builds. This
//! crate compiles values authored with `rusnix-ir` into that language. Compilation
//! happens in Rust; evaluation is a separate step using [`NixSession`].
//!
//! [`compile`] generates ordinary Nix configuration, while
//! [`nixos::compile_module`] generates a module that NixOS combines with other
//! modules. Generated artifacts retain Rust locations for error reporting through
//! [`Diagnostic`]. Evaluations use temporary, disposable Nix stores, without
//! building packages or activating a system. [`ast`] is an advanced code-generation
//! API; normal authors use `rusnix-ir` instead.
#![warn(missing_docs)]

pub mod ast;

pub mod diagnostic;

mod interop;

pub mod isolated;

pub mod nixos;

mod render;

#[cfg(test)]
mod context_audit;

#[cfg(test)]
mod render_audit;

use ast::{BinaryOp, Builtin, NixExpr, NixKind};
pub use diagnostic::{Diagnostic, DiagnosticKind, DiagnosticOrigin, OriginRole, Provenance};
pub use isolated::{Evaluation, NixSession};
pub use render::{Generated, RenderOptions, SourceSpan, render, render_with_options};
use rusnix_ir::{Config, Node, ValueKind};
use std::{cell::Cell, rc::Rc};

/// Generate Nix source for a group of settings described by [`Config`].
/// This validates the Rust description and returns source plus the information
/// needed to map generated error locations back to Rust. It does not run Nix.
/// References to final NixOS options or the system’s supplied package set require
/// [`nixos::compile_module`] and are rejected by this standalone compiler.
pub fn compile(config: &Config) -> Result<Generated, Box<Diagnostic>> {
    compile_with_options(config, RenderOptions::default())
}

/// Generate validated Nix with optional origin comments for manual inspection.
/// Normal [`compile`] output omits these comments. Both forms retain the same
/// origins, ancestry and runtime boundaries, with spans for their own text.
///
/// ```
/// use rusnix_ir::Config;
/// use rusnix_nix::{RenderOptions, compile_with_options};
/// let annotated = compile_with_options(
///     &Config::new().set("enabled", true),
///     RenderOptions { origin_comments: true },
/// ).unwrap();
/// assert!(annotated.source.contains("# rn-"));
/// ```
pub fn compile_with_options(
    config: &Config,
    options: RenderOptions,
) -> Result<Generated, Box<Diagnostic>> {
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

    Ok(render_with_options(&lower(config), options))
}

/// Find scoped dependencies without evaluating any values.
fn option_reference(node: &Node) -> Option<&Node> {
    scoped_reference(node, false)
}

fn module_package_reference(node: &Node) -> Option<&Node> {
    scoped_reference(node, true)
}

fn scoped_reference(node: &Node, packages_only: bool) -> Option<&Node> {
    let option_reference = |node| scoped_reference(node, packages_only);
    match &node.kind {
        ValueKind::OptionReference(_) => {
            if packages_only {
                None
            } else {
                Some(node)
            }
        }
        ValueKind::List(items) => items.iter().find_map(option_reference),
        ValueKind::AttrSet(fields) | ValueKind::OpaqueRecord(fields) => {
            fields.iter().find_map(|(_, node)| option_reference(node))
        }
        ValueKind::Apply(left, right)
        | ValueKind::AttrMerge(left, right)
        | ValueKind::Assert(left, right)
        | ValueKind::Divide(left, right)
        | ValueKind::Equal(left, right) => {
            option_reference(left).or_else(|| option_reference(right))
        }
        ValueKind::Select(value, _)
        | ValueKind::ToText(value)
        | ValueKind::StringPrefix { value, .. }
        | ValueKind::InRange { value, .. } => option_reference(value),
        ValueKind::Function { body, .. } => option_reference(body),
        ValueKind::FunctionAttrs { defaults, body, .. } => defaults
            .iter()
            .find_map(|(_, v)| option_reference(v))
            .or_else(|| option_reference(body)),
        ValueKind::If(condition, yes, no) => option_reference(condition)
            .or_else(|| option_reference(yes))
            .or_else(|| option_reference(no)),
        ValueKind::Reference(reference)
            if matches!(
                reference.source,
                rusnix_ir::interop::Source::NixosPackages { .. }
            ) =>
        {
            Some(node)
        }
        ValueKind::Parameter(_) => None,
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
    lower_scoped(node, &[])
}

/// Runtime markers are reserved for failures that lose their operation position.
#[derive(Clone, Copy)]
enum DiagnosticBoundary {
    /// Nix may report only the external implementation of the called function.
    OpaqueCall,
    /// Explicit constraints must identify the validating operation.
    Validation,
    /// Nix string addition can blame the operand rather than its consuming operation.
    Coercion,
    /// A merged option can force a foreign definition without a generated lookup frame.
    FinalOption,
    /// Nix's union type error can report only the enclosing field, not the // operation.
    RecordUnion,
}

/// Ordinary expression failures use generated positions and static span ancestry.
fn runtime_boundary(kind: &ValueKind) -> Option<DiagnosticBoundary> {
    match kind {
        ValueKind::Apply(..) => Some(DiagnosticBoundary::OpaqueCall),
        ValueKind::AttrMerge(..) => Some(DiagnosticBoundary::RecordUnion),
        ValueKind::InRange { .. } => Some(DiagnosticBoundary::Validation),
        ValueKind::StringPrefix { .. } => Some(DiagnosticBoundary::Coercion),
        ValueKind::OptionReference(_) => Some(DiagnosticBoundary::FinalOption),
        _ => None,
    }
}

/// A callback parameter or a native argument-set scope identified by semantic IR.
#[derive(Clone)]
struct ParameterScope<'a> {
    /// Matches references to this function, independently of argument names.
    binding: u64,
    /// Native named bindings; ordinary callbacks have only a generated parameter.
    arguments: Option<&'a [String]>,
    /// Shared by nested lowering, so an outer record is captured before shadowing.
    record_used: Rc<Cell<bool>>,
}

/// Lower one default or body, materializing resolved arguments only when needed.
fn lower_argument_region(
    node: &Node,
    binding: u64,
    arguments: &[String],
    scope: &[ParameterScope<'_>],
) -> NixExpr {
    let record_used = Rc::new(Cell::new(false));
    let mut nested = scope.to_vec();
    nested.push(ParameterScope {
        binding,
        arguments: Some(arguments),
        record_used: record_used.clone(),
    });
    let value = lower_scoped(node, &nested);

    if !record_used.get() {
        return value;
    }

    // Whole-record use and shadowed outer names need resolved bindings, not an
    // @-pattern's raw caller record (which would omit defaulted arguments).
    let record = NixExpr::plain(NixKind::AttrSet(
        arguments
            .iter()
            .map(|argument| (vec![argument.clone()], variable(argument)))
            .collect(),
    ));
    NixExpr::plain(NixKind::Let(
        format!("__rusnix_arg_{}", scope.len()),
        Box::new(record),
        Box::new(value),
    ))
}

/// Identify a selection chain rooted in a native argument-set scope.
fn argument_scope(node: &Node, scope: &[ParameterScope<'_>]) -> Option<usize> {
    match &node.kind {
        ValueKind::Parameter(binding) => scope
            .iter()
            .position(|entry| entry.binding == *binding && entry.arguments.is_some()),
        ValueKind::Select(value, _) => argument_scope(value, scope),
        _ => None,
    }
}

fn lower_scoped(node: &Node, scope: &[ParameterScope<'_>]) -> NixExpr {
    let lower_value = |node| lower_scoped(node, scope);
    let kind = match &node.kind {
        ValueKind::Function { binding, body } => {
            let name = format!("__rusnix_arg_{}", scope.len());
            let mut scope = scope.to_vec();
            scope.push(ParameterScope {
                binding: *binding,
                arguments: None,
                record_used: Rc::new(Cell::new(false)),
            });
            NixKind::Lambda(name, Box::new(lower_scoped(body, &scope)))
        }
        ValueKind::FunctionAttrs {
            binding,
            arguments,
            defaults,
            body,
        } => NixKind::ArgumentFunction(
            arguments
                .iter()
                .map(|argument| {
                    (
                        argument.clone(),
                        defaults
                            .iter()
                            .find(|(n, _)| n == argument)
                            .map(|(_, value)| {
                                lower_argument_region(value, *binding, arguments, scope)
                            }),
                    )
                })
                .collect(),
            Box::new(lower_argument_region(body, *binding, arguments, scope)),
        ),
        ValueKind::Parameter(binding) => {
            let index = scope
                .iter()
                .position(|entry| entry.binding == *binding)
                .expect("validated callback scope");
            if scope[index].arguments.is_some() {
                scope[index].record_used.set(true);
            }
            NixKind::Variable(format!("__rusnix_arg_{index}"))
        }
        ValueKind::If(condition, yes, no) => NixKind::If(
            Box::new(lower_value(condition)),
            Box::new(lower_value(yes)),
            Box::new(lower_value(no)),
        ),
        ValueKind::AttrMerge(left, right) => NixKind::Binary(
            BinaryOp::AttrMerge,
            Box::new(lower_value(left)),
            Box::new(lower_value(right)),
        ),
        ValueKind::Assert(condition, value) => NixKind::Assert(
            Box::new(lower_value(condition)),
            Box::new(lower_value(value)),
        ),
        ValueKind::Equal(left, right) => NixKind::Binary(
            crate::ast::BinaryOp::Equal,
            Box::new(lower_value(left)),
            Box::new(lower_value(right)),
        ),
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
        ValueKind::Apply(function, argument) => {
            let mut function = lower_value(function);
            // A curried .apply(...) records the same caller for every application.
            // The outer call covers evaluation of its partial applications too.
            if matches!(function.kind, NixKind::Apply(..))
                && function
                    .origin
                    .as_ref()
                    .is_some_and(|o| o.id == node.origin.id)
            {
                function.error_context = false;
            }
            NixKind::Apply(Box::new(function), Box::new(lower_value(argument)))
        }
        ValueKind::Select(value, path) => {
            let kind = if let Some(index) = argument_scope(value, scope) {
                let first = &path.parts()[0];
                let known = scope[index].arguments.unwrap().contains(first);
                let shadowed = scope[index + 1..]
                    .iter()
                    .any(|entry| entry.arguments.is_some_and(|names| names.contains(first)));

                // Only a direct parameter selection becomes a bare lexical name.
                // Keep every enclosing selection's provenance and source span.
                if matches!(value.kind, ValueKind::Parameter(_)) && known && !shadowed {
                    let root =
                        NixExpr::attributed(NixKind::Variable(first.clone()), value.origin.clone());
                    NixKind::ArgumentSelect(Box::new(root), path.parts()[1..].to_vec())
                } else {
                    NixKind::ArgumentSelect(Box::new(lower_value(value)), path.parts().to_vec())
                }
            } else {
                interop::select(lower_value(value), path).kind
            };
            return NixExpr::attributed(kind, node.origin.clone());
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

    if runtime_boundary(&node.kind).is_some() {
        NixExpr::contextual(kind, node.origin.clone())
    } else {
        NixExpr::attributed(kind, node.origin.clone())
    }
}
