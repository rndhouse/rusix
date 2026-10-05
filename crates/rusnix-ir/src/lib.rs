//! Generic configuration contributions built from ordinary user-defined Rust types.
//!
//! Use [`config`] for local structural authoring, or derive [`IntoConfig`] and
//! [`IntoRusnixValue`] for reusable types. Compose independent contributions with
//! [`nixos::NixosModule`]; use [`interop`] for existing opaque Nix objects.
//! Values are lowered into semantic IR without evaluating Nix in Rust.
#![warn(missing_docs)]

use serde::{Deserialize, Serialize};
use std::{marker::PhantomData, panic::Location};

pub mod interop;

pub mod nixos;

mod value;

/// Local inline authoring: structs and unit enums get conversion derives, with
/// `#[rusnix(root)]` selecting rooted contributions. External types keep their
/// own traits; no source files or imported type definitions are inspected.
/// Multiple rooted structs are allowed. Fields default to lowerCamelCase;
/// container `rename_all = "PascalCase"` changes the mechanical convention.
/// Field `rename` handles exceptions, `skip` omits a field and `flatten` inserts
/// a nested record at its parent's level. Symbolic and opaque values stay deferred.
/// `Option<T>` normally lowers `None` to Nix null. Explicit `#[rusnix(omit_none)]`
/// on a field or named struct omits absent definitions; a struct setting applies
/// only to its direct Option fields, not nested structs. There is no per-field
/// opt-out from a struct setting; use field annotations for mixed behavior.
///
/// ```
/// use rusnix_ir::{self as rusnix, nixos::NixosModule};
///
/// #[rusnix::config]
/// mod configuration {
///     #[rusnix(root)]
///     pub struct Machine { services: Services }
///
///     struct Services { example: Example }
///
///     struct Example { enable: bool, listen_port: u16 }
///
///     pub fn model() -> Machine {
///         Machine { services: Services {
///             example: Example { enable: true, listen_port: 8080 },
///         } }
///     }
/// }
/// let module = NixosModule::empty().add(configuration::model());
/// ```
///
/// Unit enums lower to lowerCamelCase strings (Server → "server", ReadOnly →
/// "readOnly"). Data-carrying enums still require explicit conversion semantics:
///
/// ```compile_fail,E0277
/// use rusnix_ir as rusnix;
///
/// #[rusnix::config]
/// mod configuration {
///     enum Mode { Server, Client { endpoint: String } }
///
///     #[rusnix(root)]
///     struct Machine { mode: Mode }
/// }
/// ```
pub use rusnix_derive::config;

/// Finite final-option dependencies. Navigation builds paths; tracked leaf calls
/// create OptionRef expressions. NixOS still owns existence and actual types.
/// Only a subtree marked `#[rusnix(value)]` exposes `as_value`; roots never do.
/// Requires an inline module with exactly one `#[rusnix(root)]` struct.
/// `bool`, `String` and `i64` leaves return typed [`Expr`] values; `NixValue`,
/// `Option`, `Vec`, `BTreeMap` and `HashMap` leaves return opaque symbolic values,
/// not concrete Rust collections. Local structs provide nested navigation.
/// Naming follows the same `rename` and `rename_all` rules as [`config`].
///
/// ```
/// #![deny(missing_docs)]
/// //! A finite dependency declaration.
/// use rusnix_ir::{self as rusnix, Expr};
///
/// #[rusnix::options]
/// /// Symbolic dependencies used by this component.
/// pub mod options {
///     #[rusnix(root)]
///     struct Root { services: Services }
///
///     struct Services { example: Example }
///
///     struct Example { enable: bool, settings: Settings }
///
///     #[rusnix(value)]
///     struct Settings { port: i64 }
/// }
/// let service = options::root().services.example;
/// let port: Expr<i64> = service.settings.port();
/// let all_settings = service.settings.as_value();
/// ```
pub use rusnix_derive::options;

