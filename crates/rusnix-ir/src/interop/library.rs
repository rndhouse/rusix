//! Call common functions from nixpkgs’ utility library without evaluating them in Rust.
use super::NixValue;

/// A Rust handle for nixpkgs’ utility library, `lib`.
///
/// In Nix, an *attribute set* is a collection of named values, like a Rust map or
/// record. nixpkgs supplies a `lib` attribute set containing utility functions
/// such as `optional` and `concatLists`. This wrapper provides Rust methods for
/// calling a small selection of those functions.
///
/// The methods construct expressions for Nix to evaluate later; Rust does not
/// read the library or execute its functions. Nix checks the arguments’ actual
/// types. Text results preserve the package dependencies attached to Nix strings.
///
/// If a package caller supplies `lib`, wrap that value with [`Self::from_value`].
/// All methods use that exact library, including any functions the caller replaces.
/// [`super::Nixpkgs::library`] instead selects Rusnix’s pinned nixpkgs library.
///
/// ```
/// use rusnix_ir::interop::Nixpkgs;
/// let lib = Nixpkgs::new().library();
/// let dependencies = lib.optional(true, Nixpkgs::new().get("curl"));
/// // Represents lib.optional true pkgs.curl: Nix will produce a one-element list.
/// ```
#[derive(Clone, Debug)]
pub struct NixLibrary {
    /// The Nix expression representing the nixpkgs `lib` attribute set.
    /// Methods select and call utility functions from this value.
    value: NixValue,
}

impl NixLibrary {
    /// Wrap a Nix value representing the nixpkgs `lib` attribute set.
    /// Rust does not evaluate or check its contents. Use this for a package caller’s
    /// library; use [`super::Nixpkgs::library`] to select the pinned library.
    pub fn from_value(value: NixValue) -> Self {
        Self { value }
    }

    /// Look up a utility function and pass its arguments in order.
    /// For example, `apply("optional", [condition, value])` represents
    /// `lib.optional condition value`. Nix functions commonly take one argument at a
    /// time; this repeated application is called *currying*.
    ///
    /// Dots in `name` select nested fields. For a single field containing a literal
    /// dot, use [`NixValue::select_segments`]. Nix checks arguments and result types.
    #[track_caller]
    pub fn apply(&self, name: &str, arguments: impl IntoIterator<Item = NixValue>) -> NixValue {
        self.value.clone().select(name).apply(arguments)
    }

    /// Return a Nix expression for a one-element list when `condition` is true,
    /// or an empty list when false: `lib.optional condition value`.
    /// The standard nixpkgs function does not evaluate `value` in the false branch.
    #[track_caller]
    pub fn optional(&self, condition: impl Into<NixValue>, value: impl Into<NixValue>) -> NixValue {
        self.apply("optional", [condition.into(), value.into()])
    }

    /// Return the supplied Nix list when `condition` is true, or an empty list
    /// when false: `lib.optionals condition values`.
    /// This keeps the list’s elements rather than adding a nesting level. The
    /// standard function leaves the false branch unevaluated; Nix checks the types.
    #[track_caller]
    pub fn optionals(
        &self,
        condition: impl Into<NixValue>,
        values: impl Into<NixValue>,
    ) -> NixValue {
        self.apply("optionals", [condition.into(), values.into()])
    }

    /// Return the supplied text when `condition` is true, or empty text when
    /// false: `lib.optionalString condition text`.
    /// The standard function leaves the false branch unevaluated. The selected text
    /// keeps its bytes and package dependencies; values are not converted to text.
    #[track_caller]
    pub fn optional_text(
        &self,
        condition: impl Into<NixValue>,
        text: impl Into<NixValue>,
    ) -> NixValue {
        self.apply("optionalString", [condition.into(), text.into()])
    }

    /// Construct a Nix expression that tests whether every condition is true.
    /// This calls `lib.all` with a function that returns each condition unchanged.
    /// The standard function returns true for an empty list and stops at the first
    /// false condition. Rust builds the expression; Nix checks boolean values.
    #[track_caller]
    pub fn all(&self, conditions: impl IntoIterator<Item = impl Into<NixValue>>) -> NixValue {
        self.apply(
            "all",
            [
                NixValue::function(|value| value),
                NixValue::list(conditions.into_iter().map(Into::into)),
            ],
        )
    }

    /// Join several Nix lists into one, preserving their element order.
    /// For example, `[[1], [2, 3]]` becomes `[1, 2, 3]` through `lib.concatLists`.
    /// The standard function returns an empty list for no inputs and evaluates
    /// element values only when they are needed.
    #[track_caller]
    pub fn concat_lists(&self, lists: impl IntoIterator<Item = NixValue>) -> NixValue {
        self.apply("concatLists", [NixValue::list(lists)])
    }
}
