//! Compose independent contributions using ordinary NixOS merge and priority semantics.
//!
//! [`NixosModule`] describes definitions and imports; it does not run NixOS.
//! [`OptionRef`] declares finite dependencies on the final merged configuration.
//! NixOS owns option schemas, merging and actual value types.
use crate::interop::AttrPath;
use crate::interop::{ModuleRef, NixValue, Nixpkgs, PackageRef};
use crate::{Config, ConfigValue, Expr, IntoConfig, Node, Origin, ValueKind};
use std::marker::PhantomData;

/// Combine deferred definition trees using NixOS mkMerge, not Rust merging.
#[track_caller]
pub fn merge(values: impl IntoIterator<Item = NixValue>) -> NixValue {
    Nixpkgs::new()
        .function("mkMerge")
        .call(NixValue::list(values))
}

impl NixValue {
    /// NixOS mkIf retains a deferred definition until module processing.
    /// Unlike if_else, this constructs module metadata, not a selected value.
    #[track_caller]
    pub fn when(self, condition: impl Into<Self>) -> Self {
        Nixpkgs::new()
            .function("mkIf")
            .apply([condition.into(), self])
    }

    /// Definition priority is selected by NixOS, including for symbolic values.
    /// This is the per-value counterpart of NixosModule::priority.
    #[track_caller]
    pub fn priority(self, priority: DefinitionPriority) -> Self {
        let library = Nixpkgs::new();
        match priority {
            DefinitionPriority::Normal => self,
            DefinitionPriority::Default => library.function("mkDefault").call(self),
            DefinitionPriority::Force => library.function("mkForce").call(self),
            DefinitionPriority::Override(value) => {
                library.function("mkOverride").apply([value.into(), self])
            }
        }
    }

    /// Order this definition before ordinary list/lines definitions (mkBefore).
    #[track_caller]
    pub fn before(self) -> Self {
        Nixpkgs::new().function("mkBefore").call(self)
    }

    /// Order this definition after ordinary list/lines definitions (mkAfter).
    #[track_caller]
    pub fn after(self) -> Self {
        Nixpkgs::new().function("mkAfter").call(self)
    }
}

/// An explicit dependency on a final merged NixOS option.
///
/// `T` is the author's expected type, not a verified option schema. Supported
/// expression types are `i64`, `bool`, and `String`; NixOS checks actual values.
/// This handle has no read/resolve operation. It requires NixOS module lowering.
/// Origins are captured when constructing the reference, not when converting it.
/// [`crate::options`] generates equivalent tracked accessors for finite option trees.
///
/// ```
/// use rusnix_ir::nixos::OptionRef;
/// let port = OptionRef::<i64>::new("services.example.port");
/// let command = port.into_expr().to_text().with_prefix("example --port=");
/// // NixOS resolves the port after merging; Rust cannot read it here.
/// ```
#[derive(Clone, Debug)]
pub struct OptionRef<T> {
    path: AttrPath,
    origin: Origin,
    ty: PhantomData<T>,
}

impl<T> OptionRef<T> {
    /// Declare a dotted dependency without reading its value or checking its schema.
    /// For a literal dot within an attribute name, use [`Self::from_segments`].
    #[track_caller]
    pub fn new(path: &str) -> Self {
        Self::from_segments(path.split('.'))
    }

    /// Literal attribute segments; dots and special characters stay within a key.
    #[track_caller]
    pub fn from_segments(parts: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let path = AttrPath::segments(parts);
        Self {
            origin: Origin::caller(format!("NixOS option reference {}", path.parts().join("."))),
            path,
            ty: PhantomData,
        }
    }

    /// Explicitly cross into the opaque boundary, including collection options.
    /// This does not expose the referenced value to Rust.
    pub fn into_value(self) -> NixValue {
        NixValue::from_node(Node {
            origin: self.origin,
            kind: ValueKind::OptionReference(self.path),
        })
    }

    /// Preserve the expected Rust type and original provenance in a deferred expression.
    /// Only `Expr<bool>`, `Expr<i64>` and `Expr<String>` currently support lowering
    /// as configuration values; use [`Self::into_value`] for opaque shapes.
    pub fn into_expr(self) -> Expr<T> {
        Expr::new(ValueKind::OptionReference(self.path), self.origin)
    }
}

/// A pinned-tree module import with the Rust operation that introduced it.
/// Normally constructed by [`NixosModule::import`].
#[derive(Clone, Debug)]
pub struct Import {
    /// Relative file under the evaluator's nixpkgs root; absolute/parent paths are rejected.
    pub path: String,
    /// Rust import boundary used when failures cross into the imported module.
    pub origin: Origin,
}

/// A deferred NixOS assertion with a Rust-origin marker in its failure message.
/// Normally constructed by [`NixosModule::assertion`].
#[derive(Clone, Debug)]
pub struct Assertion {
    /// Diagnostic identity of this assertion, not a declared NixOS option name.
    pub name: String,
    /// Deferred boolean condition checked when assertion evaluation is requested.
    pub condition: Node,
    /// Human-readable reason for a false condition, retained alongside provenance.
    pub message: String,
    /// Rust operation that introduced the assertion.
    pub origin: Origin,
}