/// Typed symbolic navigation over an existing deferred Nix function argument record.
/// Requires an inline module with one `#[rusnix(root)]` named struct. Bind with
/// the generated `from_value`; navigation never reads or evaluates values in Rust.
/// `bool`, `String` and `i64` leaves return [`Expr`] with the declared expected type.
/// `NixValue`, `Option`, `Vec`, `BTreeMap` and `HashMap` leaves stay opaque, including
/// deferred collections. Nix remains authoritative for existence and actual types.
/// Naming and explicit subtree `#[rusnix(value)]` / `as_value` follow [`options`].
/// Roots have no `as_value` or dynamic traversal; retain the supplied raw
/// [`interop::NixValue`] for advanced selections. External aliases/view types are
/// not inspected; use explicit lower-level selections outside the declaration.
/// Leaf calls capture caller provenance. Views can be cloned without evaluation.
///
/// ```
/// #![deny(missing_docs)]
/// //! A package implementation's explicit dependencies.
/// use rusnix_ir::{self as rusnix, interop::NixValue};
///
/// #[rusnix::args]
/// /// Finite package arguments used by this implementation.
/// pub mod arguments {
///     #[rusnix(root)]
///     struct Inputs { feature: bool, platform: Platform }
///
///     #[rusnix(value)]
///     struct Platform { system: String }
/// }
/// let factory = NixValue::function_attrs(["feature", "platform"], |value| {
///     let args = arguments::from_value(value);
///     let body = NixValue::if_else(args.feature(), args.platform.system(), "disabled");
///     (Vec::<(&str, NixValue)>::new(), body)
/// });
/// ```
pub use rusnix_derive::args;

#[doc(hidden)]
pub use rusnix_derive::symbolic_text as __symbolic_text;

pub use rusnix_derive::{IntoConfig, IntoRusnixValue};
pub use value::{IntoRusnixValue, RusnixValue};

/// A captured Rust source location and semantic purpose used to explain failures.
/// IDs are deterministic for the same file, line, column and purpose, not stable
/// across source edits. Several uses of the same expression may share an ID.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Origin {
    /// Identity embedded in generated metadata and matched when translating errors.
    pub id: String,
    /// Rust source path as recorded by the caller; it may be relative to the build.
    pub file: String,
    /// One-based source line at which the operation was captured.
    pub line: u32,
    /// One-based source column recorded by Rust's caller tracking.
    pub column: u32,
    /// Operation or configuration path that gives this location semantic meaning.
    pub purpose: String,
}

impl Origin {
    /// Capture the calling Rust operation; tracked helpers forward their caller.
    #[track_caller]
    pub fn caller(purpose: impl Into<String>) -> Self {
        let location = Location::caller();
        Self::new(location.file(), location.line(), location.column(), purpose)
    }

    /// Construct provenance from an explicit location, deriving its deterministic ID.
    pub fn new(file: &str, line: u32, column: u32, purpose: impl Into<String>) -> Self {
        let purpose = purpose.into();
        // Specified FNV-1a, rather than Rust's implementation-dependent DefaultHasher.
        let key = format!("{file}\0{line}\0{column}\0{purpose}");
        let hash = key.bytes().fold(0xcbf29ce484222325_u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
        });
        Self {
            id: format!("rn-{hash:016x}"),
            file: file.into(),
            line,
            column,
            purpose,
        }
    }
}

/// A semantic expression with provenance, exposed for backend implementation and inspection.
/// Normal authoring uses [`Expr`], [`IntoRusnixValue`] or [`interop::NixValue`].
#[derive(Clone, Debug)]
pub struct Node {
    /// Rust operation that introduced this expression.
    pub origin: Origin,
    /// Deferred operation or literal; evaluating it is the backend's responsibility.
    pub kind: ValueKind,
}

