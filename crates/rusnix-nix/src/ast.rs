//! Advanced backend syntax for rendering and compiler diagnostics.
//!
//! This is deliberately separate from semantic configuration in `rusnix-ir`.
//! Constructors do not validate arbitrary ASTs; invalid syntax is a compiler
//! failure when evaluated. Ordinary configuration should not construct this tree.
use rusnix_ir::Origin;

/// A Nix syntax expression with optional source attribution and runtime context.
#[derive(Clone, Debug)]
pub struct NixExpr {
    /// Rust origin for generated comments and source-map entries, if available.
    pub origin: Option<Origin>,
    /// Runtime context only for operations that can fail when demanded.
    pub error_context: bool,
    /// Syntax to render; attribution does not alter its semantic operation.
    pub kind: NixKind,
}

/// Syntax supported by this backend; child ordering determines generated source.
#[derive(Clone, Debug)]
pub enum NixKind {
    /// A boolean literal.
    Bool(bool),
    /// A signed integer literal, including the minimum 64-bit value.
    Int(i64),
    /// A floating literal; finite representable values should be validated in IR.
    Float(f64),
    /// The Nix null literal.
    Null,
    /// Text rendered as an escaped Nix string, never as source syntax.
    String(String),
    /// Compiler-owned relative paths; never supplied by the frontend as syntax.
    Path(String),
    /// Ordered expressions in a lazy Nix list.
    List(Vec<NixExpr>),
    /// Explicit parentheses around a child expression.
    Group(Box<NixExpr>),
    /// Attribute bindings with literal path segments, escaped during rendering.
    AttrSet(Vec<(Vec<String>, NixExpr)>),
    /// A supported builtin with curried arguments in application order.
    Call(Builtin, Vec<NixExpr>),
    /// General function application to one argument.
    Apply(Box<NixExpr>, Box<NixExpr>),
    /// A binary operator applied to left and right operands.
    Binary(BinaryOp, Box<NixExpr>, Box<NixExpr>),
    /// Condition, then branch and else branch, with ordinary Nix laziness.
    If(Box<NixExpr>, Box<NixExpr>, Box<NixExpr>),
    /// One local binding name, its value and the body using it.
    Let(String, Box<NixExpr>, Box<NixExpr>),
    /// A compiler-selected identifier; the renderer does not escape or validate it.
    Variable(String),
    /// Select literal attribute segments from a deferred value.
    Select(Box<NixExpr>, Vec<String>),
    /// Lexical argument access; safe path names use identifiers, others are quoted.
    /// An empty path preserves the attributed lexical root without selecting a field.
    ArgumentSelect(Box<NixExpr>, Vec<String>),
    /// Attribute-pattern function with the compiler-owned argument names.
    Function(Vec<String>, Box<NixExpr>),
    /// One compiler-selected parameter name and its deferred function body.
    Lambda(String, Box<NixExpr>),
    /// Native named arguments, including lazy default expressions; extra names are rejected.
    ArgumentFunction(Vec<(String, Option<NixExpr>)>, Box<NixExpr>),
}

/// Nix builtins emitted by supported lowering operations.
#[derive(Clone, Copy, Debug)]
pub enum Builtin {
    /// Signed integer division via `builtins.div`.
    Div,
    /// Evaluator failure with a supplied message via `builtins.throw`.
    Throw,
    /// Import an existing Nix expression file.
    Import,
    /// Select an attribute by a string key, including literal punctuation.
    GetAttr,
    /// Convert a string into a Nix path value.
    ToPath,
    /// Coerce to a string while retaining Nix dependency context.
    ToString,
}

/// Native Nix binary operations used by this backend.
#[derive(Clone, Copy, Debug)]
pub enum BinaryOp {
    /// Native value equality (`==`).
    Equal,
    /// Inclusive lower-bound comparison (`>=`).
    GreaterEqual,
    /// Inclusive upper-bound comparison (`<=`).
    LessEqual,
    /// Short-circuit boolean conjunction (`&&`).
    And,
    /// Native addition, also used for string/path concatenation (`+`).
    Add,
}

impl NixExpr {
    /// Construct backend syntax without Rust source attribution or runtime context.
    pub fn plain(kind: NixKind) -> Self {
        Self {
            origin: None,
            error_context: false,
            kind,
        }
    }

    /// Attach source-map attribution without adding evaluation or forcing.
    pub fn attributed(kind: NixKind, origin: Origin) -> Self {
        Self {
            origin: Some(origin),
            error_context: false,
            kind,
        }
    }

    /// Attach attribution and `builtins.addErrorContext` around this operation.
    /// Children remain lazy; this does not force containers to retain context.
    pub fn contextual(kind: NixKind, origin: Origin) -> Self {
        Self {
            origin: Some(origin),
            error_context: true,
            kind,
        }
    }
}
