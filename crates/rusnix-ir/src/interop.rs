//! Category-typed references; Nix owns object existence and internal schemas.
use crate::{ConfigValue, Node, Origin, ValidationError, ValueKind, sealed};
use std::path::{Component, PathBuf};

#[derive(Clone, Debug)]
pub struct AttrPath(pub(crate) Vec<String>);

impl AttrPath {
    pub fn dotted(path: &str) -> Self {
        Self(path.split('.').map(str::to_owned).collect())
    }

    pub fn segments(parts: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self(parts.into_iter().map(Into::into).collect())
    }

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

#[derive(Clone, Debug)]
pub enum Source {
    Packages { overlays: Vec<Reference> },
    Library,
    NixosPackages { overlays: Vec<Reference> },
    ModuleFile { path: String },
    Input { name: String, file: PathBuf },
}

#[derive(Clone, Debug)]
pub struct Reference {
    pub source: Source,
    pub path: Option<AttrPath>,
    pub origin: Origin,
}

impl Reference {
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
    ($name:ident) => {
        #[derive(Clone, Debug)]
        pub struct $name(pub(crate) Reference);

        impl $name {
            pub fn reference(&self) -> &Reference {
                &self.0
            }

            pub fn as_value(&self) -> NixValue {
                NixValue(self.0.node())
            }
        }
    };
}

handle!(PackageRef);

handle!(ModuleRef);

handle!(NixFunction);

handle!(OverlayRef);

impl sealed::Sealed for PackageRef {}

impl ConfigValue for PackageRef {
    fn into_node(self, _: Origin) -> Node {
        self.0.node()
    }
}

/// Structured, opaque boundary data. References and symbolic expressions retain
/// their native IR meaning; Nix remains authoritative for function schemas.
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

    /// Only the chosen branch is demanded. Conditions remain authoritative in Nix.
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

    #[track_caller]
    pub fn equals(self, other: impl Into<Self>) -> Self {
        Self(Node {
            origin: Origin::caller("opaque Nix equality"),
            kind: ValueKind::Equal(Box::new(self.0), Box::new(other.into().0)),
        })
    }

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

    #[track_caller]
    pub fn null() -> Self {
        Self(Node {
            origin: Origin::caller("opaque Nix null"),
            kind: ValueKind::Null,
        })
    }

    /// Keys are literal attribute names, not dotted paths. Accepts BTreeMap
    /// directly; iterator order determines the deterministic generated order.
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
    #[track_caller]
    pub fn call(&self, argument: impl ConfigValue) -> NixValue {
        self.as_value().call(argument)
    }

    /// Apply any number of curried arguments through the opaque boundary.
    #[track_caller]
    pub fn apply(&self, arguments: impl IntoIterator<Item = NixValue>) -> NixValue {
        self.as_value().apply(arguments)
    }
}

/// Deferred named interpolation, using Nix toString rather than Rust formatting.
/// Templates preserve whitespace verbatim and accept `{name}`, `{{` and `}}`.
/// Each named argument is constructed once; repeated holes reuse its graph.
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

/// Structured mixed values with literal or parenthesized dynamic Rust keys.
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

#[derive(Clone, Debug, Default)]
pub struct Nixpkgs {
    module_scope: bool,
    overlays: Vec<Reference>,
}

impl Nixpkgs {
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

    pub fn with_overlay(mut self, overlay: OverlayRef) -> Self {
        self.overlays.push(overlay.0);
        self
    }

    #[track_caller]
    pub fn get(&self, path: &str) -> PackageRef {
        self.path(AttrPath::dotted(path).0)
    }

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

    #[track_caller]
    pub fn module(&self, file: &str) -> ModuleRef {
        ModuleRef(Reference {
            source: Source::ModuleFile { path: file.into() },
            path: None,
            origin: Origin::caller(format!("NixOS module lookup {file}")),
        })
    }

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

#[derive(Clone, Debug)]
pub struct InputRef {
    source: Source,
}

impl InputRef {
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

    #[track_caller]
    pub fn package(&self, path: &str) -> PackageRef {
        PackageRef(self.lookup(path, "package"))
    }

    #[track_caller]
    pub fn module(&self, path: &str) -> ModuleRef {
        ModuleRef(self.lookup(path, "module"))
    }

    #[track_caller]
    pub fn function(&self, path: &str) -> NixFunction {
        NixFunction(self.lookup(path, "function"))
    }

    #[track_caller]
    pub fn overlay(&self, path: &str) -> OverlayRef {
        OverlayRef(self.lookup(path, "overlay"))
    }

    #[track_caller]
    pub fn value(&self, path: &str) -> NixValue {
        NixValue(self.lookup(path, "value").node())
    }
}