/// Semantic operations understood by Rusnix's lowering and generic validation.
/// This is backend-facing IR, not a Nix syntax API or a concrete evaluated value.
#[derive(Clone, Debug)]
pub enum ValueKind {
    /// A concrete Rust boolean embedded as a literal.
    Bool(bool),
    /// A signed integer literal using Nix's 64-bit integer range.
    Int(i64),
    /// A floating literal; validation accepts finite normal values and zero.
    Float(f64),
    /// Nix's explicit null value.
    Null,
    /// Literal text, escaped by the backend rather than interpreted as Nix source.
    String(String),
    /// Ordered deferred elements; constructing the list does not demand its children.
    List(Vec<Node>),
    /// A structural record whose fields can become configuration paths.
    AttrSet(Vec<(String, Node)>),
    /// An interop record stays one value; structural authoring must not flatten
    /// its literal keys into NixOS option paths.
    OpaqueRecord(Vec<(String, Node)>),
    /// An opaque ecosystem lookup; existence and internal type are checked by Nix.
    Reference(interop::Reference),
    /// One deferred application of a function to its argument.
    Apply(Box<Node>, Box<Node>),
    /// A deferred lookup through literal attribute segments.
    Select(Box<Node>, interop::AttrPath),
    /// Scoped callbacks at the opaque Nix boundary, not Rust-side evaluation.
    Function {
        /// Lexical identity shared with parameter references in this callback.
        binding: u64,
        /// Deferred callback result, possibly referring to the symbolic parameter.
        body: Box<Node>,
    },
    /// A finite Nix argument-set callback, retaining native defaults and callPackage introspection.
    FunctionAttrs {
        /// Lexical identity of the record of resolved argument values.
        binding: u64,
        /// Accepted Nix argument names; names without defaults are required.
        arguments: Vec<String>,
        /// Deferred defaults, which can refer to other resolved arguments.
        defaults: Vec<(String, Node)>,
        /// Deferred result, potentially a derivation from an existing builder.
        body: Box<Node>,
    },
    /// A symbolic callback parameter; validation rejects uses outside its scope.
    Parameter(u64),
    /// A deferred condition, then branch and else branch; only one branch is demanded.
    If(Box<Node>, Box<Node>, Box<Node>),
    /// Equality checked by the backend using its native value semantics.
    Equal(Box<Node>, Box<Node>),
    /// A NixOS-scoped dependency, never a concrete Rust value.
    OptionReference(interop::AttrPath),
    /// Deferred text coercion that retains Nix string dependency context.
    ToText(Box<Node>),
    /// Concrete text followed by a deferred string value.
    StringPrefix {
        /// Literal prefix, escaped as data during code generation.
        prefix: String,
        /// String expression whose dependency context survives concatenation.
        value: Box<Node>,
    },
    /// Signed integer division, including evaluator-side division-by-zero failures.
    Divide(Box<Node>, Box<Node>),
    /// An inclusive range constraint checked when the expression is evaluated.
    InRange {
        /// Deferred integer to check.
        value: Box<Node>,
        /// Inclusive lower bound.
        min: i64,
        /// Inclusive upper bound.
        max: i64,
        /// Failure reason presented if the value falls outside the bounds.
        message: String,
    },
}

/// A typed deferred expression whose value is resolved by Nix, never read into Rust.
/// `T` restricts available Rust operations; symbolic option references still rely
/// on NixOS to validate their actual backend types. Child expression origins are retained.
/// Boolean expressions support `!` and lazy [`Expr::and`], without reading a Rust value.
///
/// Integer and boolean expressions cannot be interchanged:
#[doc = concat!("```compile_fail,E0308\n", include_str!("../../../tests/fixtures/rust-type-failure.rs"), "\n```")]
#[derive(Clone, Debug)]
pub struct Expr<T> {
    node: Node,
    ty: PhantomData<T>,
}

impl<T> Expr<T> {
    fn new(kind: ValueKind, origin: Origin) -> Self {
        Self {
            node: Node { origin, kind },
            ty: PhantomData,
        }
    }
}

impl Expr<i64> {
    /// Format a deferred integer without resolving it in Rust.
    #[track_caller]
    pub fn to_text(self) -> Expr<String> {
        Expr::new(
            ValueKind::ToText(Box::new(self.node)),
            Origin::caller("integer to text"),
        )
    }

    /// Embed a concrete integer while recording its Rust source origin.
    #[track_caller]
    pub fn int(value: i64) -> Self {
        Self::new(ValueKind::Int(value), Origin::caller("integer literal"))
    }

    /// Defer signed integer division to Nix, retaining both operand origins.
    /// Division by zero is reported during evaluation at this operation's caller.
    #[track_caller]
    pub fn divide(self, denominator: Self) -> Self {
        Self::new(
            ValueKind::Divide(Box::new(self.node), Box::new(denominator.node)),
            Origin::caller("integer division"),
        )
    }

    /// Require an inclusive range when Nix evaluates this integer.
    /// Failure uses `message` and this operation's origin; no Rust-time check is performed.
    #[track_caller]
    pub fn in_range(self, min: i64, max: i64, message: impl Into<String>) -> Self {
        Self::new(
            ValueKind::InRange {
                value: Box::new(self.node),
                min,
                max,
                message: message.into(),
            },
            Origin::caller("integer range constraint"),
        )
    }
}

impl Expr<String> {
    /// Prepend concrete text to a deferred string. Nix preserves string context.
    #[track_caller]
    pub fn with_prefix(self, prefix: impl Into<String>) -> Self {
        Self::new(
            ValueKind::StringPrefix {
                prefix: prefix.into(),
                value: Box::new(self.node),
            },
            Origin::caller("symbolic string prefix"),
        )
    }
}

