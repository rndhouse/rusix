//! Structured values and opaque handles for existing Nix ecosystem objects.
//!
//! Rust checks broad categories such as packages versus modules. Nix remains
//! authoritative for existence, function schemas and actual object types. Lookups,
//! calls and text coercion stay deferred and preserve their Rust provenance.
use crate::{ConfigValue, Node, Origin, ValidationError, ValueKind, sealed};
use std::path::{Component, PathBuf};

/// Literal attribute segments for an opaque lookup or symbolic option dependency.
/// Segment contents are data, not Nix source; use [`Self::segments`] for names with dots.
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

/// Backend-facing identity of the Nix environment in which a reference is resolved.
/// Authors normally obtain these through [`Nixpkgs`] or [`InputRef`].
#[derive(Clone, Debug)]
pub enum Source {
    /// The pinned standalone package set, with Nix applying overlays in order.
    Packages {
        /// Deferred overlay functions, ordered as supplied by the author.
        overlays: Vec<Reference>,
    },
    /// The pinned nixpkgs library, independent of package-set overlays.
    Library,
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

/// Backend-facing description of a deferred ecosystem lookup with Rust provenance.
/// Category handles expose this metadata for inspection; it is not an evaluated value.
#[derive(Clone, Debug)]
pub struct Reference {
    /// Environment or input that owns the referenced object.
    pub source: Source,
    /// Literal selection within that source, or `None` for the source itself.
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
            Source::ModuleFile { path } => {
                if path.is_empty()
                    || path.contains('\0')
                    || PathBuf::from(path)
                        .components()
                        .any(|c| !matches!(c, Component::Normal(_)))
                {
                    return Err(ValidationError {
                        origin: self.origin.clone(),
                        message:
                            "module paths must be relative, nonempty paths without parent traversal"
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
            Source::Library => {}
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
            /// Inspect source/path provenance without resolving the opaque value.
            pub fn reference(&self) -> &Reference {
                &self.0
            }

            /// Pass this deferred object through the generic [`NixValue`] boundary.
            /// Preserves the lookup and its origin; does not stringify or validate it.
            pub fn as_value(&self) -> NixValue {
                NixValue(self.0.node())
            }
        }
    };
}

handle!(
    PackageRef,
    "An opaque package lookup for package-category APIs such as system packages.
Rust prevents module/package interchange; NixOS checks whether the resolved object
is really a package. No package-specific Rust bindings are needed."
);

handle!(
    ModuleRef,
    "An opaque existing NixOS module, obtained from nixpkgs or a local input.
Import with [`crate::nixos::NixosModule::import_ref`]; Nix evaluates its contents
and Rusnix retains the Rust import boundary when upstream errors occur."
);

handle!(
    NixFunction,
    "An opaque callable selected from nixpkgs/lib, a package set or a local input.
Calls remain deferred; Nix owns the function schema, currying and result type.
Results are generic [`NixValue`] values, not inferred package or module handles."
);

handle!(
    OverlayRef,
    "An opaque overlay function for [`Nixpkgs::with_overlay`]. Nix applies its
ordinary overlay semantics; Rusnix neither models package internals nor verifies
the overlay's function schema in Rust."
);

impl sealed::Sealed for PackageRef {}

impl ConfigValue for PackageRef {
    fn into_node(self, _: Origin) -> Node {
        self.0.node()
    }
}

/// A structured or opaque value resolved in Nix, including deferred expressions.
/// Records/lists may mix Rust literals, package references and symbolic option
/// dependencies without converting them to strings. Use [`Self::to_text`] only
/// when text coercion is intended; Nix owns object types and function schemas.
///
/// Constructors build a graph and record Rust origins without evaluating it.
/// `From` conversions support literals, supported [`crate::Expr`] types, handles,
/// `Option<T>` (`None` becomes null), and `BTreeMap<String, NixValue>`.
/// Floating-point literals must be finite normal values or zero; IR validation
/// rejects NaN, infinities and subnormals instead of emitting invalid Nix numbers.
#[derive(Clone, Debug)]
pub struct NixValue(Node);

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

    /// Describe a scoped callback for an existing Nix function. Rust executes
    /// the builder once with a symbolic parameter; it never reads Nix values.
    /// Captured outer parameters remain lexical; escaping a parameter outside its
    /// callback is an IR validation error. Nix invokes the resulting function.
    #[track_caller]
    pub fn function(build: impl FnOnce(Self) -> Self) -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};

