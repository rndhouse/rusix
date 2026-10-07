//! Refer to existing Nix packages, functions and configuration from Rust.
//!
//! Nix is a language for describing values and build recipes. nixpkgs is a
//! collection of packages and utility functions written in that language; NixOS
//! uses Nix modules to combine system configuration from multiple sources.
//!
//! [`NixValue`] describes a value or expression that Nix will evaluate later.
//! An attribute set is Nix’s collection of named fields, like a Rust record or
//! map. [`PackageRef`] and other handles distinguish common uses of those values
//! without inspecting their contents in Rust. Nix checks lookups and function
//! arguments; Rusnix records Rust locations to help explain failures.
use crate::{ConfigValue, Node, Origin, ValidationError, ValueKind, sealed};
use std::path::{Component, PathBuf};

mod library;

mod typed;

mod text;

pub use text::ToNixText;

pub use library::NixLibrary;

pub use typed::{
    IntoNixExpression, NixAttrs, NixCallable, NixExpression, NixList, NixOverridable, Overridable,
    Package, Stdenv,
};

/// A sequence of field names to look up in Nix attribute sets.
/// For example, `["services", "example", "port"]` describes
/// `services.example.port`. Names are data, never executable Nix source.
/// Use [`Self::segments`] when a field name itself contains a dot.
#[derive(Clone, Debug)]
pub struct AttrPath(pub(crate) Vec<String>);

impl AttrPath {
    /// Split a convenience dotted path; literal dots require [`Self::segments`].
    /// Empty or NUL-containing segments are rejected during IR validation.
    pub fn dotted(path: &str) -> Self {
        Self(path.split('.').map(str::to_owned).collect())
    }

    /// Construct a path from literal names, preserving punctuation within each segment.
    pub fn segments(parts: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self(parts.into_iter().map(Into::into).collect())
    }

    /// Inspect literal segments without resolving the referenced Nix value.
    pub fn parts(&self) -> &[String] {
        &self.0
    }

    pub(crate) fn validate(&self, origin: &Origin) -> Result<(), ValidationError> {
        if self.0.is_empty() || self.0.iter().any(|p| p.is_empty() || p.contains('\0')) {
            return Err(ValidationError {
                origin: origin.clone(),
                message: "Nix attribute paths need nonempty, NUL-free segments".into(),
            });
        }

        Ok(())
    }
}

/// Where Nix should obtain a referenced package, function or other value.
/// This is inspection data for backend authors. Normal configuration code
/// chooses a source through [`Nixpkgs`] or [`InputRef`] instead.
#[derive(Clone, Debug)]
pub enum Source {
    /// The Nix language's own builtin namespace, independent of nixpkgs or caller libraries.
    Builtins,
    /// The pinned standalone package set, with Nix applying overlays in order.
    Packages {
        /// Deferred overlay functions, ordered as supplied by the author.
        overlays: Vec<Reference>,
    },
    /// The pinned nixpkgs library, independent of package-set overlays.
    Library,
    /// A file or directory in the pinned source tree, without importing or fetching it.
    PinnedPath {
        /// Relative path under nixpkgs; parent traversal is rejected.
        path: String,
    },
    /// The NixOS module's supplied `pkgs`, preserving its configuration and overlays.
    NixosPackages {
        /// Additional overlays applied to the supplied package set in order.
        overlays: Vec<Reference>,
    },
    /// An existing NixOS module file in the pinned nixpkgs tree.
    ModuleFile {
        /// Relative path under `nixos/modules`, without parent traversal.
        path: String,
    },
    /// A local Nix expression supplying external objects; no fetching is implied.
    Input {
        /// Human-readable identity used in provenance and diagnostics.
        name: String,
        /// File imported by Nix; relative paths use Rust's working directory during lowering.
        file: PathBuf,
    },
}

/// A description of where Nix should look up an existing value.
/// It contains the source to load, the field names to select and the Rust
/// location to report on failure. It does not contain the evaluated Nix value.
/// Normal authoring uses category handles such as [`PackageRef`]; this metadata
/// is public for backend implementation and inspection.
#[derive(Clone, Debug)]
pub struct Reference {
    /// Package set, utility library or local file from which Nix obtains the value.
    pub source: Source,
    /// Field names to select from that source, or `None` to reference the source itself.
    pub path: Option<AttrPath>,
    /// Rust lookup operation to report if Nix cannot resolve the object.
    pub origin: Origin,
}

impl Reference {
    /// Check path/input invariants without opening files or validating Nix object types.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if let Some(path) = &self.path {
            path.validate(&self.origin)?;
        }

        match &self.source {
            Source::Packages { overlays } | Source::NixosPackages { overlays } => {
                for overlay in overlays {
                    overlay.validate()?;
                }
            }
            Source::ModuleFile { path } | Source::PinnedPath { path } => {
                if path.is_empty()
                    || path.contains('\0')
                    || PathBuf::from(path)
                        .components()
                        .any(|c| !matches!(c, Component::Normal(_)))
                {
                    return Err(ValidationError {
                        origin: self.origin.clone(),
                        message: if matches!(self.source, Source::PinnedPath { .. }) {
                            "source paths must be relative, nonempty paths without parent traversal"
                        } else {
                            "module paths must be relative, nonempty paths without parent traversal"
                        }
                        .into(),
                    });
                }
            }
            Source::Input { name, file } => {
                if name.is_empty()
                    || name.contains('\0')
                    || file
                        .to_str()
                        .is_none_or(|s| s.is_empty() || s.contains('\0'))
                {
                    return Err(ValidationError {
                        origin: self.origin.clone(),
                        message: "input identities and paths must be nonempty, UTF-8 and NUL-free"
                            .into(),
                    });
                }
            }
            Source::Library | Source::Builtins => {}
        }

        Ok(())
    }

    fn node(&self) -> Node {
        Node {
            origin: self.origin.clone(),
            kind: ValueKind::Reference(self.clone()),
        }
    }
}

macro_rules! handle {
    ($name:ident, $docs:literal) => {
        #[doc = $docs]
        #[derive(Clone, Debug)]
        pub struct $name(pub(crate) Reference);

        impl $name {
            /// Inspect where Nix will look up this object and which Rust location created the reference.
            pub fn reference(&self) -> &Reference {
                &self.0
            }

            /// Represent this object as a [`NixValue`] for use in records or function calls.
            /// The lookup and Rust source location are preserved. Rust neither evaluates
            /// the object nor converts it to a string.
            pub fn as_value(&self) -> NixValue {
                NixValue(self.0.node())
            }
        }
    };
}