impl Expr<bool> {
    /// Embed a concrete boolean while recording its Rust source origin.
    #[track_caller]
    pub fn boolean(value: bool) -> Self {
        Self::new(ValueKind::Bool(value), Origin::caller("boolean literal"))
    }

    /// Deferred boolean conjunction, independent of any supplied Nix library.
    /// Nix checks both operands as booleans and leaves the right operand unforced
    /// when the left is false. Rust only constructs the dependency expression.
    #[track_caller]
    pub fn and(self, other: Self) -> Self {
        let other = interop::NixValue::if_else(other, true, false);
        interop::NixValue::if_else(self, other, false).into_expr()
    }
}

/// Keeps boolean negation typed without evaluating the expression in Rust.
impl std::ops::Not for Expr<bool> {
    type Output = Self;

    #[track_caller]
    fn not(self) -> Self {
        (!interop::NixValue::from(self)).into_expr()
    }
}

// A sealed conversion keeps arbitrary syntax and incorrectly typed Expr<T> out.
mod sealed {
    pub trait Sealed {}
}

/// Sealed conversion of supported literals, expressions and opaque boundary values.
/// Used by the generic [`Config::set`] escape hatch and interop calls. For your
/// own domain types, implement [`IntoRusnixValue`] rather than this trait.
pub trait ConfigValue: sealed::Sealed {
    /// Lower into semantic IR, using `origin` for concrete leaves and retaining
    /// any provenance already captured by symbolic expressions or references.
    fn into_node(self, origin: Origin) -> Node;
}

macro_rules! primitive {
    ($ty:ty, $variant:ident) => {
        impl sealed::Sealed for $ty {}

        impl ConfigValue for $ty {
            fn into_node(self, origin: Origin) -> Node {
                Node {
                    origin,
                    kind: ValueKind::$variant(self.into()),
                }
            }
        }
    };
}

primitive!(bool, Bool);

primitive!(i32, Int);

primitive!(i64, Int);

primitive!(u16, Int);

primitive!(f64, Float);

primitive!(String, String);

primitive!(&str, String);

impl<T: ConfigValue> sealed::Sealed for Vec<T> {}

impl<T: ConfigValue> ConfigValue for Vec<T> {
    fn into_node(self, origin: Origin) -> Node {
        let children = self
            .into_iter()
            .enumerate()
            .map(|(i, value)| {
                let child = Origin::new(
                    &origin.file,
                    origin.line,
                    origin.column,
                    format!("{}[{i}]", origin.purpose),
                );
                value.into_node(child)
            })
            .collect();
        Node {
            origin,
            kind: ValueKind::List(children),
        }
    }
}

macro_rules! expression_value {
    ($ty:ty) => {
        impl sealed::Sealed for Expr<$ty> {}

        impl ConfigValue for Expr<$ty> {
            fn into_node(self, _: Origin) -> Node {
                self.node
            }
        }
    };
}

expression_value!(i64);

expression_value!(bool);

expression_value!(String);

/// One binding within a [`Config`], exposed for backend lowering and inspection.
#[derive(Clone, Debug)]
pub struct Assignment {
    /// Rust operation that introduced this definition, distinct from child expression origins.
    pub origin: Origin,
    /// Human-readable path; use [`Self::path_segments`] for unambiguous code generation.
    pub path: String,
    /// Deferred right-hand side of the definition.
    pub value: Node,
    segments: Vec<String>,
}

impl Assignment {
    /// Attribute segments are data; a renamed field may contain a literal dot.
    pub fn path_segments(&self) -> &[String] {
        &self.segments
    }
}

/// A contribution of bindings. Independent contributions are composed by the
/// backend (for NixOS, with [`nixos::NixosModule::add`]), not flattened into this value.
/// Prefer structural authoring or [`IntoConfig`] adapters. [`Self::set`] provides
/// generic option access; duplicate or ancestor/descendant bindings within one
/// contribution are validation errors, unlike separate NixOS contributions.
#[derive(Clone, Debug)]
pub struct Config {
    /// Source location at which this contribution was created.
    pub origin: Origin,
    /// Ordered bindings belonging to this contribution, before backend merging.
    pub assignments: Vec<Assignment>,
    error: Option<ValidationError>,
}

