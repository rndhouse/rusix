//! Narrow extension API for structural data and native Rusnix leaves.
use crate::{Assignment, Config, ConfigValue, Node, Origin, ValidationError, ValueKind};

/// Convert a reusable value without deciding its global configuration placement.
pub trait IntoRusnixValue {
    #[track_caller]
    fn into_value(self) -> RusnixValue;
}

/// Structured lowering data. Nodes and backend syntax stay behind this boundary.
#[derive(Clone, Debug)]
pub struct RusnixValue {
    origin: Origin,
    kind: Kind,
}

#[derive(Clone, Debug)]
enum Kind {
    Leaf(Box<Node>),
    Record(Vec<(Option<String>, RusnixValue)>),
    List(Vec<RusnixValue>),
}

impl RusnixValue {
    /// Preserve expressions/opaque objects; primitive leaves acquire placed origins.
    #[track_caller]
    pub fn leaf(value: impl ConfigValue) -> Self {
        let origin = Origin::caller("configuration value");
        Self {
            kind: Kind::Leaf(Box::new(value.into_node(origin.clone()))),
            origin,
        }
    }

    #[track_caller]
    pub fn record(fields: impl IntoIterator<Item = (impl Into<String>, Self)>) -> Self {
        Self {
            origin: Origin::caller("configuration record"),
            kind: Kind::Record(
                fields
                    .into_iter()
                    .map(|(name, value)| (Some(name.into()), value))
                    .collect(),
            ),
        }
    }

    // None represents flatten. Normal extensions use record; derive emits this.
    #[doc(hidden)]
    #[track_caller]
    pub fn __record(fields: Vec<(Option<&str>, Self)>) -> Self {
        Self {
            origin: Origin::caller("configuration record"),
            kind: Kind::Record(
                fields
                    .into_iter()
                    .map(|(name, value)| (name.map(str::to_owned), value))
                    .collect(),
            ),
        }
    }

    fn resolve(self, path: &[String]) -> Result<Node, ValidationError> {
        let placed = |purpose: &str| {
            Origin::new(
                &self.origin.file,
                self.origin.line,
                self.origin.column,
                format!("{purpose} {}", display_path(path)),
            )
        };
        let (origin, kind) = match self.kind {
            Kind::Leaf(mut node) => {
                if node.origin == self.origin {
                    node.origin = placed("value of");
                }
                return Ok(*node);
            }
            Kind::List(items) => {
                let mut nodes = Vec::new();
                for (index, item) in items.into_iter().enumerate() {
                    let mut item_path = path.to_vec();
                    item_path.push(format!("[{index}]"));
                    nodes.push(item.resolve(&item_path)?);
                }
                (placed("list at"), ValueKind::List(nodes))
            }
            Kind::Record(fields) => {
                let mut nodes = Vec::new();
                for (name, value) in fields {
                    if let Some(name) = name {
                        let mut child_path = path.to_vec();
                        child_path.push(name.clone());
                        nodes.push((name, value.resolve(&child_path)?));
                    } else {
                        let flattened = value.resolve(path)?;
                        let ValueKind::AttrSet(fields) = flattened.kind else {
                            return Err(ValidationError {
                                origin: flattened.origin,
                                message: "rusnix flatten requires a record value".into(),
                            });
                        };
                        nodes.extend(fields);
                    }
                }
                (placed("record at"), ValueKind::AttrSet(nodes))
            }
        };
        Ok(Node { origin, kind })
    }
}

fn display_path(path: &[String]) -> String {
    path.iter()
        .map(|part| {
            if part.contains('.') || part.contains('"') {
                format!("{part:?}")
            } else {
                part.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(".")
}

impl Config {
    /// Lower a rooted record. Shape errors are reported by normal IR validation.
    #[track_caller]
    pub fn from_value(value: RusnixValue) -> Self {
        let mut config = Self::new();
        match value.resolve(&[]) {
            Ok(node) if matches!(node.kind, ValueKind::AttrSet(_)) => {
                flatten(&mut config, node, Vec::new());
            }
            Ok(node) => {
                config.error = Some(ValidationError {
                    origin: node.origin,
                    message: "a configuration contribution must be a rooted record".into(),
                })
            }
            Err(error) => config.error = Some(error),
        }
        config
    }
}

fn flatten(config: &mut Config, node: Node, path: Vec<String>) {
    if matches!(&node.kind, ValueKind::AttrSet(fields) if !fields.is_empty() || path.is_empty()) {
        let ValueKind::AttrSet(fields) = node.kind else {
            unreachable!()
        };
        for (name, child) in fields {
            let mut child_path = path.clone();
            child_path.push(name);
            flatten(config, child, child_path);
        }
    } else {
        let path_text = display_path(&path);
        config.assignments.push(Assignment {
            origin: Origin::new(
                &config.origin.file,
                config.origin.line,
                config.origin.column,
                format!("set {path_text}"),
            ),
            path: path_text,
            segments: path,
            value: node,
        });
    }
}

macro_rules! leaf {
    ($($ty:ty),* $(,)?) => { $(
        impl IntoRusnixValue for $ty {
            #[track_caller]
            fn into_value(self) -> RusnixValue { RusnixValue::leaf(self) }
        }
    )* };
}

leaf!(
    bool,
    i32,
    i64,
    u16,
    String,
    &str,
    crate::Expr<i64>,
    crate::Expr<bool>,
    crate::Expr<String>,
    crate::interop::PackageRef,
    crate::interop::NixValue
);

impl<T: IntoRusnixValue> IntoRusnixValue for Vec<T> {
    #[track_caller]
    fn into_value(self) -> RusnixValue {
        // A function-pointer/closure adapter would stop tracked caller forwarding.
        let mut items = Vec::new();
        for value in self {
            items.push(value.into_value());
        }
        RusnixValue {
            origin: Origin::caller("configuration list"),
            kind: Kind::List(items),
        }
    }
}

macro_rules! opaque {
    ($($ty:ty),* $(,)?) => { $(
        impl IntoRusnixValue for $ty {
            #[track_caller]
            fn into_value(self) -> RusnixValue { RusnixValue::leaf(self.as_value()) }
        }
    )* };
}

opaque!(
    crate::interop::ModuleRef,
    crate::interop::NixFunction,
    crate::interop::OverlayRef
);

macro_rules! option_ref {
    ($($ty:ty),* $(,)?) => { $(
        impl IntoRusnixValue for crate::nixos::OptionRef<$ty> {
            #[track_caller]
            fn into_value(self) -> RusnixValue { RusnixValue::leaf(self.into_expr()) }
        }
    )* };
}

option_ref!(i64, bool, String);