        static NEXT: AtomicU64 = AtomicU64::new(0);

        let binding = NEXT.fetch_add(1, Ordering::Relaxed);
        let origin = Origin::caller("opaque Nix callback");
        let parameter = Self(Node {
            origin: origin.clone(),
            kind: ValueKind::Parameter(binding),
        });

        Self(Node {
            origin,
            kind: ValueKind::Function {
                binding,
                body: Box::new(build(parameter).0),
            },
        })
    }

    /// Build both branches in Rust, but demand only the selected branch in Nix.
    /// Nix checks the condition's actual boolean type; the result stays opaque.
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

    /// Defer native Nix equality, returning an opaque boolean-valued expression.
    #[track_caller]
    pub fn equals(self, other: impl Into<Self>) -> Self {
        Self(Node {
            origin: Origin::caller("opaque Nix equality"),
            kind: ValueKind::Equal(Box::new(self.0), Box::new(other.into().0)),
        })
    }

    /// Coerce with Nix `builtins.toString`, retaining string dependency context.
    /// Coercion and failures happen in Nix, not Rust; unsupported values may fail.
    #[track_caller]
    pub fn to_text(self) -> Self {
        Self(Node {
            origin: Origin::caller("opaque Nix to text"),
            kind: ValueKind::ToText(Box::new(self.0)),
        })
    }

    /// Native leaves, preserving captured expression/reference origins. Float
    /// literals support finite normal f64 values and zero; validation rejects
    /// NaN, infinities and subnormals, which Nix cannot parse as literals.
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

    /// Keys are literal attribute names, not dotted paths. Accepts BTreeMap
    /// directly; iterator order determines the deterministic generated order.
    /// Nested references remain opaque. Duplicate or NUL-containing keys are
    /// rejected by validation; empty literal attribute names are permitted here.
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

    /// Build ordered deferred elements without evaluating their values.
    #[track_caller]
    pub fn list(items: impl IntoIterator<Item = Self>) -> Self {
        Self(Node {
            origin: Origin::caller("opaque Nix list"),
            kind: ValueKind::List(items.into_iter().map(|value| value.0).collect()),
        })
    }

    /// One ordinary Nix application. Chain calls for curried functions.
    #[track_caller]
    pub fn call(self, argument: impl ConfigValue) -> Self {
        let origin = Origin::caller("opaque Nix function call");
        Self(Node {
            origin: origin.clone(),
            kind: ValueKind::Apply(Box::new(self.0), Box::new(argument.into_node(origin))),
        })
    }

    /// Apply curried arguments in order. Nix owns their schemas and evaluation.
    #[track_caller]
    pub fn apply(mut self, arguments: impl IntoIterator<Item = Self>) -> Self {
        // A direct call keeps track_caller; a function-pointer fold loses it.
        for argument in arguments {
            self = self.call(argument);
        }
        self
    }

    /// Join deferred strings without dropping their Nix dependency contexts.
    /// Parts is an opaque Nix list, possibly computed by a deferred callback.
    /// Its entries must evaluate to strings; use to_text for explicit coercion.
    #[track_caller]
    pub fn join_text(separator: &str, parts: Self) -> Self {
        Nixpkgs::new()
            .function("concatStringsSep")
            .apply([separator.into(), parts])
    }

    /// Concatenate Rust-selected parts while leaving their values deferred in Nix.
    #[track_caller]
    pub fn concat_text(parts: impl IntoIterator<Item = Self>) -> Self {
        Self::join_text("", Self::list(parts))
    }