/// Lower a complete component whose placement in configuration is defined.
/// Reusable domain values should remain values inside such components.
///
/// The caller location propagates to implementations and tracked lowering
/// helpers, so bindings created during conversion point to the authoring call.
/// Already-captured expression origins are retained. Untracked intermediate
/// helpers stop that propagation; mark lowering helpers `#[track_caller]` too.
pub trait IntoConfig {
    /// Consume a complete component into rooted configuration bindings.
    /// Conversion describes definitions; it does not evaluate or merge NixOS options.
    #[track_caller]
    fn into_config(self) -> Config;
}

/// Allow an explicit generic escape-hatch contribution alongside typed models.
impl IntoConfig for Config {
    fn into_config(self) -> Config {
        self
    }
}

impl Config {
    /// Start an empty contribution, capturing its caller for provenance.
    #[track_caller]
    pub fn new() -> Self {
        Self {
            origin: Origin::caller("configuration"),
            assignments: Vec::new(),
            error: None,
        }
    }

    /// Add a dotted option path through the generic escape hatch or an adapter.
    /// Values stay deferred; schema/type checks belong to NixOS. Paths and
    /// duplicate definitions are checked by [`Self::validate`], not by this call.
    /// Use structural lowering when a literal attribute segment contains a dot.
    #[track_caller]
    pub fn set(mut self, path: impl Into<String>, value: impl ConfigValue) -> Self {
        let path = path.into();
        let origin = Origin::caller(format!("set {path}"));
        let value_origin = Origin::caller(format!("value of {path}"));
        self.assignments.push(Assignment {
            origin,
            segments: path.split('.').map(str::to_owned).collect(),
            path,
            value: value.into_node(value_origin),
        });
        self
    }

    /// Check generic IR invariants without evaluating expressions or consulting NixOS.
    /// Rejects ambiguous paths/records, unsupported strings/floats and escaped
    /// callback parameters. Backend existence, option types and assertions remain unchecked.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if let Some(error) = &self.error {
            return Err(error.clone());
        }

        let mut seen: Vec<&[String]> = Vec::new();

        for assignment in &self.assignments {
            let path = &assignment.path;
            reject_nul(path, &assignment.origin)?;
            let parts = assignment.path_segments();
            if parts.is_empty()
                || parts
                    .iter()
                    .any(|part| part.is_empty() || part.contains('\0'))
            {
                return Err(ValidationError {
                    origin: assignment.origin.clone(),
                    message: "option paths must have nonempty dot-separated segments".into(),
                });
            }

            if seen
                .iter()
                .any(|old| parts.starts_with(old) || old.starts_with(parts))
            {
                return Err(ValidationError {
                    origin: assignment.origin.clone(),
                    message: format!("duplicate or conflicting option path: {path}"),
                });
            }

            seen.push(parts);
            validate_value(&assignment.value)?;
        }

        Ok(())
    }
}

fn reject_nul(text: &str, origin: &Origin) -> Result<(), ValidationError> {
    if text.contains('\0') {
        return Err(ValidationError {
            origin: origin.clone(),
            message: "NUL bytes are not supported in configuration strings".into(),
        });
    }

    Ok(())
}

fn validate_value(node: &Node) -> Result<(), ValidationError> {
    validate_scoped(node, &[])
}