handle!(
    PackageRef,
    "A reference to a package that Nix will look up later.

In nixpkgs, a package describes how to build software and where its outputs will
be stored. This handle can be passed to package-specific APIs such as
[`crate::nixos::NixosModule::system_packages`] without building the package.
Rust distinguishes it from a module handle; NixOS checks whether the referenced
value really is a package. The lookup records its Rust source location."
);

handle!(
    ModuleRef,
    "A reference to an existing NixOS configuration module.

A NixOS module contributes settings or declares configurable options for a
system; NixOS combines it with other modules. Import this handle with
[`crate::nixos::NixosModule::import_ref`]. Nix evaluates the module’s contents,
and Rusnix records the Rust import location for errors in external Nix code."
);

handle!(
    NixFunction,
    "A reference to a Nix function that Rust can describe calls to.

Use it to reuse existing nixpkgs helpers or functions from a local Nix file.
[`Self::call`] passes one argument; [`Self::apply`] passes arguments one at a
time, as Nix functions commonly require. Neither executes the function in Rust.
Nix checks the arguments, and the result is a [`NixValue`]; Rusnix does not
infer that a function returns a package or module."
);

/// A nixpkgs package definition with named dependencies and feature options.
///
/// A package is commonly defined by a Nix function such as:
/// ```nix
/// { stdenv, lib, openssl, ... }: stdenv.mkDerivation { /* recipe */ }
/// ```
/// [`Nixpkgs::call_package`] examines its argument names and supplies matching
/// dependencies from nixpkgs, with explicit caller arguments taking precedence.
/// Construct one with [`Self::from_function_attrs`]; it stays deferred and supports
/// direct placement through [`ConfigValue`]. Typed calls and bindings retain its
/// interface; [`Self::as_value`] remains available for dynamic inspection.
/// Native defaults and `builtins.functionArgs` are preserved. The instantiated
/// result supports nixpkgs' `.override` machinery where Nix permits it.
///
/// The result parameter preserves the body's declared expression interface.
/// Its default is NixValue for dynamic interop; a Package result describes a
/// package, while `NixAttrs<Package>` can describe a family. Rust does not
/// validate external nixpkgs dependencies or prove that they return a derivation;
/// Nix checks the actual arguments and behavior. It wraps the existing function
/// expression, whereas [`NixFunction`] is a reference to an existing function.
#[derive(Clone, Debug)]
pub struct PackageFunction<R: NixExpression = NixValue> {
    function: NixValue,
    result: std::marker::PhantomData<R>,
}

impl<R: NixExpression> PackageFunction<R> {
    /// Define named dependencies/options, optional lazy defaults, and a body.
    /// This uses [`NixValue::function_attrs`]: Rust constructs the expression once
    /// with placeholders, and Nix supplies arguments and evaluates the body later.
    /// Defaults may depend on other arguments and remain unforced until needed.
    ///
    /// ```
    /// use rusnix_ir::{Config, interop::{NixValue, Nixpkgs, PackageFunction}};
    ///
    /// let factory = PackageFunction::from_function_attrs(["lib", "label"], |args| {
    ///     (vec![("label", "example".into())], args.select("label"))
    /// });
    /// let result = Nixpkgs::new().call_package(&factory, NixValue::record([] as [(&str, NixValue); 0]));
    /// let output = Config::new().set("factory", factory).set("result", result);
    /// ```
    #[track_caller]
    pub fn from_function_attrs<K: Into<String>>(
        arguments: impl IntoIterator<Item = impl Into<String>>,
        build: impl FnOnce(NixValue) -> (Vec<(K, NixValue)>, R),
    ) -> Self {
        Self {
            function: NixValue::function_attrs(arguments, |args| {
                let (defaults, result) = build(args);
                (defaults, result.as_expression())
            }),
            result: std::marker::PhantomData,
        }
    }

    /// Call this definition directly, preserving its declared result interface.
    /// For dependency injection and override support, prefer Nixpkgs::call_package.
    #[track_caller]
    pub fn call(&self, arguments: impl ConfigValue) -> R {
        R::from_expression(self.function.clone().call(arguments))
    }

    /// Use this function in ordinary Nix calls, records or `functionArgs` inspection.
    /// The expression and its construction provenance are preserved without evaluation.
    pub fn as_value(&self) -> NixValue {
        self.function.clone()
    }
}

impl<R: NixExpression> NixExpression for PackageFunction<R> {
    fn from_expression(function: NixValue) -> Self {
        Self {
            function,
            result: std::marker::PhantomData,
        }
    }

    fn as_expression(&self) -> NixValue {
        self.function.clone()
    }
}

impl<R: NixExpression> crate::IntoRusnixValue for PackageFunction<R> {
    #[track_caller]
    fn into_value(self) -> crate::RusnixValue {
        crate::RusnixValue::leaf(self)
    }
}

impl<R: NixExpression> sealed::Sealed for PackageFunction<R> {}

impl<R: NixExpression> ConfigValue for PackageFunction<R> {
    fn into_node(self, _: Origin) -> Node {
        self.function.0
    }
}

impl<R: NixExpression> From<PackageFunction<R>> for NixValue {
    fn from(value: PackageFunction<R>) -> Self {
        value.function
    }
}

handle!(
    OverlayRef,
    "A reference to a Nix function that extends or replaces packages in nixpkgs.

Such a function is called an *overlay*. It receives the final package set and
the preceding package set, then returns named additions or replacements. Pass
it to [`Nixpkgs::with_overlay`]; Nix applies it later. Rust does not inspect
package internals or verify the overlay’s function arguments."
);

impl sealed::Sealed for PackageRef {}

impl ConfigValue for PackageRef {
    fn into_node(self, _: Origin) -> Node {
        self.0.node()
    }
}