    /// Select a dotted attribute path when Nix evaluates this value.
    /// Missing attributes map to this Rust operation; this does not imply a type
    /// schema or materialize the referenced record in Rust.
    #[track_caller]
    pub fn select(self, path: &str) -> Self {
        Self(Node {
            origin: Origin::caller(format!("opaque Nix selection {path}")),
            kind: ValueKind::Select(Box::new(self.0), AttrPath::dotted(path)),
        })
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
    /// Supply one curried argument; Nix checks its schema and returns an opaque value.
    /// Continue with [`NixValue::call`] if more arguments are required.
    #[track_caller]
    pub fn call(&self, argument: impl ConfigValue) -> NixValue {
        self.as_value().call(argument)
    }

    /// Apply any number of curried arguments through the opaque boundary.
    /// Arguments can contain records, symbolic expressions and native references.
    ///
    /// ```
    /// use rusnix_ir::interop::Nixpkgs;
    /// let file = Nixpkgs::new().package_function("writeText")
    ///     .apply(["example.conf".into(), "workers=4\n".into()]);
    /// let contents = file.select("text"); // Still deferred; nothing is built.
    /// ```
    #[track_caller]
    pub fn apply(&self, arguments: impl IntoIterator<Item = NixValue>) -> NixValue {
        self.as_value().apply(arguments)
    }
}

/// Deferred named interpolation, using Nix toString rather than Rust formatting.
/// Builds a lazy [`NixValue`] string and preserves child provenance and Nix string
/// dependency context, including store-path dependencies. No deferred value is
/// evaluated or flattened into a Rust string.
///
/// Leading-newline templates remove that newline and a final indentation-only
/// closing line, then strip the common space/tab prefix of nonblank content lines.
/// Relative indentation, blank lines and the newline before the closing line remain;
/// tabs match tabs rather than visual columns. Other templates are verbatim.
/// Accepts `{name}`, `{{` and `}}`; interpolation never reindents supplied values.
/// Each named argument is constructed once; repeated holes reuse its graph.
/// Arguments must be named explicitly. Unknown, unused or duplicate names,
/// malformed braces and formatting specifiers produce compile-time errors.
///
/// ```
/// use rusnix_ir::{nix_text, nixos::OptionRef};
/// let port = OptionRef::<i64>::new("services.example.port").into_expr();
/// let command = nix_text!("postgres --port={port}", port = port);
/// ```
///
/// The original comma-separated fragment form remains available for dynamic
/// assembly. Its parts must already be strings; use to_text for coercion.
#[macro_export]
macro_rules! nix_text {
    ($template:literal $(, $name:ident = $value:expr)* $(,)?) => {
        $crate::__symbolic_text!($crate::interop::NixValue; $template $(, $name = $value)*)
    };
    ($($part:expr),* $(,)?) => {
        $crate::interop::NixValue::concat_text([$($crate::interop::NixValue::from($part)),*])
    };
}

/// Construct a deferred [`NixValue`] record from mixed Rust and Nix values.
/// Keys are literal strings or parenthesized Rust expressions, not attribute
/// paths: dots and punctuation remain within one key. Values use `NixValue::from`,
/// preserving symbolic expressions and opaque handles rather than stringifying them.
/// Duplicate or NUL-containing keys are rejected by IR validation.
///
/// ```
/// use rusnix_ir::{nix_record, nixos::OptionRef, interop::Nixpkgs};
/// let key = String::from("custom.setting");
/// let args = nix_record! {
///     "package": Nixpkgs::new().get("hello"),
///     "enabled": true,
///     (key): OptionRef::<i64>::new("services.example.port").into_expr(),
/// };
/// ```
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

/// Access pinned nixpkgs objects without generating Rust bindings for its schema.
/// Constructors perform no Nix evaluation. Package lookups and package functions
/// use either a standalone package set or NixOS's supplied set; [`Self::function`]
/// always selects from the pinned library.
#[derive(Clone, Debug, Default)]
pub struct Nixpkgs {
    module_scope: bool,
    overlays: Vec<Reference>,
}

impl Nixpkgs {
    /// Use the standalone pinned package set for deferred lookups and calls.
    /// The current backend imports it for `x86_64-linux` with empty nixpkgs config.
    /// Use [`Self::from_module`] to follow a NixOS module's platform/configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Use the package set supplied by NixOS, including its config and overlays.
    /// References require NixosModule lowering, like OptionRef dependencies.
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

