//! Semantic IR construction and inspection for compiler/backend implementations.
//!
//! Normal authors use [`crate::prelude`], typed expressions and conversion derives.
//! This public module supports the separate Nix backend crate; it is not an
//! authoring prelude or an interface for reading evaluated Nix values.
use crate::interop::raw::AttrPath;
use crate::{ValidationError, validate_scoped};
use serde::{Deserialize, Serialize};
use std::{
    panic::Location,
    path::{Component, PathBuf},
};

/// Lower a supported literal or symbolic value into the semantic IR.
/// Compiler implementations supply an origin for concrete leaves; symbolic
/// expressions retain their previously captured origins.
pub trait IntoNode {
    /// Consume the authoring value without evaluating its generated Nix expression.
    fn into_node(self, origin: Origin) -> Node;
}

/// Inspect the lookup metadata of an external reference handle.
/// This is compiler/provenance information, not the referenced Nix value.
pub trait ReferencedExpression {
    /// Obtain the source, literal attribute path and original Rust location.
    fn reference(&self) -> &Reference;
}

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
/// performing it. Normal authoring uses [`crate::Expr`], [`crate::IntoRusnixValue`] or
/// [`crate::interop::raw::NixValue`] instead of constructing nodes.
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
    Reference(Reference),
    /// One deferred application of a function to its argument.
    Apply(Box<Node>, Box<Node>),
    /// Native shallow attribute-set union; right-hand fields replace left-hand fields.
    AttrMerge(Box<Node>, Box<Node>),
    /// Native expression assertion; only a true condition permits demanding the result.
    Assert(Box<Node>, Box<Node>),
    /// A deferred lookup through literal attribute segments.
    Select(Box<Node>, AttrPath),
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
    OptionReference(AttrPath),
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

/// A setting with its destination path, value expression and Rust location.
/// This is inspection data within [`crate::Config`]. The backend turns it into a Nix
/// field assignment; its value is evaluated later by Nix.
#[derive(Clone, Debug)]
pub struct Assignment {
    /// Rust operation that introduced this definition, distinct from child expression origins.
    pub origin: Origin,
    /// Human-readable path; use [`Self::path_segments`] for unambiguous code generation.
    pub path: String,
    /// Deferred right-hand side of the definition.
    pub value: Node,
    pub(crate) segments: Vec<String>,
}

impl Assignment {
    /// Attribute segments are data; a renamed field may contain a literal dot.
    pub fn path_segments(&self) -> &[String] {
        &self.segments
    }
}

/// Where Nix should obtain a referenced package, function or other value.
/// This is inspection data for backend authors. Normal configuration code
/// chooses a source through [`crate::interop::Nixpkgs`] or [`crate::interop::InputRef`] instead.
#[derive(Clone, Debug)]
pub enum Source {
    /// The Nix language's own builtin namespace, independent of nixpkgs or caller libraries.
    Builtins,
    /// The pinned standalone package set, with Nix applying overlays in order.
    Packages {
        /// Referenced or Rust-authored overlay expressions, ordered as supplied by the author.
        overlays: Vec<Node>,
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
        /// Referenced or Rust-authored overlays applied to the supplied package set in order.
        overlays: Vec<Node>,
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
/// Normal authoring uses category handles such as [`crate::interop::PackageRef`]; this metadata
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
    /// Check lookup paths, inputs and overlay expressions without opening files or evaluating Nix.
    /// Overlay callback parameters must stay within their function scopes; to
    /// validate a reference that captures an enclosing parameter, validate the
    /// enclosing [`Node`] instead.
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.validate_scoped(&[])
    }

    pub(crate) fn validate_scoped(&self, scope: &[u64]) -> Result<(), ValidationError> {
        if let Some(path) = &self.path {
            path.validate(&self.origin)?;
        }

        match &self.source {
            Source::Packages { overlays } | Source::NixosPackages { overlays } => {
                for overlay in overlays {
                    validate_scoped(overlay, scope)?;
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

    pub(crate) fn node(&self) -> Node {
        Node {
            origin: self.origin.clone(),
            kind: ValueKind::Reference(self.clone()),
        }
    }
}