/// A Rust description of a value or expression that Nix will evaluate later.
///
/// This can represent a literal such as `true` or `"hello"`, a list or record,
/// a package lookup, or a function call. Constructing it does not run Nix or
/// read a result into Rust. Even a value created from a concrete Rust literal
/// is stored as a description to include in generated Nix.
///
/// An attribute set is Nix’s collection of named fields, like a Rust record or
/// map. [`Self::record`] and [`Self::list`] can mix literals, package references
/// and expressions that depend on final NixOS options. These objects remain
/// Nix values; they are not converted to strings. Use [`Self::to_text`] when
/// conversion to text is intended.
///
/// Unlike [`crate::Expr<T>`], this wrapper does not promise a particular Rust
/// result type. Nix checks field existence, function arguments and actual value
/// types when it evaluates the expression. Rusnix also records the Rust operations
/// that constructed it, so errors can be mapped back to source.
/// To construct one record from a user-defined Rust struct, derive
/// [`crate::IntoRusnixValue`] and call
/// [`try_into_nix_value`](crate::IntoRusnixValue::try_into_nix_value).
/// [`crate::RusnixValue`] is the structural intermediate used by custom
/// conversions, rather than the value passed to Nix functions.
///
/// ```
/// use rusnix_ir::interop::{NixValue, Nixpkgs};
/// let args = NixValue::record([
///     ("enabled", true.into()),
///     ("package", Nixpkgs::new().get("git").into()),
/// ]);
/// // Describes { enabled = true; package = pkgs.git; }, without evaluating pkgs.git.
/// ```
///
/// `From` accepts supported literals, expressions, handles, `Option<T>` and
/// `BTreeMap<String, NixValue>`. `None` becomes Nix `null`. Floating literals
/// must be finite normal numbers or zero; validation rejects NaN, infinity and
/// subnormal numbers. `!value` constructs Nix boolean negation.
#[derive(Clone, Debug)]
pub struct NixValue(
    /// The literal, lookup or operation to emit in Nix, with its Rust source location.
    Node,
);

impl sealed::Sealed for NixValue {}

impl ConfigValue for NixValue {
    fn into_node(self, _: Origin) -> Node {
        self.0
    }
}

impl NixValue {
    pub(crate) fn from_node(node: Node) -> Self {
        Self(node)
    }

    /// Refer to a function or value supplied by the Nix language, such as `replaceStrings`.
    /// This represents `builtins.<name>`, separately from nixpkgs' caller-supplied `lib`.
    /// Rust does not execute the builtin; use [`Self::apply`] to describe its arguments.
    /// The name is one literal attribute, checked for existence later by Nix.
    #[track_caller]
    pub fn builtin(name: &str) -> Self {
        Self(
            Reference {
                source: Source::Builtins,
                path: Some(AttrPath::segments([name])),
                origin: Origin::caller(format!("Nix builtin lookup {name}")),
            }
            .node(),
        )
    }

    /// Combine two Nix attribute sets, with right-hand fields replacing left-hand fields.
    /// This represents `left // right`; replacement is shallow and selected values stay lazy.
    /// Nix checks both operands when needed. This is distinct from NixOS definition merging.
    #[track_caller]
    pub fn merge_attrs(self, right: impl Into<Self>) -> Self {
        Self(Node {
            origin: Origin::caller("Nix attribute-set union"),
            kind: ValueKind::AttrMerge(Box::new(self.0), Box::new(right.into().0)),
        })
    }

    /// Require a Nix condition before returning a value: `assert condition; value`.
    /// Rust constructs the expression without checking the condition. Nix evaluates it
    /// when the result is demanded, and a false condition leaves `value` unevaluated.
    /// This uses Nix's assertion syntax, independent of `lib.throwIfNot`, and does not
    /// contribute to the NixOS module assertion list.
    #[track_caller]
    pub fn assert(condition: impl Into<Self>, value: impl Into<Self>) -> Self {
        Self(Node {
            origin: Origin::caller("Nix expression assertion"),
            kind: ValueKind::Assert(Box::new(condition.into().0), Box::new(value.into().0)),
        })
    }

    /// Describe a Nix function that takes one argument and computes a result.
    /// Use this when an existing Nix helper expects a callback, such as a predicate
    /// or a `stdenv.mkDerivation` attribute builder.
    ///
    /// Rust executes `build` once with a placeholder [`NixValue`]. The returned
    /// expression becomes the function body; Nix supplies the actual argument later.
    /// Nixpkgs calls the attribute-builder argument `finalAttrs`: its fields reflect
    /// later derivation overrides, rather than a Rust snapshot of the build recipe.
    /// Outer Nix parameters remain accessible inside nested functions. A placeholder
    /// used outside its function is rejected during Rusnix validation.
    #[track_caller]
    pub fn function(build: impl FnOnce(Self) -> Self) -> Self {
        let origin = Origin::caller("opaque Nix callback");
        let (binding, parameter) = Self::parameter(&origin);
        Self(Node {
            origin,
            kind: ValueKind::Function {
                binding,
                body: Box::new(build(parameter).0),
            },
        })
    }

    fn parameter(origin: &Origin) -> (u64, Self) {
        use std::sync::atomic::{AtomicU64, Ordering};

        static NEXT: AtomicU64 = AtomicU64::new(0);

        let binding = NEXT.fetch_add(1, Ordering::Relaxed);
        let parameter = Self(Node {
            origin: origin.clone(),
            kind: ValueKind::Parameter(binding),
        });

        (binding, parameter)
    }

    /// Describe a Nix function with named arguments and optional defaults.
    /// For example, the interface `["name", "label"]` can represent
    /// `{ name, label ? name }: label`. This is the usual shape of a nixpkgs package
    /// function, where named arguments supply dependencies and feature choices.
    ///
    /// Rust executes `build` once with placeholders for the arguments. Return the
    /// named defaults and body expression; names without defaults are required.
    /// Nix evaluates defaults only when needed, and defaults can refer to other
    /// arguments. Rust never reads the caller’s argument values.
    ///
    /// nixpkgs’ `callPackage` calls a package function and automatically supplies
    /// named dependencies from its package set. The generated function supports
    /// that mechanism and Nix’s `builtins.functionArgs` inspection of argument names
    /// and whether defaults exist. Nix checks missing or unexpected arguments.
    ///
    /// Names must be valid Nix parameter identifiers and cannot start with
    /// `__rusnix_`. Known selections use parameter names directly. Whole-record uses
    /// or shadowed outer names retain a lazy record representation; placeholders
    /// that escape their function are rejected.
    ///
    /// ```
    /// use rusnix_ir::interop::NixValue;
    /// let factory = NixValue::function_attrs(["name", "label"], |args| {
    ///     let name = args.clone().select("name");
    ///     (vec![("label", name)], args.select("label"))
    /// });
    /// let label = factory.call(NixValue::record([("name", "git".into())]));
    /// // Nix will return "git" using the label default; Rust only describes the call.
    /// ```
    #[track_caller]
    pub fn function_attrs<K: Into<String>>(
        arguments: impl IntoIterator<Item = impl Into<String>>,
        build: impl FnOnce(Self) -> (Vec<(K, Self)>, Self),
    ) -> Self {
        let origin = Origin::caller("Nix argument-set function");
        let (binding, parameter) = Self::parameter(&origin);
        let (defaults, body) = build(parameter);
        Self(Node {
            origin,
            kind: ValueKind::FunctionAttrs {
                binding,
                arguments: arguments.into_iter().map(Into::into).collect(),
                defaults: defaults
                    .into_iter()
                    .map(|(name, value)| (name.into(), value.0))
                    .collect(),
                body: Box::new(body.0),
            },
        })
    }

