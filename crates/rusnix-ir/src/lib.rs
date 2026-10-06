//! Describe Nix configuration with ordinary Rust structs, enums and functions.
//!
//! Nix is a language used to describe configuration and software builds. Rusnix
//! turns Rust values into Nix expressions; Nix evaluates those expressions later.
//! Rust code can also describe references and calls without reading their results.
//!
//! Start with [`config`] for a local configuration tree. Nested structs become
//! nested named fields in Nix. Use [`IntoConfig`] and [`IntoRusnixValue`] derives
//! for reusable types, [`interop`] to reuse existing packages and functions, and
//! [`nixos::NixosModule`] to combine configuration for NixOS. NixOS combines
//! modules and checks configurable fields, called *options*, during evaluation.
#![warn(missing_docs)]

use serde::{Deserialize, Serialize};
use std::{marker::PhantomData, panic::Location};

pub mod interop;

pub mod nixos;

pub mod package;

mod value;

/// Turn an inline Rust module’s local types into Nix configuration values.
/// Nested structs become nested Nix attribute sets: collections of named fields.
/// Mark a root struct with `#[rusnix(root)]` so it can be added to a
/// [`nixos::NixosModule`]. NixOS will combine that contribution with other modules.
/// The macro describes values; it does not evaluate Nix or declare option types.
///
/// ```
/// use rusnix_ir::{self as rusnix, nixos::NixosModule};
///
/// #[rusnix::config]
/// mod configuration {
///     #[rusnix(root)]
///     pub struct Machine { pub services: Services }
///
///     pub struct Services { pub example: Example }
///
///     pub struct Example { pub enable: bool }
/// }
///
/// use configuration::{Machine, Services, Example};
/// let module = NixosModule::empty().add(Machine {
///     services: Services { example: Example { enable: true } },
/// });
/// // Defines services.example.enable = true in the generated NixOS module.
/// ```
///
/// # Mapping rules
///
/// Local structs and unit enums receive the existing conversion derives. Types
/// imported from elsewhere must already implement the conversion traits. Multiple
/// roots are allowed. Only inline modules are supported; no external files or
/// type definitions are inspected.
///
/// Fields use lowerCamelCase by default. `rename_all = "PascalCase"` changes the
/// convention for a struct; `rename` handles a field exception. `skip` omits a
/// field, and `flatten` inserts a nested record’s fields at its parent’s level.
/// Symbolic expressions and package handles keep their Nix behavior.
///
/// `Option<T>` normally maps `None` to Nix `null`. Explicit `#[rusnix(omit_none)]`
/// omits a field when it is `None`. On a named struct, it applies only to direct
/// `Option` fields and does not propagate into nested types. Use field annotations
/// when some absent fields should be omitted and others should be null.
///
/// Unit enums become strings: `Server` becomes `"server"`, and `ReadOnly` becomes
/// `"readOnly"`. Enums carrying data need an explicit conversion that chooses
/// what their data means in Nix:
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

/// Generate typed accessors for final NixOS configuration values.
/// NixOS combines configuration supplied by many modules. These accessors refer
/// to the resulting values, so other modules’ overrides still affect dependent
/// expressions. Rust constructs references; it never reads the final values.
/// This declares dependencies, not NixOS options or their actual types.
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
///     struct Example { port: i64 }
/// }
/// let port: Expr<i64> = options::root().services.example.port();
/// // Equivalent to OptionRef::<i64>::new("services.example.port").into_expr().
/// ```
///
/// Use exactly one root in an inline module. Local structs provide navigation;
/// leaf calls capture their Rust call location. `bool`, `String` and `i64`
/// leaves return [`Expr`] with the declared expectation. `NixValue`, `Option`,
/// `Vec`, `BTreeMap` and `HashMap` leaves return Nix expressions, not Rust
/// collections. NixOS checks option existence and actual value types.
///
/// Naming follows [`config`]. Mark a nested struct `#[rusnix(value)]` to expose
/// `as_value()` for its entire Nix subtree. Roots have no whole-value accessor
/// or arbitrary field traversal.
pub use rusnix_derive::options;