/// An explicit NixOS boundary grouping independent definitions, imports and assertions.
/// Each [`Self::add`] keeps its own contribution rather than flattening bindings
/// into one [`Config`]. NixOS performs merging and priority selection; symbolic
/// dependencies follow final values supplied by Rust or ordinary Nix modules.
/// Creating this value does not evaluate options, build packages or activate a system.
#[derive(Clone, Debug)]
pub struct NixosModule {
    /// Bindings owned directly by this module, subject to its priority.
    pub config: Config,
    /// Pinned-tree imports, each retaining its introducing Rust operation.
    pub imports: Vec<Import>,
    /// Assertion contributions, merged into NixOS's ordinary assertion list.
    pub assertions: Vec<Assertion>,
    /// Independent child modules; each validates and retains its own priority.
    pub modules: Vec<NixosModule>,
    /// Override policy for this module's direct bindings, not its children or imports.
    pub priority: DefinitionPriority,
    /// Opaque imported modules paired with the Rust import call's provenance.
    pub opaque_imports: Vec<(ModuleRef, Origin)>,
}

/// Priority of this module's option assignments; NixOS performs filtering and merging.
#[derive(Clone, Copy, Debug, Default)]
pub enum DefinitionPriority {
    /// Ordinary definitions, left unwrapped (NixOS's normal override priority is 100).
    #[default]
    Normal,
    /// `mkDefault` (1000): ordinary definitions take precedence over these defaults.
    Default,
    /// `mkForce` (50): wins over ordinary/default definitions, subject to lower overrides.
    Force,
    /// `mkOverride` with an explicit priority; lower numbers win in NixOS.
    Override(u16),
}

impl DefinitionPriority {
    /// Numeric wrapper priority used by lowering; `None` leaves ordinary definitions unwrapped.
    pub fn override_priority(self) -> Option<u16> {
        match self {
            Self::Normal => None,
            Self::Default => Some(1000),
            Self::Force => Some(50),
            Self::Override(priority) => Some(priority),
        }
    }
}

impl NixosModule {
    /// Start a composition with no direct bindings, imports, assertions or children.
    #[track_caller]
    pub fn empty() -> Self {
        Self::new(Config::new())
    }

    /// Wrap one existing binding contribution without changing its origins.
    pub fn new(config: Config) -> Self {
        Self {
            config,
            imports: vec![],
            assertions: vec![],
            modules: vec![],
            priority: DefinitionPriority::Normal,
            opaque_imports: vec![],
        }
    }

    /// Add one independent contribution. NixOS performs merging and priority
    /// filtering across children; duplicate bindings within one Config still
    /// fail IR validation. To set a contribution's priority explicitly, use
    /// `module(NixosModule::new(value.into_config()).priority(...))`.
    /// Conversion happens now in Rust; the contribution's values and NixOS merge
    /// remain deferred. Caller tracking forwards the authoring location to adapters.
    ///
    /// ```
    /// use rusnix_ir::{self as rusnix, nixos::NixosModule};
    /// #[rusnix::config]
    /// mod configuration {
    ///     #[rusnix(root)]
    ///     pub struct Layer { pub environment: Environment }
    ///
    ///     pub struct Environment { pub paths_to_link: Vec<String> }
    /// }
    /// use configuration::{Environment, Layer};
    /// let module = NixosModule::empty()
    ///     .add(Layer { environment: Environment { paths_to_link: vec!["/share".into()] } })
    ///     .add(Layer { environment: Environment { paths_to_link: vec!["/lib".into()] } });
    /// // NixOS merges both list definitions; Rusnix does not choose a winner.
    /// ```
    #[track_caller]
    // Component addition is an authoring operation; no arithmetic/+ API is intended.
    #[allow(clippy::should_implement_trait)]
    pub fn add<T: IntoConfig>(self, value: T) -> Self {
        self.module(Self::new(value.into_config()))
    }

    /// Add an already assembled child, preserving its imports, assertions and priority.
    pub fn module(mut self, module: NixosModule) -> Self {
        self.modules.push(module);
        self
    }

    /// Applies to this module's Config assignments. Children have their own
    /// policies; assertion messages retain their normal list merge semantics.
    pub fn priority(mut self, priority: DefinitionPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Import a path relative to the evaluator's pinned nixpkgs source tree.
    /// For general ecosystem objects use `import_ref(ModuleRef)`. Paths are
    /// validated during lowering; Nix remains authoritative for module contents.
    #[track_caller]
    pub fn import(mut self, path: impl Into<String>) -> Self {
        let path = path.into();
        self.imports.push(Import {
            origin: Origin::caller(format!("import {path}")),
            path,
        });
        self
    }

    /// Import an existing opaque module and record this call as a Rust boundary.
    /// Nix remains authoritative for its contents and interactions with other definitions.
    #[track_caller]
    pub fn import_ref(mut self, module: ModuleRef) -> Self {
        let origin = Origin::caller(format!(
            "import opaque module {}",
            module.reference().origin.purpose
        ));
        self.opaque_imports.push((module, origin));
        self
    }

    /// Only category-typed PackageRef values cross this boundary. NixOS checks
    /// that each resolved value really is a package.
    /// Adds a direct `environment.systemPackages` binding; repeated use within
    /// one module conflicts in IR validation. Compose separate contributions to merge lists.
    ///
    /// ```compile_fail
    #[doc = include_str!("../../../tests/ui/module-as-package.rs")]
    /// ```
    #[track_caller]
    pub fn system_packages(mut self, packages: Vec<PackageRef>) -> Self {
        self.config = self.config.set("environment.systemPackages", packages);
        self
    }

    /// Add a deferred assertion whose message identifies this Rust operation.
    /// The condition is not resolved in Rust. The evaluator decides when to
    /// demand assertions; ordinary NixOS activation/build checks remain in NixOS.
    #[track_caller]
    pub fn assertion(
        mut self,
        name: impl Into<String>,
        condition: Expr<bool>,
        message: impl Into<String>,
    ) -> Self {
        let name = name.into();
        let origin = Origin::caller(format!("assertion assertions.{name}"));
        self.assertions.push(Assertion {
            name,
            condition: condition.into_node(origin.clone()),
            message: message.into(),
            origin,
        });
        self
    }
}