    /// Choose between two values when Nix evaluates a boolean condition.
    /// This represents `if condition then yes else no`. Both expressions are
    /// constructed in Rust, but Nix evaluates only the chosen branch. Nix checks
    /// the condition’s actual type; the result remains a [`NixValue`].
    #[track_caller]
    pub fn if_else(condition: impl Into<Self>, yes: impl Into<Self>, no: impl Into<Self>) -> Self {
        Self(Node {
            origin: Origin::caller("opaque Nix choice"),
            kind: ValueKind::If(
                Box::new(condition.into().0),
                Box::new(yes.into().0),
                Box::new(no.into().0),
            ),
        })
    }

    /// Construct boolean AND for Nix to evaluate later.
    /// Both operands must evaluate to booleans, but Nix evaluates the right operand
    /// only when the left is true. This shares [`crate::Expr::and`] semantics and
    /// uses Nix conditionals independently of nixpkgs `lib`.
    #[track_caller]
    pub fn and(self, other: impl Into<Self>) -> Self {
        self.into_expr::<bool>()
            .and(other.into().into_expr::<bool>())
            .into()
    }

    /// Construct boolean OR, sharing [`crate::Expr::or`] semantics.
    /// Nix checks demanded operands as booleans and evaluates the right operand
    /// only when the left is false. No caller-supplied library is consulted.
    #[track_caller]
    pub fn or(self, other: impl Into<Self>) -> Self {
        self.into_expr::<bool>()
            .or(other.into().into_expr::<bool>())
            .into()
    }

    /// Construct boolean implication, sharing [`crate::Expr::implies`] semantics.
    /// Nix checks demanded operands as booleans. A false left operand returns true
    /// without evaluating the right operand. No caller-supplied library is consulted.
    #[track_caller]
    pub fn implies(self, other: impl Into<Self>) -> Self {
        self.into_expr::<bool>()
            .implies(other.into().into_expr::<bool>())
            .into()
    }

    /// Compare two values using Nix’s `==` operation when they are evaluated.
    /// The result represents a Nix boolean; no comparison happens in Rust.
    #[track_caller]
    pub fn equals(self, other: impl Into<Self>) -> Self {
        Self(Node {
            origin: Origin::caller("opaque Nix equality"),
            kind: ValueKind::Equal(Box::new(self.0), Box::new(other.into().0)),
        })
    }

    /// Convert a value to text later using Nix’s `builtins.toString`.
    /// `builtins` contains functions supplied by the Nix language itself, separately
    /// from nixpkgs’ `lib`. Unsupported conversions fail during Nix evaluation.
    ///
    /// Nix strings can carry dependencies on package outputs as well as text bytes.
    /// This method preserves those dependencies, known as *string context*, instead
    /// of flattening a package path into an unrelated Rust string.
    #[track_caller]
    pub fn to_text(self) -> Self {
        Self(Node {
            origin: Origin::caller("opaque Nix to text"),
            kind: ValueKind::ToText(Box::new(self.0)),
        })
    }

    /// Describe a supported Rust literal or preserve an existing Nix expression.
    /// Rust literals become Nix literals; expression and reference inputs keep their
    /// recorded source locations. Float values must be finite normal numbers or zero;
    /// Rusnix validation rejects NaN, infinity and subnormals.
    #[track_caller]
    pub fn literal(value: impl ConfigValue) -> Self {
        Self(value.into_node(Origin::caller("opaque call argument")))
    }

    /// Construct explicit Nix null, useful for nullable settings and function arguments.
    #[track_caller]
    pub fn null() -> Self {
        Self(Node {
            origin: Origin::caller("opaque Nix null"),
            kind: ValueKind::Null,
        })
    }

    /// Build a Nix attribute set: a collection of named values, like a Rust record.
    /// It stays one value when passed to a function or configuration field; its keys
    /// are not expanded into configuration paths.
    ///
    /// Keys are literal names, so `"a.b"` is one field rather than two nested fields.
    /// A `BTreeMap` can supply dynamic keys. Iterator order determines generated field
    /// order, while Nix owns attribute-set semantics. Duplicate or NUL-containing
    /// keys are rejected during validation; empty names are allowed.
    #[track_caller]
    pub fn record(fields: impl IntoIterator<Item = (impl Into<String>, Self)>) -> Self {
        Self(Node {
            origin: Origin::caller("opaque Nix record"),
            kind: ValueKind::OpaqueRecord(
                fields
                    .into_iter()
                    .map(|(key, value)| (key.into(), value.0))
                    .collect(),
            ),
        })
    }

    /// Describe an ordered Nix list without evaluating its elements in Rust.
    /// Nix evaluates an element only when it is needed; elements can mix literals,
    /// packages and other expressions.
    #[track_caller]
    pub fn list(items: impl IntoIterator<Item = Self>) -> Self {
        Self(Node {
            origin: Origin::caller("opaque Nix list"),
            kind: ValueKind::List(items.into_iter().map(|value| value.0).collect()),
        })
    }

    /// Join several deferred Nix lists, preserving their element order.
    /// Uses `builtins.concatLists`, independently of nixpkgs `lib`. No inputs
    /// produce an empty list; individual element values remain lazy. Nix checks
    /// that each input evaluates to a list when the result is needed.
    /// Use [`NixLibrary::concat_lists`] to call the supplied library's function.
    #[track_caller]
    pub fn concat_lists(lists: impl IntoIterator<Item = Self>) -> Self {
        Self::builtin("concatLists").call(Self::list(lists))
    }

