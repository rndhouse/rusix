//! Ordinary Rust constructs semantic configuration; no Nix syntax enters this API.
use serde::{Deserialize, Serialize};
use std::{marker::PhantomData, panic::Location};

pub mod interop;

pub mod nixos;

mod value;

/// Local inline authoring: structs and unit enums get conversion derives, with
/// `#[rusnix(root)]` selecting rooted contributions. External types keep their
/// own traits; no source files or imported type definitions are inspected.
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
///
/// ```
/// use rusnix_ir::{self as rusnix, Expr};
///
/// #[rusnix::options]
/// mod options {
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

#[doc(hidden)]
pub use rusnix_derive::symbolic_text as __symbolic_text;

pub use rusnix_derive::{IntoConfig, IntoRusnixValue};
pub use value::{IntoRusnixValue, RusnixValue};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Origin {
    pub id: String,
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub purpose: String,
}

impl Origin {
    #[track_caller]
    pub fn caller(purpose: impl Into<String>) -> Self {
        let location = Location::caller();
        Self::new(location.file(), location.line(), location.column(), purpose)
    }

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

#[derive(Clone, Debug)]
pub struct Node {
    pub origin: Origin,
    pub kind: ValueKind,
}

#[derive(Clone, Debug)]
pub enum ValueKind {
    Bool(bool),
    Int(i64),
    Float(f64),
    Null,
    String(String),
    List(Vec<Node>),
    AttrSet(Vec<(String, Node)>),
    /// An interop record stays one value; structural authoring must not flatten
    /// its literal keys into NixOS option paths.
    OpaqueRecord(Vec<(String, Node)>),
    Reference(interop::Reference),
    Apply(Box<Node>, Box<Node>),
    Select(Box<Node>, interop::AttrPath),
    /// Scoped callbacks at the opaque Nix boundary, not Rust-side evaluation.
    Function {
        binding: u64,
        body: Box<Node>,
    },
    Parameter(u64),
    If(Box<Node>, Box<Node>, Box<Node>),
    Equal(Box<Node>, Box<Node>),
    /// A NixOS-scoped dependency, never a concrete Rust value.
    OptionReference(interop::AttrPath),
    ToText(Box<Node>),
    StringPrefix {
        prefix: String,
        value: Box<Node>,
    },
    Divide(Box<Node>, Box<Node>),
    InRange {
        value: Box<Node>,
        min: i64,
        max: i64,
        message: String,
    },
}

/// A typed deferred expression, not a Nix AST.
///
/// The broken fixture must fail with a Rust type mismatch:
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

    #[track_caller]
    pub fn int(value: i64) -> Self {
        Self::new(ValueKind::Int(value), Origin::caller("integer literal"))
    }

    #[track_caller]
    pub fn divide(self, denominator: Self) -> Self {
        Self::new(
            ValueKind::Divide(Box::new(self.node), Box::new(denominator.node)),
            Origin::caller("integer division"),
        )
    }

    /// Defer a domain constraint to the backend, to exercise evaluator diagnostics.
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
    #[track_caller]
    pub fn boolean(value: bool) -> Self {
        Self::new(ValueKind::Bool(value), Origin::caller("boolean literal"))
    }
}

// A sealed conversion keeps arbitrary syntax and incorrectly typed Expr<T> out.
mod sealed {
    pub trait Sealed {}
}

pub trait ConfigValue: sealed::Sealed {
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

#[derive(Clone, Debug)]
pub struct Assignment {
    pub origin: Origin,
    pub path: String,
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
/// backend (for NixOS, with `NixosModule::add`), not flattened into this value.
#[derive(Clone, Debug)]
pub struct Config {
    pub origin: Origin,
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
    #[track_caller]
    pub fn new() -> Self {
        Self {
            origin: Origin::caller("configuration"),
            assignments: Vec::new(),
            error: None,
        }
    }

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

    /// Reject ambiguous attribute construction before reaching the Nix backend.
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

#[derive(Clone, Debug)]
pub struct ValidationError {
    pub origin: Origin,
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