    /// Append an opaque overlay; Nix composes it with preceding overlays.
    /// The standalone set is imported with overlays; a module-supplied set is extended.
    pub fn with_overlay(mut self, overlay: OverlayRef) -> Self {
        self.overlays.push(overlay.0);
        self
    }

    /// Reference a package by dotted path; existence is checked only in Nix.
    /// The Rust lookup location is retained for missing-attribute diagnostics.
    ///
    /// ```
    /// use rusnix_ir::interop::Nixpkgs;
    /// let pkgs = Nixpkgs::new();
    /// let git = pkgs.get("git");
    /// let requests = pkgs.get("python312Packages.requests");
    /// ```
    #[track_caller]
    pub fn get(&self, path: &str) -> PackageRef {
        self.path(AttrPath::dotted(path).0)
    }

    /// Reference a package using literal attribute segments, including names with dots.
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

    /// Reference an upstream file relative to pinned nixpkgs's `nixos/modules`.
    /// Nix checks module contents when imported; package-set overlays do not alter the file.
    #[track_caller]
    pub fn module(&self, file: &str) -> ModuleRef {
        ModuleRef(Reference {
            source: Source::ModuleFile { path: file.into() },
            path: None,
            origin: Origin::caller(format!("NixOS module lookup {file}")),
        })
    }

    /// Reference a dotted function in pinned `nixpkgs/lib`, without a Rust schema.
    /// For package-set builders such as `writeText`, use [`Self::package_function`].
    #[track_caller]
    pub fn function(&self, path: &str) -> NixFunction {
        NixFunction(Reference {
            source: Source::Library,
            path: Some(AttrPath::dotted(path)),
            origin: Origin::caller(format!("nixpkgs lib function lookup {path}")),
        })
    }

    /// Arbitrary package-set data, without claiming a package/function category.
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

    /// The complete opaque package set, for helpers accepting pkgs as an argument.
    /// This is ecosystem access, not introspection of the final NixOS configuration.
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

    /// Opaque data from pinned nixpkgs/lib, including real NixOS option types.
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

    /// A function in the package set (e.g. writeText), including its overlays.
    /// `function` separately addresses nixpkgs/lib. No schemas are inferred.
    #[track_caller]
    pub fn package_function(&self, path: &str) -> NixFunction {
        NixFunction(Reference {
            source: self.package_source(),
            path: Some(AttrPath::dotted(path)),
            origin: Origin::caller(format!("nixpkgs function lookup {path}")),
        })
    }
}

/// A named local Nix input supplying packages, modules, overlays or other objects.
/// This is an import boundary, not a fetching/flake API. Selecting values records
/// their Rust lookup origin; existence and category checks remain in Nix.
#[derive(Clone, Debug)]
pub struct InputRef {
    source: Source,
}

impl InputRef {
    /// Identify a local expression file without opening or evaluating it in Rust.
    /// `name` is a diagnostic identity; relative paths use Rust's working directory
    /// during lowering, so the file must remain available during Nix evaluation.
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

    /// Select a dotted package path; Nix validates existence and package contents.
    #[track_caller]
    pub fn package(&self, path: &str) -> PackageRef {
        PackageRef(self.lookup(path, "package"))
    }

    /// Select a deferred module for [`crate::nixos::NixosModule::import_ref`].
    #[track_caller]
    pub fn module(&self, path: &str) -> ModuleRef {
        ModuleRef(self.lookup(path, "module"))
    }

    /// Select a callable without describing its argument or result schema in Rust.
    #[track_caller]
    pub fn function(&self, path: &str) -> NixFunction {
        NixFunction(self.lookup(path, "function"))
    }

    /// Select an overlay whose composition is left to [`Nixpkgs::with_overlay`].
    #[track_caller]
    pub fn overlay(&self, path: &str) -> OverlayRef {
        OverlayRef(self.lookup(path, "overlay"))
    }

    /// Select arbitrary deferred input data without promising an object category.
    #[track_caller]
    pub fn value(&self, path: &str) -> NixValue {
        NixValue(self.lookup(path, "value").node())
    }
}