    /// Pass one argument to a Nix function when Nix evaluates the expression.
    /// Many Nix functions take arguments one at a time: chain calls to represent
    /// `function first second`, or use [`Self::apply`]. Rust does not execute the call.
    #[track_caller]
    pub fn call(self, argument: impl ConfigValue) -> Self {
        let origin = Origin::caller("opaque Nix function call");
        Self(Node {
            origin: origin.clone(),
            kind: ValueKind::Apply(Box::new(self.0), Box::new(argument.into_node(origin))),
        })
    }

    /// Pass arguments to a Nix function one at a time, in the supplied order.
    /// `function.apply([a, b])` represents `function a b`. This is repeated Nix
    /// function application, not a Rust call or a single list argument. Nix checks
    /// the arguments and evaluates them as required by the function.
    #[track_caller]
    pub fn apply(mut self, arguments: impl IntoIterator<Item = Self>) -> Self {
        // A direct call keeps track_caller; a function-pointer fold loses it.
        for argument in arguments {
            self = self.call(argument);
        }
        self
    }

    /// Join a Nix list of strings using `separator` between each pair.
    /// The list itself can be computed by Nix later. Elements must evaluate to
    /// strings; use [`Self::to_text`] for conversion. Package dependencies attached
    /// to each string are retained.
    #[track_caller]
    pub fn join_text(separator: &str, parts: Self) -> Self {
        Nixpkgs::new()
            .function("concatStringsSep")
            .apply([separator.into(), parts])
    }

    /// Join the supplied expressions as text with no separator.
    /// Rust chooses the sequence of parts; Nix evaluates their string values later.
    /// For a mostly literal template with named holes, prefer [`crate::nix_text!`].
    #[track_caller]
    pub fn concat_text(parts: impl IntoIterator<Item = Self>) -> Self {
        Self::join_text("", Self::list(parts))
    }

    /// Replace text using ordered `(from, to)` pairs through `builtins.replaceStrings`.
    /// Inputs stay deferred and string dependencies are retained. This follows Nix's
    /// replacement rules, rather than applying a sequence of Rust string replacements.
    /// Use [`NixLibrary::replace_text`] to call the supplied library instead.
    #[track_caller]
    pub fn replace_text(
        self,
        replacements: impl IntoIterator<Item = (impl Into<Self>, impl Into<Self>)>,
    ) -> Self {
        let [from, to] = replacement_lists(replacements);
        Self::builtin("replaceStrings").apply([from, to, self])
    }

    /// Test for one literal attribute name supplied as text, possibly computed by Nix.
    /// Dots and punctuation remain part of the name, rather than describing a path.
    /// This uses `builtins.hasAttr` and does not evaluate the attribute's value.
    #[track_caller]
    pub fn has_attr(self, name: impl Into<Self>) -> Self {
        Self::builtin("hasAttr").apply([name.into(), self])
    }

    /// Select one literal attribute name, or evaluate `fallback` if it is absent.
    /// The name may be computed by Nix; dots and punctuation remain literal data.
    /// An existing null value is returned as null. The unchosen value stays lazy.
    /// Uses `builtins.hasAttr` and `builtins.getAttr`, independently of nixpkgs `lib`.
    #[track_caller]
    pub fn attr_or(self, name: impl Into<Self>, fallback: impl Into<Self>) -> Self {
        let name = name.into();
        let fallback = fallback.into();
        let origin = Origin::caller("opaque Nix attribute fallback");
        let (attrs_binding, attrs) = Self::parameter(&origin);
        let (name_binding, attribute_name) = Self::parameter(&origin);
        // Build outside Rust callbacks so track_caller reaches the public call.
        // Nix callbacks share both inputs while keeping the fallback lazy.
        let selected = Self::if_else(
            attrs.clone().has_attr(attribute_name.clone()),
            Self::builtin("getAttr").apply([attribute_name, attrs]),
            fallback,
        );
        let lookup = Self(Node {
            origin: origin.clone(),
            kind: ValueKind::Function {
                binding: name_binding,
                body: Box::new(selected.0),
            },
        });
        Self(Node {
            origin,
            kind: ValueKind::Function {
                binding: attrs_binding,
                body: Box::new(lookup.call(name).0),
            },
        })
        .call(self)
    }

    /// Call this package's supplied `override` function with argument changes.
    /// `overrides` may be a record or a Nix callback accepted by the package.
    /// nixpkgs owns default handling and dependency splicing; Rust only describes
    /// the call and retains the returned package's ordinary override interface.
    #[track_caller]
    pub fn override_args(self, overrides: impl Into<Self>) -> Self {
        self.select("override").call(overrides.into())
    }

    /// Call this package's supplied `overrideAttrs` function with an update.
    /// Pass a record or deferred callback accepted by the package's implementation,
    /// including nested callbacks for the recursive final/previous-attributes form.
    /// nixpkgs owns attribute and self-reference semantics; Rust does not rebuild
    /// or inspect the derivation.
    #[track_caller]
    pub fn override_attrs(self, update: impl Into<Self>) -> Self {
        self.select("overrideAttrs").call(update.into())
    }

    /// Look up named fields in a Nix attribute set later.
    /// `value.select("foo.bar")` represents `value.foo.bar`; it does not read
    /// fields into Rust. Missing fields can be reported at this Rust call.
    /// Use [`Self::select_segments`] when a field name itself contains a dot.
    #[track_caller]
    pub fn select(self, path: &str) -> Self {
        self.select_segments(path.split('.'))
    }

    /// Look up a sequence of literal field names in Nix attribute sets.
    /// For example, `["foo.bar", "baz"]` selects `value."foo.bar".baz`, preserving
    /// the dot within the first name. The lookup records this Rust call location.
    /// Inside named-argument functions, known arguments use their Nix parameter
    /// bindings directly.
    #[track_caller]
    pub fn select_segments(self, parts: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let path = AttrPath::segments(parts);
        Self(Node {
            origin: Origin::caller(format!("opaque Nix selection {}", path.parts().join("."))),
            kind: ValueKind::Select(Box::new(self.0), path),
        })
    }

    /// Attach an expected scalar type so Rust can offer typed expression operations.
    /// For example, an `Expr<i64>` permits [`crate::Expr::divide`]. This does not
    /// read or check the Nix value. Supported expectations are `bool`, `String` and
    /// `i64`; Nix still checks the actual type when the expression is evaluated.
    pub fn into_expr<T>(self) -> crate::Expr<T>
    where
        crate::Expr<T>: ConfigValue,
    {
        crate::Expr {
            node: self.0,
            ty: std::marker::PhantomData,
        }
    }
}

