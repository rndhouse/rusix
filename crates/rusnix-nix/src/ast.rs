//! Backend syntax, deliberately separate from the public Rust configuration API.
use rusnix_ir::Origin;

#[derive(Clone, Debug)]
pub struct NixExpr {
    pub origin: Option<Origin>,
    /// Runtime context only for operations that can fail when demanded.
    pub error_context: bool,
    pub kind: NixKind,
}

#[derive(Clone, Debug)]
pub enum NixKind {
    Bool(bool),
    Int(i64),
    Float(f64),
    Null,
    String(String),
    /// Compiler-owned relative paths; never supplied by the frontend as syntax.
    Path(String),
    List(Vec<NixExpr>),
    Group(Box<NixExpr>),
    AttrSet(Vec<(Vec<String>, NixExpr)>),
    Call(Builtin, Vec<NixExpr>),
    Apply(Box<NixExpr>, Box<NixExpr>),
    Binary(BinaryOp, Box<NixExpr>, Box<NixExpr>),
    If(Box<NixExpr>, Box<NixExpr>, Box<NixExpr>),
    Let(String, Box<NixExpr>, Box<NixExpr>),
    Variable(String),
    Select(Box<NixExpr>, Vec<String>),
    /// Attribute-pattern function with the compiler-owned argument names.
    Function(Vec<String>, Box<NixExpr>),
}

#[derive(Clone, Copy, Debug)]
pub enum Builtin {
    Div,
    Throw,
    Import,
    GetAttr,
    ToPath,
    ToString,
}

#[derive(Clone, Copy, Debug)]
pub enum BinaryOp {
    GreaterEqual,
    LessEqual,
    And,
    Add,
}

impl NixExpr {
    pub fn plain(kind: NixKind) -> Self {
        Self {
            origin: None,
            error_context: false,
            kind,
        }
    }

    pub fn attributed(kind: NixKind, origin: Origin) -> Self {
        Self {
            origin: Some(origin),
            error_context: false,
            kind,
        }
    }

    pub fn contextual(kind: NixKind, origin: Origin) -> Self {
        Self {
            origin: Some(origin),
            error_context: true,
            kind,
        }
    }
}