/// Generate typed accessors for values in a Nix function’s named arguments.
/// Nixpkgs package functions commonly receive an attribute set of dependencies
/// and feature choices. This macro describes the fields an adapter needs.
/// Accessors construct Nix expressions; they do not read argument values in Rust.
///
/// ```
/// use rusnix_ir::{self as rusnix, interop::NixValue};
///
/// #[rusnix::args]
/// mod arguments {
///     #[rusnix(root)]
///     struct Inputs { enabled: bool, platform: Platform }
///
///     struct Platform { system: String }
/// }
/// let value = NixValue::record([
///     ("enabled", true.into()),
///     ("platform", NixValue::record([("system", "x86_64-linux".into())])),
/// ]);
/// let args = arguments::from_value(value);
/// let enabled = args.enabled();
/// let system = args.platform.system();
/// // Each accessor describes a field lookup; no Nix evaluation happens here.
/// ```
///
/// Bind the view to an existing [`interop::NixValue`] using generated `from_value`.
/// This also works with the placeholders supplied by
/// [`interop::NixValue::function_attrs`]. Nix remains responsible for caller
/// arguments, defaults and actual types.
///
/// Use one root in an inline module. Leaf types, naming and optional subtree
/// `as_value()` access follow [`options`]. Views can be cloned without evaluating
/// them. Accessor calls record their own Rust locations. External aliases and
/// view types are not inspected; retain the raw NixValue for dynamic selections.
/// Roots do not expose whole-value access or arbitrary field traversal.
pub use rusnix_derive::args;

#[doc(hidden)]
pub use rusnix_derive::symbolic_text as __symbolic_text;

pub use rusnix_derive::{IntoConfig, IntoRusnixValue};
pub use value::{IntoRusnixValue, RusnixValue};

/// Where a Rust operation was written, and what it was doing.
/// Rusnix includes this information in generated output so Nix evaluation errors
/// can be linked back to Rust source. This source attribution is also called
/// *provenance*.
///
/// IDs are deterministic for the same file, line, column and purpose, but can
/// change after edits. Reusing an expression can produce several uses of one ID.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Origin {
    /// Deterministic `rn-` followed by 16 lowercase hexadecimal digits, embedded
    /// unchanged in generated comments, error contexts and source-map metadata.
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

    /// Create source attribution from an explicit Rust location and operation name.
    /// The ID is derived from these inputs; it is not an identifier assigned by Nix.
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

/// A description of one value or operation, with its Rust source location.
/// Backend authors use these nodes to generate Nix; the collection of nodes is
/// Rusnix’s *intermediate representation* (IR). It describes computation without
/// performing it. Normal authoring uses [`Expr`], [`IntoRusnixValue`] or
/// [`interop::NixValue`] instead of constructing nodes.
#[derive(Clone, Debug)]
pub struct Node {
    /// Rust operation that introduced this expression.
    pub origin: Origin,
    /// Deferred operation or literal; evaluating it is the backend's responsibility.
    pub kind: ValueKind,
}

impl Node {
    /// Check expression invariants without evaluating Nix or checking its actual types.
    /// Rejects invalid strings, paths, literals and escaped callback parameters.
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_scoped(self, &[])
    }
}

/// The value or operation described by a [`Node`].
/// Variants distinguish literals, containers and computations Nix will perform
/// later. This is an interface for backend implementation and inspection, not
/// a way for Rust authors to obtain evaluated Nix values.
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
    /// Native shallow attribute-set union; right-hand fields replace left-hand fields.
    AttrMerge(Box<Node>, Box<Node>),
    /// Native expression assertion; only a true condition permits demanding the result.
    Assert(Box<Node>, Box<Node>),
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
        /// Lexical identity of the named argument scope, including resolved defaults.
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

/// An expression that Nix will evaluate later, with an expected Rust result type.
///
/// For example, [`Expr<i64>`] describes an integer expression and provides
/// integer operations such as [`Self::divide`]. Calling those methods constructs
/// more expressions; it does not compute or read the result in Rust. Use
/// [`interop::NixValue`] when the value’s category should remain unspecified.
///
/// `T` restricts Rust composition. For references to external Nix values, it is
/// an expectation rather than proof of their actual types; Nix or NixOS checks
/// those during evaluation. Child operations retain their Rust source locations.
/// Boolean expressions support `!` and short-circuit [`Self::and`].
///
/// Integer and boolean expressions cannot be interchanged:
#[doc = concat!("```compile_fail,E0308\n", include_str!("../../../tests/fixtures/rust-type-failure.rs"), "\n```")]
#[derive(Clone, Debug)]
pub struct Expr<T> {
    /// The literal or computation Nix should evaluate, with its Rust source location.
    node: Node,
    /// The expected result category used to restrict Rust operations, not an evaluated value.
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
    /// Describe decimal text conversion of this integer using Nix’s `toString`.
    /// The result is an [`Expr<String>`]; Rust does not read or format the integer.
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