// Both builtin and library replacements use aligned lists without reading any text.
#[track_caller]
fn replacement_lists(
    replacements: impl IntoIterator<Item = (impl Into<NixValue>, impl Into<NixValue>)>,
) -> [NixValue; 2] {
    let mut from = Vec::new();
    let mut to = Vec::new();
    // Conversions inside a Rust iterator callback would lose track_caller.
    for (pattern, replacement) in replacements {
        from.push(pattern.into());
        to.push(replacement.into());
    }
    [NixValue::list(from), NixValue::list(to)]
}

/// Negates a deferred Nix boolean, preserving its origin and checking its type in Nix.
impl std::ops::Not for NixValue {
    type Output = Self;

    #[track_caller]
    fn not(self) -> Self {
        Self::if_else(self, false, true)
    }
}

macro_rules! literal_conversion {
    ($($ty:ty),* $(,)?) => { $(
        impl From<$ty> for NixValue {
            #[track_caller]
            fn from(value: $ty) -> Self { Self::literal(value) }
        }
    )* };
}

literal_conversion!(
    bool,
    i32,
    i64,
    u16,
    f64,
    String,
    &str,
    crate::Expr<bool>,
    crate::Expr<i64>,
    crate::Expr<String>
);

macro_rules! reference_conversion {
    ($($ty:ty),* $(,)?) => { $(
        impl From<$ty> for NixValue {
            fn from(value: $ty) -> Self { value.as_value() }
        }
    )* };
}

reference_conversion!(PackageRef, ModuleRef, NixFunction, OverlayRef);

impl<T: Into<NixValue>> From<Option<T>> for NixValue {
    #[track_caller]
    fn from(value: Option<T>) -> Self {
        match value {
            Some(value) => value.into(),
            None => Self::null(),
        }
    }
}

impl From<std::collections::BTreeMap<String, NixValue>> for NixValue {
    #[track_caller]
    fn from(value: std::collections::BTreeMap<String, NixValue>) -> Self {
        Self::record(value)
    }
}

impl NixFunction {
    /// Declare the expected result interface of an external callable reference.
    /// This does not inspect its Nix implementation or evaluate its result.
    pub fn returning<R: NixExpression>(&self) -> NixCallable<R> {
        NixCallable::from_expression(self.as_value())
    }

    /// State both parameter and result expectations for an external callable.
    /// This does not inspect or eagerly validate the external Nix function.
    #[track_caller]
    pub fn signature<A: NixExpression, R: NixExpression>(&self) -> NixCallable<R, A> {
        NixCallable::from_expression(self.as_value())
    }

    /// Describe a call to this Nix function with one argument.
    /// Nix executes the call later and checks the argument. If the result is another
    /// function, continue with [`NixValue::call`] or use [`Self::apply`].
    #[track_caller]
    pub fn call(&self, argument: impl ConfigValue) -> NixValue {
        self.as_value().call(argument)
    }

    /// Describe successive calls to this Nix function, one for each argument.
    /// For example, `write_text.apply([name, text])` represents `writeText name text`.
    /// Arguments can mix Rust literals, records, package references and expressions.
    /// Nix executes the calls later; Rusnix does not infer the result’s category.
    ///
    /// ```
    /// use rusnix_ir::interop::Nixpkgs;
    /// let file = Nixpkgs::new().pkgs_function("writeText")
    ///     .apply(["example.conf".into(), "workers=4\n".into()]);
    /// // Nix will describe a generated file; constructing this call does not build it.
    /// ```
    #[track_caller]
    pub fn apply(&self, arguments: impl IntoIterator<Item = NixValue>) -> NixValue {
        self.as_value().apply(arguments)
    }
}

/// Build text containing values that Nix will evaluate later.
/// This resembles named Rust formatting, but returns a [`NixValue`] rather
/// than a Rust `String`. Each hole uses Nix’s `builtins.toString`, so a hole
/// can contain a Rust literal, a symbolic option reference or a package.
///
/// Nix strings can carry dependencies on package outputs. Interpolation keeps
/// those dependencies and the child expressions’ Rust locations; it does not
/// read symbolic values into Rust. Nix evaluates the text only when needed.
///
/// ```
/// use rusnix_ir::{nix_text, nixos::OptionRef};
/// let port = OptionRef::<i64>::new("services.example.port").into_expr();
/// let command = nix_text!("postgres --port={port}", port = port);
/// // Represents "postgres --port=" followed by Nix's toString of the final port.
/// ```
///
/// Use `{name}` for a hole and `{{` or `}}` for literal braces. Arguments must
/// be explicitly named and are constructed once, even if reused in the template.
/// Unknown, unused or duplicate names, malformed braces and formatting specifiers
/// are compile-time errors. Width, precision and debug formatting are unsupported.
///
/// # Multiline templates
///
/// A template beginning with a newline removes that first newline and a final
/// indentation-only closing line, then removes the common space/tab prefix from
/// nonblank lines. Relative indentation, blank lines and the newline before the
/// closing line remain. Tabs match tabs, not visual columns. Other templates
/// retain their exact whitespace; interpolated values are never reindented.
///
/// ```
/// use rusnix_ir::nix_text;
/// let script = nix_text!(
///     r#"
///         echo {message}
///     "#,
///     message = "ready",
/// );
/// // Describes "echo ready\n" after removing the source indentation.
/// ```
///
/// The comma-separated fragment form remains available for dynamic assembly.
/// Its parts must already be strings; use [`NixValue::to_text`] to convert them.
#[macro_export]
macro_rules! nix_text {
    ($template:literal $(, $name:ident = $value:expr)* $(,)?) => {
        $crate::__symbolic_text!($crate; $template $(, $name = $value)*)
    };
    ($($part:expr),* $(,)?) => {
        $crate::Expr::<String>::concat([$($part.into()),*])
    };
}

/// Build a Nix attribute set from named values using a concise macro.
/// An attribute set is Nix’s collection of named fields, like a Rust record or
/// map. Values can mix Rust literals with packages and expressions that Nix will
/// evaluate later; the macro does not evaluate them or convert them to strings.
///
/// ```
/// use rusnix_ir::nix_record;
/// let args = nix_record! { "name": "example", "enabled": true };
/// // Represents { name = "example"; enabled = true; }.
/// ```
///
/// Keys are literal strings or parenthesized Rust expressions. Dots remain
/// within one name rather than creating a nested path. Duplicate or NUL-containing
/// keys are rejected during validation. Prefer ordinary Rust structs for large
/// fixed records, and [`NixValue::record`] for dynamic keys.
#[macro_export]
macro_rules! nix_record {
    (@key ($key:expr)) => { $key };
    (@key $key:expr) => { $key };
    ($($key:tt : $value:expr),* $(,)?) => {{
        let fields: ::std::vec::Vec<(::std::string::String, $crate::interop::NixValue)> =
            ::std::vec![$((::std::convert::Into::into($crate::nix_record!(@key $key)), $crate::interop::NixValue::from($value))),*];
        $crate::interop::NixValue::record(fields)
    }};
}

