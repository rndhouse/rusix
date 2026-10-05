//! Narrow extension API for structural data and native Rusnix leaves.
use crate::{Assignment, Config, ConfigValue, Node, Origin, ValidationError, ValueKind};

/// Convert a reusable value without deciding its global configuration placement.
/// Prefer the derive for mechanical struct/unit-enum mapping; implement this
/// trait when conversion carries domain meaning. The parent supplies placement.
/// Ordinary `Option<T>` converts `Some(value)` normally and `None` to Nix null.
/// Use the explicit `#[rusnix(omit_none)]` field or named-struct attribute to omit
/// absent definitions instead; it applies only to direct Option fields and is not inherited.
pub trait IntoRusnixValue {
    /// Consume this value into structural data or a native deferred leaf.
    /// Caller tracking propagates through tracked helpers and preserves child origins.
    #[track_caller]
    fn into_value(self) -> RusnixValue;
}

/// Structural lowering data used by derives and custom domain conversions.
/// Records nest into configuration paths; lists keep ordered values. Native
/// expressions and opaque objects remain deferred leaves, retaining their origins.
/// This is not a Nix source string or an evaluated value.
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

    /// Build named structural fields whose parent determines their configuration path.
    /// Unlike [`crate::interop::NixValue::record`], these records are flattened
    /// into rooted bindings by [`Config::from_value`]; keys remain literal segments.
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
    /// Derive implementation hook; `None` flattens a nested record into its parent.
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

    /// Keep this structural value atomic at the opaque Nix boundary instead of
    /// flattening it into configuration bindings. Useful for derived structs
    /// passed to Nix functions or wrapped in NixOS conditions and priorities.
    /// Child expressions retain their provenance and stay deferred; invalid
    /// structural flattening returns an error. Other IR checks run at compilation.
    pub fn into_nix_value(self) -> Result<crate::interop::NixValue, ValidationError> {
        self.resolve(&[])
            .map(opaque_record_nodes)
            .map(crate::interop::NixValue::from_node)
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

// Convert only structural containers; native deferred nodes stay untouched.
fn opaque_record_nodes(mut node: Node) -> Node {
    node.kind = match node.kind {
        ValueKind::AttrSet(fields) => ValueKind::OpaqueRecord(
            fields
                .into_iter()
                .map(|(name, value)| (name, opaque_record_nodes(value)))
                .collect(),
        ),
        ValueKind::List(items) => {
            ValueKind::List(items.into_iter().map(opaque_record_nodes).collect())
        }
        kind => kind,
    };

    node
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
    /// Lower a rooted structural record into this contribution's bindings.
    /// Shape errors are retained and returned by [`Self::validate`]. Native
    /// opaque records remain atomic values rather than additional option paths.
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
    f64,
    String,
    &str,
    crate::Expr<i64>,
    crate::Expr<bool>,
    crate::Expr<String>,
    crate::interop::PackageRef,
    crate::interop::NixValue
);

impl<T: IntoRusnixValue> IntoRusnixValue for Option<T> {
    #[track_caller]
    fn into_value(self) -> RusnixValue {
        match self {
            Some(value) => value.into_value(),
            None => RusnixValue::leaf(crate::interop::NixValue::null()),
        }
    }
}

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
