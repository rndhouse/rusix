//! Call common functions from nixpkgs’ utility library without evaluating them in Rust.
// Promote named helpers from demonstrated real-world usage; keep arbitrary lib
// access on the existing NixValue escape hatch rather than mirroring nixpkgs.
use super::{NixExpression, NixValue, replacement_lists};
use crate::Expr;

/// A Rust handle for nixpkgs’ utility library, `lib`.
///
/// `lib` is nixpkgs’ utility library: a collection of named values, called an
/// *attribute set*, containing functions such as `lib.optional`, `lib.concatLists`
/// and `lib.throwIfNot`. It is separate from the Nix language’s `builtins`
/// namespace, which provides functions such as `builtins.map` and `builtins.getAttr`.
/// This wrapper provides named Rust helpers for a small supported subset of `lib`.
///
/// The methods construct expressions for Nix to evaluate later; Rust does not
/// read the library or execute its functions. Nix checks the arguments’ actual
/// types. Text results preserve the package dependencies attached to Nix strings.
///
/// If a package caller supplies `lib`, wrap that value with [`Self::from_value`].
/// All methods use that exact library, including any functions the caller replaces.
/// [`super::Nixpkgs::library`] instead selects Rusnix’s pinned nixpkgs library.
/// Functions without a named helper remain available through [`Self::as_value`]
/// using the generic [`NixValue`] selection and application methods.
///
/// ```
/// use rusnix_ir::interop::Nixpkgs;
/// let lib = Nixpkgs::new().library();
/// let dependencies = lib.optional(true, Nixpkgs::new().get("curl"));
/// // Represents lib.optional true pkgs.curl: Nix will produce a one-element list.
/// ```
#[derive(Clone, Debug)]
pub struct NixLibrary {
    /// The deferred Nix value representing the nixpkgs `lib` attribute set.
    /// Methods look up and call utility functions on this exact value; Rust does
    /// not evaluate its contents.
    value: NixValue,
}

impl NixLibrary {
    /// Wrap a deferred Nix value representing nixpkgs `lib`.
    /// Methods call functions from this exact library value. Rust does not evaluate
    /// or check its contents; [`super::Nixpkgs::library`] selects the pinned library.
    pub fn from_value(value: NixValue) -> Self {
        Self { value }
    }

    /// Access the deferred nixpkgs `lib` value wrapped by this handle.
    ///
    /// Use this escape hatch for functions without a supported helper. Clone the
    /// value, then use [`NixValue::select`] and [`NixValue::apply`] to describe the
    /// call. Rust does not read the library or execute the function.
    ///
    /// ```
    /// use rusnix_ir::interop::Nixpkgs;
    /// let lib = Nixpkgs::new().library();
    /// let headers = lib.as_value().clone().select("getDev")
    ///     .apply([Nixpkgs::new().get("curl").into()]);
    /// // Represents lib.getDev pkgs.curl, evaluated later by Nix.
    /// ```
    pub fn as_value(&self) -> &NixValue {
        &self.value
    }

    // Common dispatch for supported helpers; track_caller preserves their callers.
    #[track_caller]
    fn apply(&self, name: &str, arguments: impl IntoIterator<Item = NixValue>) -> NixValue {
        self.value.clone().select(name).apply(arguments)
    }

    /// Return `value` when `condition` is true, otherwise throw `message` in Nix.
    /// This represents `lib.throwIfNot condition message value`.
    ///
    /// Rust only constructs the expression; Nix evaluates the condition later.
    /// The standard nixpkgs function does not evaluate `value` when the condition
    /// is false, or `message` when it is true. This calls the wrapped library’s
    /// function, so a caller’s replacement remains authoritative.
    ///
    /// This validates an expression when it is evaluated. It does not contribute
    /// to NixOS’s assertion collection; use [`crate::nixos::assertion`] for that.
    #[track_caller]
    pub fn throw_if_not(
        &self,
        condition: impl Into<NixValue>,
        message: impl Into<NixValue>,
        value: impl Into<NixValue>,
    ) -> NixValue {
        self.apply(
            "throwIfNot",
            [condition.into(), message.into(), value.into()],
        )
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
    ) -> Expr<String> {
        self.apply("optionalString", [condition.into(), text.into()])
            .into_expr()
    }

    /// Construct a Nix expression that tests whether every condition is true.
    /// This calls `lib.all` with a function that returns each condition unchanged.
    /// The standard function returns true for an empty list and stops at the first
    /// false condition. Rust builds the expression; Nix checks boolean values.
    #[track_caller]
    pub fn all(&self, conditions: impl IntoIterator<Item = impl Into<NixValue>>) -> Expr<bool> {
        self.apply(
            "all",
            [
                NixValue::function(|value| value),
                NixValue::list(conditions.into_iter().map(Into::into)),
            ],
        )
        .into_expr()
    }

    /// Join several Nix lists into one, preserving their element order.
    /// For example, `[[1], [2, 3]]` becomes `[1, 2, 3]` through `lib.concatLists`.
    /// The standard function returns an empty list for no inputs and evaluates
    /// element values only when they are needed.
    #[track_caller]
    pub fn concat_lists(&self, lists: impl IntoIterator<Item = NixValue>) -> NixValue {
        self.apply("concatLists", [NixValue::list(lists)])
    }

    /// Compare deferred version strings using this exact caller-supplied library.
    #[track_caller]
    pub fn version_at_least(
        &self,
        version: impl Into<Expr<String>>,
        minimum: impl Into<Expr<String>>,
    ) -> Expr<bool> {
        self.apply(
            "versionAtLeast",
            [version.into().into(), minimum.into().into()],
        )
        .into_expr()
    }

    /// Select the development output using the caller's normal getDev fallback.
    #[track_caller]
    pub fn get_dev(&self, package: super::Package) -> super::Package {
        super::Package::from_expression(self.apply("getDev", [package.into()]))
    }

    /// Select the library output using the caller's normal getLib fallback.
    #[track_caller]
    pub fn get_lib(&self, package: super::Package) -> super::Package {
        super::Package::from_expression(self.apply("getLib", [package.into()]))
    }

    /// Test whether `version` precedes `other` through the supplied `lib.versionOlder`.
    /// Rust constructs the comparison; the library compares strings when Nix evaluates it.
    #[track_caller]
    pub fn version_older(
        &self,
        version: impl Into<NixValue>,
        other: impl Into<NixValue>,
    ) -> Expr<bool> {
        self.apply("versionOlder", [version.into(), other.into()])
            .into_expr()
    }

    /// Replace text using ordered `(from, to)` pairs through the supplied `lib.replaceStrings`.
    /// Rust constructs aligned lists; the library performs the replacement in Nix.
    /// Use [`NixValue::replace_text`] for builtin replacement independent of `lib`.
    #[track_caller]
    pub fn replace_text(
        &self,
        text: impl Into<NixValue>,
        replacements: impl IntoIterator<Item = (impl Into<NixValue>, impl Into<NixValue>)>,
    ) -> NixValue {
        let [from, to] = replacement_lists(replacements);
        self.apply("replaceStrings", [from, to, text.into()])
    }
}