/// Access packages and utility functions from the pinned nixpkgs collection.
///
/// nixpkgs contains build descriptions for software, plus functions and source
/// files used to describe those builds. Its *package set* is a Nix attribute set
/// whose fields include packages such as `git`, helpers such as `writeText`, and
/// `stdenv`, the standard build environment. `stdenv.mkDerivation` turns build
/// attributes into a *derivation*: a recipe with inputs and output paths, not a
/// completed build.
///
/// Use [`Self::get`] for a package, [`Self::pkgs_function`] for a build helper,
/// and [`Self::function`] for a function in nixpkgs’ separate `lib` utility library.
/// Use [`Self::call_package`] to instantiate a [`PackageFunction`] with automatic
/// dependency selection. These methods construct expressions for Nix to evaluate later. They do not
/// fetch, build or inspect packages in Rust, and no package-specific Rust bindings
/// are generated.
///
/// [`Self::new`] uses a standalone package set. [`Self::from_module`] instead
/// uses the package set supplied by NixOS, preserving its platform and customizations.
/// Library lookups always use the pinned `lib`, independently of package overlays.
#[derive(Clone, Debug, Default)]
pub struct Nixpkgs {
    /// Whether package lookups use the NixOS module’s supplied `pkgs` value.
    module_scope: bool,
    /// Functions that extend or replace packages, applied in the author’s order.
    overlays: Vec<Reference>,
}

impl Nixpkgs {
    /// Select the pinned nixpkgs collection for standalone package lookups.
    /// The current backend imports it for `x86_64-linux` with an empty nixpkgs
    /// configuration. No Nix evaluation happens here. Use [`Self::from_module`]
    /// to follow a NixOS system’s platform and package customizations.
    pub fn new() -> Self {
        Self::default()
    }

    /// Refer to a file or directory within the pinned nixpkgs source tree.
    /// Use this for an existing patch, script or other build input. The result
    /// represents a Nix path; Rust does not read the file, import Nix code or fetch it.
    /// Paths must be relative and cannot traverse to parent directories.
    #[track_caller]
    pub fn source_path(&self, path: &str) -> NixValue {
        NixValue(
            Reference {
                source: Source::PinnedPath { path: path.into() },
                path: None,
                origin: Origin::caller(format!("nixpkgs source path {path}")),
            }
            .node(),
        )
    }

    /// Select the packages supplied to the generated module by NixOS.
    /// NixOS calls this set `pkgs`; it includes the system’s platform, package
    /// configuration and overlays. References constructed here must be compiled
    /// inside a [`crate::nixos::NixosModule`], rather than as standalone configuration.
    pub fn from_module() -> Self {
        Self {
            module_scope: true,
            overlays: vec![],
        }
    }

    fn package_source(&self) -> Source {
        if self.module_scope {
            Source::NixosPackages {
                overlays: self.overlays.clone(),
            }
        } else {
            Source::Packages {
                overlays: self.overlays.clone(),
            }
        }
    }

    /// Add a function that extends or replaces packages in this package set.
    /// Nixpkgs calls these functions *overlays*; Nix applies them in the supplied
    /// order. This extends the NixOS-supplied set or configures the standalone import.
    /// Rust does not run the overlay.
    pub fn with_overlay(mut self, overlay: OverlayRef) -> Self {
        self.overlays.push(overlay.0);
        self
    }

    /// Refer to a package that Nix will look up later.
    /// `get("git")` represents `pkgs.git`; dots select nested package collections.
    /// Nix checks existence, and lookup failures retain this Rust call location.
    ///
    /// ```
    /// use rusnix_ir::interop::Nixpkgs;
    /// let git = Nixpkgs::new().get("git");
    /// // A package reference, not a built Git executable.
    /// ```
    #[track_caller]
    pub fn get(&self, path: &str) -> PackageRef {
        self.path(AttrPath::dotted(path).0)
    }

    /// Refer to a package using literal field names instead of a dotted path.
    /// For example, `["packages", "name.with.dot"]` keeps the second name intact.
    /// Nix performs the lookup later.
    #[track_caller]
    pub fn path(&self, parts: impl IntoIterator<Item = impl Into<String>>) -> PackageRef {
        let path = AttrPath::segments(parts);
        let origin = Origin::caller(format!("nixpkgs package lookup {}", path.0.join(".")));
        PackageRef(Reference {
            source: self.package_source(),
            path: Some(path),
            origin,
        })
    }

    /// Refer to an existing NixOS module file in the pinned nixpkgs tree.
    /// `file` is relative to `nixos/modules`. The module contributes configuration
    /// when imported by NixOS; Rust does not read it. Package overlays do not alter
    /// which source file this handle references.
    #[track_caller]
    pub fn module(&self, file: &str) -> ModuleRef {
        ModuleRef(Reference {
            source: Source::ModuleFile { path: file.into() },
            path: None,
            origin: Origin::caller(format!("NixOS module lookup {file}")),
        })
    }

    /// Refer to a function in nixpkgs’ `lib` utility library.
    /// For example, `function("concatStringsSep")` selects `lib.concatStringsSep`.
    /// Nix checks the function and arguments later. Build helpers such as `writeText`
    /// belong to the package set; select those with [`Self::pkgs_function`].
    #[track_caller]
    pub fn function(&self, path: &str) -> NixFunction {
        NixFunction(Reference {
            source: Source::Library,
            path: Some(AttrPath::dotted(path)),
            origin: Origin::caller(format!("nixpkgs lib function lookup {path}")),
        })
    }

    /// Wrap the pinned nixpkgs `lib` utility library with common call helpers.
    /// This uses the same library as [`Self::function`], without package-set overlays.
    /// To use the `lib` supplied by a package caller instead, wrap that argument
    /// with [`NixLibrary::from_value`].
    #[track_caller]
    pub fn library(&self) -> NixLibrary {
        NixLibrary::from_value(NixValue(
            Reference {
                source: Source::Library,
                path: None,
                origin: Origin::caller("nixpkgs library lookup"),
            }
            .node(),
        ))
    }