    /// Describe integer division for Nix to compute later.
    /// This uses Nix’s signed integer division. Division by zero is reported when
    /// Nix evaluates the result, with this Rust call as the consuming operation.
    #[track_caller]
    pub fn divide(self, denominator: Self) -> Self {
        Self::new(
            ValueKind::Divide(Box::new(self.node), Box::new(denominator.node)),
            Origin::caller("integer division"),
        )
    }

    /// Require the integer to be between `min` and `max`, inclusive, when Nix
    /// evaluates it. A rejected value fails with `message` at this Rust operation.
    /// The result remains an integer expression; no range check happens in Rust.
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
    /// Place concrete text before this expression’s string when Nix evaluates it.
    /// Nix strings also track dependencies on package outputs; concatenation retains
    /// those dependencies. Rust does not read the string value.
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

    /// Construct boolean AND for Nix to evaluate later.
    /// Nix checks the operands as booleans and evaluates the right operand only if
    /// the left is true. This uses Nix conditionals, independently of nixpkgs `lib`.
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

/// A supported value that can be used directly in [`Config::set`] or a Nix call.
/// Built-in Rust literals, supported [`Expr`] types and Nix references implement
/// this trait. It is sealed, so users cannot add implementations. For your own
/// configuration types, derive or implement [`IntoRusnixValue`] instead.
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

/// A setting with its destination path, value expression and Rust location.
/// This is inspection data within [`Config`]. The backend turns it into a Nix
/// field assignment; its value is evaluated later by Nix.
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

/// A group of settings that one component contributes to configuration.
/// For example, it can define `services.example.enable` and
/// `services.example.port`. Values may be concrete Rust literals or expressions
/// that Nix will evaluate later; constructing Config does not run Nix.
///
/// Prefer [`config`] or the [`IntoConfig`] derive for ordinary Rust authoring.
/// [`Self::set`] is the general escape hatch for explicitly named paths.
///
/// A NixOS module supplies settings to a larger system configuration. Combine
/// independent groups with [`nixos::NixosModule::add`] so NixOS can merge them.
/// Repeated or overlapping paths inside a single Config are validation errors;
/// separate contributions can define the same option under NixOS’s merge rules.
#[derive(Clone, Debug)]
pub struct Config {
    /// Source location at which this contribution was created.
    pub origin: Origin,
    /// Ordered bindings belonging to this contribution, before backend merging.
    pub assignments: Vec<Assignment>,
    error: Option<ValidationError>,
}

/// Convert a complete Rust configuration component into a group of settings.
/// The component defines where its fields belong: a root with a `services`
/// field, for example, defines paths starting with `services`. Reusable nested
/// values use [`IntoRusnixValue`] and let their parent choose placement.
///
/// Use the [`config`] macro for local types or derive this trait for reusable
/// root types. [`nixos::NixosModule::add`] accepts either approach. Conversion
/// runs in Rust now, but generated expressions and NixOS merging happen later.
/// Derived conversion saves structural errors in the returned [`Config`] for
/// [`Config::validate`] or compilation to report. Returning `Config` without a
/// `Result` does not guarantee validity. By contrast,
/// [`IntoRusnixValue::try_into_nix_value`] reports invalid flattening immediately
/// when constructing one Nix value for a function argument.
///
/// For custom implementations, `#[track_caller]` propagates the author’s call
/// location through tracked helpers. Existing expression locations are retained;
/// untracked helper calls stop that location propagation.
pub trait IntoConfig {
    /// Turn this component into settings at their complete configuration paths.
    /// This describes the settings without evaluating expressions or combining them
    /// with other NixOS contributions.
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

    /// Assign a value at a dotted configuration path, such as `services.example.port`.
    /// Use this escape hatch when a typed configuration struct is inconvenient.
    /// The value is described now and evaluated by Nix later. [`Self::validate`]
    /// checks paths and duplicates; NixOS checks option existence and actual types.
    /// Use structural authoring for a literal field name containing a dot.
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

    /// Check that this contribution can be represented unambiguously in generated Nix.
    /// This runs in Rust and rejects conflicting paths, invalid strings or float
    /// literals, and callback parameters used outside their functions. It does not
    /// run Nix or check NixOS option existence, types or assertions.
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
            assignment.value.validate()?;
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
        | ValueKind::AttrMerge(left, right)
        | ValueKind::Assert(left, right)
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

/// A configuration-description error found by Rust before Nix evaluation.
/// It identifies the Rust operation and explains why its description cannot be
/// compiled. NixOS option type errors are checked later by NixOS instead.
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
