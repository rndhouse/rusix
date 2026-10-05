//! Explicit NixOS composition, priorities, imports and assertion boundaries.
use crate::interop::AttrPath;
use crate::interop::{ModuleRef, PackageRef};
use crate::{Config, ConfigValue, Expr, IntoConfig, Node, Origin, ValueKind};
use std::marker::PhantomData;

/// An explicit dependency on a final merged NixOS option.
///
/// `T` is the author's expected type, not a verified option schema. Supported
/// expression types are `i64`, `bool`, and `String`; NixOS checks actual values.
/// This handle has no read/resolve operation. It requires NixOS module lowering.
#[derive(Clone, Debug)]
pub struct OptionRef<T> {
    path: AttrPath,
    origin: Origin,
    ty: PhantomData<T>,
}

impl<T> OptionRef<T> {
    #[track_caller]
    pub fn new(path: &str) -> Self {
        Self {
            path: AttrPath::dotted(path),
            origin: Origin::caller(format!("NixOS option reference {path}")),
            ty: PhantomData,
        }
    }

    pub fn into_expr(self) -> Expr<T> {
        Expr::new(ValueKind::OptionReference(self.path), self.origin)
    }
}

#[derive(Clone, Debug)]
pub struct Import {
    pub path: String,
    pub origin: Origin,
}

#[derive(Clone, Debug)]
pub struct Assertion {
    pub name: String,
    pub condition: Node,
    pub message: String,
    pub origin: Origin,
}

#[derive(Clone, Debug)]
pub struct NixosModule {
    pub config: Config,
    pub imports: Vec<Import>,
    pub assertions: Vec<Assertion>,
    pub modules: Vec<NixosModule>,
    pub priority: DefinitionPriority,
    pub opaque_imports: Vec<(ModuleRef, Origin)>,
}

/// Priority of this module's option assignments; NixOS performs filtering and merging.
#[derive(Clone, Copy, Debug, Default)]
pub enum DefinitionPriority {
    #[default]
    Normal,
    Default,
    Force,
    Override(u16),
}

impl DefinitionPriority {
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
    #[track_caller]
    pub fn empty() -> Self {
        Self::new(Config::new())
    }

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
    #[track_caller]
    // Component addition is an authoring operation; no arithmetic/+ API is intended.
    #[allow(clippy::should_implement_trait)]
    pub fn add<T: IntoConfig>(self, value: T) -> Self {
        self.module(Self::new(value.into_config()))
    }

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
    ///
    /// ```compile_fail
    #[doc = include_str!("../../../tests/ui/module-as-package.rs")]
    /// ```
    #[track_caller]
    pub fn system_packages(mut self, packages: Vec<PackageRef>) -> Self {
        self.config = self.config.set("environment.systemPackages", packages);
        self
    }

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