    /// Refer to any named value in the package set, without claiming it is a package.
    /// Use this for metadata, records or other objects. Dots select nested fields;
    /// Rust constructs the lookup and Nix evaluates it later.
    #[track_caller]
    pub fn value(&self, path: &str) -> NixValue {
        NixValue(
            Reference {
                source: self.package_source(),
                path: Some(AttrPath::dotted(path)),
                origin: Origin::caller(format!("nixpkgs value lookup {path}")),
            }
            .node(),
        )
    }

    /// Pass the entire package set as a Nix value, for helpers that expect `pkgs`.
    /// Rust does not inspect the packages or the final NixOS configuration.
    #[track_caller]
    pub fn as_value(&self) -> NixValue {
        NixValue(
            Reference {
                source: self.package_source(),
                path: None,
                origin: Origin::caller("nixpkgs package set"),
            }
            .node(),
        )
    }

    /// Refer to a named value in nixpkgs’ `lib` utility library.
    /// This includes NixOS type objects such as `types.port`; they describe how
    /// NixOS validates and merges option values. Rust does not evaluate the lookup.
    #[track_caller]
    pub fn lib_value(&self, path: &str) -> NixValue {
        NixValue(
            Reference {
                source: Source::Library,
                path: Some(AttrPath::dotted(path)),
                origin: Origin::caller(format!("nixpkgs lib value lookup {path}")),
            }
            .node(),
        )
    }

    /// Instantiate a package function using nixpkgs' dependency scope.
    /// This represents `pkgs.callPackage packageFunction overrides`: the real
    /// pinned nixpkgs helper supplies matching dependencies, and the caller's
    /// override attribute set takes precedence. Native defaults remain lazy.
    /// Lookups follow this set's overlays or NixOS-supplied package scope.
    ///
    /// Nix checks missing arguments and the package body. The factory's declared
    /// result interface is preserved; external expectations are not evaluated. Where
    /// supported by nixpkgs, [`Package::override_arguments`] changes package
    /// arguments after instantiation. Dynamic results retain ordinary Nix interop.
    #[track_caller]
    pub fn call_package<R: NixExpression>(
        &self,
        function: &PackageFunction<R>,
        overrides: impl Into<NixValue>,
    ) -> R {
        R::from_expression(
            self.pkgs_function("callPackage")
                .call(function.clone())
                .call(overrides.into()),
        )
    }

    /// Instantiate with a structured Rust override record, lowering at this boundary.
    /// Structural conversion errors are returned without evaluating any Nix values.
    #[track_caller]
    pub fn try_call_package<R: NixExpression>(
        &self,
        function: &PackageFunction<R>,
        overrides: impl crate::IntoRusnixValue,
    ) -> Result<R, ValidationError> {
        Ok(self.call_package(function, overrides.try_into_nix_value()?))
    }

    /// Refer to an arbitrary function in the package set, such as `writeText` or
    /// `stdenv.mkDerivation`. These helpers describe generated files or builds.
    /// Lookups follow this set’s package overlays. Use [`Self::function`] for
    /// the separate `lib` utility library; Nix checks function arguments later.
    /// This selector returns a generic function reference, not a [`PackageFunction`].
    /// Use [`Self::call_package`] to instantiate a package definition.
    #[track_caller]
    pub fn pkgs_function(&self, path: &str) -> NixFunction {
        NixFunction(Reference {
            source: self.package_source(),
            path: Some(AttrPath::dotted(path)),
            origin: Origin::caller(format!("nixpkgs function lookup {path}")),
        })
    }
}

/// Refer to objects defined in a local Nix expression file.
///
/// Nix can load a file with `import` and then select fields from its result.
/// Use this handle for packages, modules, overlays or functions that are not
/// in the pinned nixpkgs collection. Rust records the file and lookup location;
/// Nix evaluates its contents later. This API does not fetch inputs or use flakes.
#[derive(Clone, Debug)]
pub struct InputRef {
    /// The local expression file and its human-readable name for diagnostics.
    source: Source,
}

impl InputRef {
    /// Identify a local Nix expression file to load later.
    /// `name` labels the input in diagnostics. Rust does not open or evaluate the
    /// file. Relative paths are resolved from Rust’s working directory when compiled;
    /// the file must still be available when Nix evaluates the generated expression.
    pub fn local(name: impl Into<String>, file: impl Into<PathBuf>) -> Self {
        Self {
            source: Source::Input {
                name: name.into(),
                file: file.into(),
            },
        }
    }

    #[track_caller]
    fn lookup(&self, path: &str, category: &str) -> Reference {
        Reference {
            source: self.source.clone(),
            path: Some(AttrPath::dotted(path)),
            origin: Origin::caller(format!("external {category} lookup {path}")),
        }
    }

    /// Refer to a package returned by this local Nix file.
    /// Dots select nested fields in the file’s result; Nix checks their existence
    /// and whether the result is suitable as a package.
    #[track_caller]
    pub fn package(&self, path: &str) -> PackageRef {
        PackageRef(self.lookup(path, "package"))
    }

    /// Refer to a NixOS configuration module returned by this file.
    /// Pass the handle to [`crate::nixos::NixosModule::import_ref`] to combine it
    /// with other modules; Nix evaluates its contents later.
    #[track_caller]
    pub fn module(&self, path: &str) -> ModuleRef {
        ModuleRef(self.lookup(path, "module"))
    }

    /// Refer to a Nix function returned by this file.
    /// Its calls are described in Rust and executed by Nix later; Rust does not
    /// infer the function’s argument or result types.
    #[track_caller]
    pub fn function(&self, path: &str) -> NixFunction {
        NixFunction(self.lookup(path, "function"))
    }

    /// Refer to an overlay function returned by this file.
    /// An overlay extends or replaces packages; pass it to [`Nixpkgs::with_overlay`]
    /// to have Nix apply it later.
    #[track_caller]
    pub fn overlay(&self, path: &str) -> OverlayRef {
        OverlayRef(self.lookup(path, "overlay"))
    }

    /// Refer to any value returned by this file, without assuming its category.
    /// Dots select nested fields. The resulting expression is evaluated by Nix later.
    #[track_caller]
    pub fn value(&self, path: &str) -> NixValue {
        NixValue(self.lookup(path, "value").node())
    }
}