fn validate_scoped(node: &Node, scope: &[u64]) -> Result<(), ValidationError> {
    let validate_value = |node| validate_scoped(node, scope);

    match &node.kind {
        ValueKind::String(text) => reject_nul(text, &node.origin)?,
        ValueKind::List(items) => {
            for item in items {
                validate_value(item)?;
            }
        }
        ValueKind::AttrSet(fields) | ValueKind::OpaqueRecord(fields) => {
            let mut seen = std::collections::BTreeSet::new();

            for (name, value) in fields {
                let opaque = matches!(node.kind, ValueKind::OpaqueRecord(_));
                let origin = if opaque { &node.origin } else { &value.origin };
                reject_nul(name, origin)?;

                if (!opaque && name.is_empty()) || !seen.insert(name) {
                    return Err(ValidationError {
                        origin: origin.clone(),
                        message: format!("invalid or duplicate record field: {name}"),
                    });
                }

                validate_value(value)?;
            }
        }
        ValueKind::Divide(left, right)
        | ValueKind::Apply(left, right)
        | ValueKind::Equal(left, right) => {
            validate_value(left)?;
            validate_value(right)?;
        }
        ValueKind::InRange { value, message, .. } => {
            reject_nul(message, &node.origin)?;
            validate_value(value)?;
        }
        ValueKind::Float(value) if !value.is_finite() || value.is_subnormal() => {
            return Err(ValidationError {
                origin: node.origin.clone(),
                message: "Nix float literals require finite normal values or zero (no NaN, infinity, or subnormals)".into(),
            });
        }
        ValueKind::Bool(_) | ValueKind::Int(_) | ValueKind::Float(_) | ValueKind::Null => {}
        ValueKind::Reference(reference) => reference.validate()?,
        ValueKind::Select(value, path) => {
            validate_value(value)?;
            path.validate(&node.origin)?;
        }
        ValueKind::Function { binding, body } => {
            let mut scope = scope.to_vec();
            scope.push(*binding);
            validate_scoped(body, &scope)?;
        }
        ValueKind::FunctionAttrs {
            binding,
            arguments,
            defaults,
            body,
        } => {
            let mut names = std::collections::BTreeSet::new();
            for name in arguments {
                let mut chars = name.chars();
                let first = chars
                    .next()
                    .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
                let rest = chars.all(|c| c.is_ascii_alphanumeric() || "_-'".contains(c));
                if !first
                    || !rest
                    || name.starts_with("__rusnix_")
                    || [
                        "if", "then", "else", "assert", "with", "let", "in", "rec", "inherit",
                    ]
                    .contains(&name.as_str())
                    || !names.insert(name)
                {
                    return Err(ValidationError {
                        origin: node.origin.clone(),
                        message: format!("invalid or duplicate Nix function argument: {name}"),
                    });
                }
            }
            let mut scope = scope.to_vec();
            scope.push(*binding);
            let mut seen = std::collections::BTreeSet::new();
            for (name, value) in defaults {
                if !names.contains(name) || !seen.insert(name) {
                    return Err(ValidationError {
                        origin: node.origin.clone(),
                        message: format!("unknown or duplicate Nix function default: {name}"),
                    });
                }
                validate_scoped(value, &scope)?;
            }
            validate_scoped(body, &scope)?;
        }
        ValueKind::Parameter(binding) => {
            if !scope.contains(binding) {
                return Err(ValidationError {
                    origin: node.origin.clone(),
                    message: "opaque callback parameter escaped its function scope".into(),
                });
            }
        }
        ValueKind::If(condition, yes, no) => {
            validate_value(condition)?;
            validate_value(yes)?;
            validate_value(no)?;
        }
        ValueKind::OptionReference(path) => path.validate(&node.origin)?,
        ValueKind::ToText(value) => validate_value(value)?,
        ValueKind::StringPrefix { prefix, value } => {
            reject_nul(prefix, &node.origin)?;
            validate_value(value)?;
        }
    }

    Ok(())
}

impl Default for Config {
    #[track_caller]
    fn default() -> Self {
        Self::new()
    }
}

/// A generic configuration invariant failure detected before backend evaluation.
#[derive(Clone, Debug)]
pub struct ValidationError {
    /// Rust operation associated with the rejected value or binding.
    pub origin: Origin,
    /// Actionable reason that describes the violated invariant.
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caller_and_ids_are_deterministic() {
        fn make() -> Config {
            Config::new().set("a", true)
        }

        let first = make();
        let second = make();

        assert_eq!(first.assignments[0].origin, second.assignments[0].origin);
        assert_eq!(first.assignments[0].origin.file, file!());
        assert_ne!(first.origin.id, first.assignments[0].origin.id);
    }

    #[test]
    fn paths_are_validated_in_both_prefix_directions() {
        for paths in [["a", "a"], ["a", "a.b"], ["a.b", "a"], ["a..b", "x"]] {
            assert!(
                Config::new()
                    .set(paths[0], true)
                    .set(paths[1], 1)
                    .validate()
                    .is_err()
            );
        }
        assert!(
            Config::new()
                .set("a.b", true)
                .set("a.c", vec![22])
                .validate()
                .is_ok()
        );
    }

    #[test]
    fn unsupported_nul_is_rejected_at_its_origin() {
        let config = Config::new().set("strings", vec!["valid", "bad\0string"]);
        let error = config.validate().unwrap_err();

        let ValueKind::List(items) = &config.assignments[0].value.kind else {
            panic!()
        };

        assert_eq!(error.origin, items[1].origin);
        assert!(Config::new().set("bad\0path", true).validate().is_err());
        assert!(
            Config::new()
                .set("number", Expr::int(1).in_range(0, 2, "bad\0message"))
                .validate()
                .is_err()
        );
    }
}
