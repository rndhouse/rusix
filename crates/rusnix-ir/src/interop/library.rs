//! A small vocabulary of calls through an explicitly selected Nix library.
use super::NixValue;

/// An opaque Nix library record, usually the `lib` supplied by a package caller.
/// Helpers use exactly this record: overridden functions remain authoritative.
/// Nothing is evaluated in Rust, and construction does not verify a library schema.
/// Lookups and calls retain provenance; selected text retains Nix string context.
///
/// ```
/// use rusnix_ir::interop::{NixLibrary, NixValue};
/// let factory = NixValue::function_attrs(["lib", "enabled"], |args| {
///     let lib = NixLibrary::from_value(args.clone().select("lib"));
///     let enabled = args.select("enabled");
///     (Vec::<(&str, NixValue)>::new(), lib.optional(enabled, "dependency"))
/// });
/// // The Nix caller supplies lib; Rust does not select a separate package scope.
/// ```
#[derive(Clone, Debug)]
pub struct NixLibrary {
    /// The supplied record; standard helpers never replace it with another library.
    value: NixValue,
}

impl NixLibrary {
    /// Bind helpers to this deferred record without reading or validating its contents.
    /// For the pinned library, use [`super::Nixpkgs::library`].
    pub fn from_value(value: NixValue) -> Self {
        Self { value }
    }

    /// Select a dotted library function and apply curried arguments in order.
    /// Nix owns the function schema and result type; use raw NixValue selections
    /// for literal function names containing dots or other unusual paths.
    #[track_caller]
    pub fn apply(&self, name: &str, arguments: impl IntoIterator<Item = NixValue>) -> NixValue {
        self.value.clone().select(name).apply(arguments)
    }

    /// Call `lib.optional`: one element when true, otherwise an empty list.
    /// With the standard library, a false condition leaves the element unforced.
    #[track_caller]
    pub fn optional(&self, condition: impl Into<NixValue>, value: impl Into<NixValue>) -> NixValue {
        self.apply("optional", [condition.into(), value.into()])
    }

    /// Call `lib.optionals`: the supplied list when true, otherwise an empty list.
    /// Unlike optional, this does not add a nesting level. Standard false branches
    /// do not force the list; actual list/boolean types are checked by Nix.
    #[track_caller]
    pub fn optionals(
        &self,
        condition: impl Into<NixValue>,
        values: impl Into<NixValue>,
    ) -> NixValue {
        self.apply("optionals", [condition.into(), values.into()])
    }

    /// Call `lib.optionalString`: the supplied text when true, otherwise empty text.
    /// Standard false branches leave text unforced; selected text keeps its exact
    /// bytes and store dependencies. No implicit text coercion is added.
    #[track_caller]
    pub fn optional_text(
        &self,
        condition: impl Into<NixValue>,
        text: impl Into<NixValue>,
    ) -> NixValue {
        self.apply("optionalString", [condition.into(), text.into()])
    }

    /// Call `lib.all` with an identity predicate over these deferred booleans.
    /// The standard library returns true for an empty list and stops at the first
    /// false condition. Rust only constructs the list; Nix checks actual types.
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

    /// Call `lib.concatLists` to flatten one level of deferred lists in order.
    /// The standard library returns an empty list for no inputs and leaves element
    /// values unforced until consumed. Nix remains authoritative for list types.
    #[track_caller]
    pub fn concat_lists(&self, lists: impl IntoIterator<Item = NixValue>) -> NixValue {
        self.apply("concatLists", [NixValue::list(lists)])
    }
}
