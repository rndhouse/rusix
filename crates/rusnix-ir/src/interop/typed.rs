//! Typed handles retain authoring contracts while sharing the existing deferred IR.
use super::{
    NixLibrary, PackageRef,
    raw::{NixRepresentation, NixValue},
};
use crate::{
    ConfigValue, Expr, IntoRusnixValue, Node, Origin, RusnixValue, ValidationError, sealed,
};
use std::marker::PhantomData;

/// Typed lazy operations for a deferred expression.
///
/// Normal authoring retains the Rust interface through bindings, conditionals
/// and guards. Custom symbolic interfaces implement
/// [`NixRepresentation`](super::raw::NixRepresentation); raw representation
/// access requires importing that separate trait explicitly.
pub trait NixExpression: NixRepresentation + Clone + IntoRusnixValue {
    /// Share this expression through a lazy lexical parameter of the same type.
    /// Rust builds the callback once; Nix evaluates the application when demanded.
    #[track_caller]
    fn bind<R: NixExpression>(self, build: impl FnOnce(Self) -> R) -> R {
        R::from_expression(
            NixValue::function(|parameter| build(Self::from_expression(parameter)).as_expression())
                .call(self.as_expression()),
        )
    }

    /// Bind a structured record through this symbolic view, lowering at the boundary.
    #[track_caller]
    fn try_bind_record<R: NixExpression>(
        record: impl IntoRusnixValue,
        build: impl FnOnce(Self) -> R,
    ) -> Result<R, ValidationError> {
        Ok(Self::from_expression(record.try_into_nix_value()?).bind(build))
    }

    /// Choose between expressions with the same Rust interface, lazily in Nix.
    #[track_caller]
    fn choose(condition: impl Into<Expr<bool>>, yes: Self, no: Self) -> Self {
        Self::from_expression(NixValue::if_else(
            condition.into(),
            yes.as_expression(),
            no.as_expression(),
        ))
    }

    /// Validate lazily through the supplied library, retaining this interface.
    #[track_caller]
    fn require(
        self,
        lib: &NixLibrary,
        condition: impl Into<Expr<bool>>,
        message: impl Into<Expr<String>>,
    ) -> Self {
        lib.throw_if_not(condition, message, self)
    }

    /// Require a deferred condition, retaining the expression's Rust interface.
    #[track_caller]
    fn asserted(self, condition: impl Into<Expr<bool>>) -> Self {
        Self::from_expression(NixValue::assert(condition.into(), self.as_expression()))
    }
}

impl<T: NixRepresentation + Clone + IntoRusnixValue> NixExpression for T {}

impl NixRepresentation for NixValue {
    fn from_expression(value: NixValue) -> Self {
        value
    }

    fn as_expression(&self) -> NixValue {
        self.clone()
    }
}

macro_rules! scalar {
    ($ty:ty) => {
        impl NixRepresentation for Expr<$ty> {
            fn from_expression(value: NixValue) -> Self {
                value.into_expr()
            }

            fn as_expression(&self) -> NixValue {
                self.clone().into()
            }
        }

        impl From<$ty> for Expr<$ty> {
            #[track_caller]
            fn from(value: $ty) -> Self {
                NixValue::from(value).into_expr()
            }
        }
    };
}

scalar!(bool);

scalar!(String);

scalar!(i64);

impl From<&str> for Expr<String> {
    #[track_caller]
    fn from(value: &str) -> Self {
        NixValue::from(value).into_expr()
    }
}

impl Expr<String> {
    /// Combine deferred text without losing its string type or Nix string context.
    #[track_caller]
    pub fn concat(parts: impl IntoIterator<Item = Self>) -> Self {
        NixValue::concat_text(parts.into_iter().map(|part| part.as_expression())).into_expr()
    }
}

macro_rules! expression_handle {
    ($name:ident $(<$($parameter:ident),+>)?) => {
        impl$(<$($parameter: NixExpression),+>)? sealed::Sealed for $name$(<$($parameter),+>)? {}

        impl$(<$($parameter: NixExpression),+>)? ConfigValue for $name$(<$($parameter),+>)? {
            fn into_node(self, origin: Origin) -> Node {
                self.value.into_node(origin)
            }
        }

        impl$(<$($parameter: NixExpression),+>)? IntoRusnixValue for $name$(<$($parameter),+>)? {
            #[track_caller]
            fn into_value(self) -> RusnixValue {
                RusnixValue::leaf(self)
            }
        }

        impl$(<$($parameter: NixExpression),+>)? NixRepresentation for $name$(<$($parameter),+>)? {
            fn from_expression(value: NixValue) -> Self {
                Self { value, $(_type: PhantomData::<($($parameter,)+)>,)? }
            }

            fn as_expression(&self) -> NixValue {
                self.value.clone()
            }
        }

        impl$(<$($parameter: NixExpression),+>)? From<$name$(<$($parameter),+>)?> for NixValue {
            fn from(value: $name$(<$($parameter),+>)?) -> Self {
                value.value
            }
        }
    };
}

/// Convert a literal or symbolic value to its natural deferred interface.
/// Unlike IntoRusnixValue, this conversion retains its expression category.
pub trait IntoNixExpression {
    /// Symbolic interface associated with this value.
    type Expression: NixExpression;

    /// Construct the expression without evaluating Nix.
    #[track_caller]
    fn into_expression(self) -> Self::Expression;
}

impl<T: NixExpression> IntoNixExpression for T {
    type Expression = T;

    fn into_expression(self) -> T {
        self
    }
}

macro_rules! expression_literal {
    ($ty:ty, $expression:ty) => {
        impl IntoNixExpression for $ty {
            type Expression = $expression;

            #[track_caller]
            fn into_expression(self) -> Self::Expression {
                self.into()
            }
        }
    };
}

expression_literal!(bool, Expr<bool>);

expression_literal!(i64, Expr<i64>);

expression_literal!(String, Expr<String>);

expression_literal!(&str, Expr<String>);

expression_literal!(PackageRef, Package);

/// An expression expected to expose nixpkgs' argument override interface.
/// This capability applies to instantiated packages and overridable callables.
/// External values are checked by Nix only when the override is demanded.
pub trait NixOverridable: NixExpression {
    /// Call the value's real override method, retaining its original interface.
    #[track_caller]
    fn override_arguments(&self, arguments: impl ConfigValue) -> Self {
        Self::from_expression(
            self.as_expression()
                .override_args(NixValue::literal(arguments)),
        )
    }
}

/// An instantiated package expression, independent of its authoring language.
/// External values are expectations; this handle does not prove buildability or ABI.
#[derive(Clone, Debug)]
pub struct Package {
    value: NixValue,
}

expression_handle!(Package);

impl From<PackageRef> for Package {
    fn from(reference: PackageRef) -> Self {
        Self::from_expression(reference.as_value())
    }
}

impl NixOverridable for Package {}

impl Package {
    /// Select a named output as a package value; Nix checks whether it exists.
    #[track_caller]
    pub fn output(&self, name: &str) -> Self {
        Self::from_expression(self.value.clone().select_segments([name]))
    }

    /// Select a package-specific field with an explicit expected expression type.
    #[track_caller]
    pub fn field<T: NixExpression>(&self, name: &str) -> T {
        T::from_expression(self.value.clone().select(name))
    }

    /// Use real nixpkgs argument overrides, preserving this package interface.
    #[track_caller]
    pub fn override_arguments(&self, arguments: impl ConfigValue) -> Self {
        NixOverridable::override_arguments(self, arguments)
    }

    /// Lower a structured partial override only at its call boundary.
    #[track_caller]
    pub fn try_override_arguments(
        &self,
        arguments: impl IntoRusnixValue,
    ) -> Result<Self, ValidationError> {
        Ok(self.override_arguments(arguments.try_into_nix_value()?))
    }

    /// Use real overrideAttrs with a lazy typed view of the previous attributes.
    #[track_caller]
    pub fn override_attrs(&self, update: impl FnOnce(NixAttrs) -> NixAttrs) -> Self {
        Self::from_expression(
            self.value
                .clone()
                .override_attrs(NixCallable::from_function(update)),
        )
    }

    /// Extend top-level package fields using ordinary shallow Nix attribute union.
    /// This retains existing override methods; the fields are not derivation overrides
    /// and do not automatically propagate to other outputs or later overrides.
    #[track_caller]
    pub fn extend(&self, attributes: NixAttrs) -> Self {
        Self::from_expression(self.value.clone().merge_attrs(attributes))
    }
}

/// A symbolic interface expected to expose nixpkgs' argument override method.
///
/// Packages already have this contract. Wrap other external expressions explicitly
/// when they are known to be overridable, such as nixpkgs fetcher callables. Plain
/// constructed functions do not implement the override capability automatically.
/// This expectation does not inspect the Nix expression or force its attributes.
#[derive(Clone, Debug)]
pub struct Overridable<T: NixExpression> {
    value: NixValue,
    _type: PhantomData<(T,)>,
}

expression_handle!(Overridable<T>);

impl<T: NixExpression> NixOverridable for Overridable<T> {}

impl<T: NixExpression> Overridable<T> {
    /// Access the wrapped symbolic interface without discarding its result type.
    pub fn inner(&self) -> T {
        T::from_expression(self.value.clone())
    }

    /// Extend an overridable attribute set using ordinary shallow Nix union.
    /// Callable attribute sets retain their __functor and override methods.
    #[track_caller]
    pub fn extend(&self, attributes: NixAttrs) -> Self {
        Self::from_expression(self.value.clone().merge_attrs(attributes))
    }
}

impl<R: NixExpression, A: NixExpression> Overridable<NixCallable<R, A>> {
    /// Apply the wrapped callable, retaining its declared result interface.
    #[track_caller]
    pub fn call(&self, argument: impl Into<A>) -> R {
        self.inner().call(argument)
    }
}

impl<R: NixExpression> Overridable<NixCallable<R>> {
    /// Lower a structured argument only at this call boundary.
    #[track_caller]
    pub fn try_call(&self, argument: impl IntoRusnixValue) -> Result<R, ValidationError> {
        self.inner().try_call(argument)
    }
}

/// A deferred callable with result and parameter interfaces.
/// `A = NixValue` explicitly retains dynamic inputs for flexible external schemas.
/// Constructed callbacks retain their parameter type, including through binding.
/// Unlike NixFunction, this can represent constructed callbacks and lexical parameters.
#[derive(Clone, Debug)]
pub struct NixCallable<R: NixExpression = NixValue, A: NixExpression = NixValue> {
    value: NixValue,
    _type: PhantomData<(R, A)>,
}

expression_handle!(NixCallable<R, A>);

impl<R: NixExpression, A: NixExpression> NixCallable<R, A> {
    /// Construct a callback whose parameter and result preserve their interfaces.
    #[track_caller]
    pub fn from_function(build: impl FnOnce(A) -> R) -> Self {
        Self::from_expression(NixValue::function(|parameter| {
            build(A::from_expression(parameter)).as_expression()
        }))
    }

    /// Construct a callback returning a structured Rust value.
    /// Conversion happens inside this boundary; an invalid flatten returns an error.
    /// R is the expected symbolic result interface, not a check of external Nix types.
    #[track_caller]
    pub fn try_from_function<V: IntoRusnixValue>(
        build: impl FnOnce(A) -> V,
    ) -> Result<Self, ValidationError> {
        let mut error = None;
        let function = NixValue::function(|parameter| {
            match build(A::from_expression(parameter)).try_into_nix_value() {
                Ok(body) => body,
                Err(failure) => {
                    error = Some(failure);
                    NixValue::null()
                }
            }
        });
        match error {
            Some(error) => Err(error),
            None => Ok(Self::from_expression(function)),
        }
    }

    /// Describe a call; lowering happens inside this operation.
    #[track_caller]
    pub fn call(&self, argument: impl Into<A>) -> R {
        R::from_expression(self.value.clone().call(argument.into().as_expression()))
    }
}

impl<R: NixExpression> NixCallable<R> {
    /// Accept a user-defined record without requiring caller-side type erasure.
    /// Invalid structural flattening is returned as a Rust validation error.
    #[track_caller]
    pub fn try_call(&self, argument: impl IntoRusnixValue) -> Result<R, ValidationError> {
        Ok(self.call(argument.try_into_nix_value()?))
    }
}

/// A deferred attribute set with a known member expression interface.
/// NixValue members permit heterogeneous records; Package members express families.
#[derive(Clone, Debug)]
pub struct NixAttrs<T: NixExpression = NixValue> {
    value: NixValue,
    _type: PhantomData<(T,)>,
}

expression_handle!(NixAttrs<T>);

impl<T: NixExpression> NixAttrs<T> {
    /// Construct a record without evaluating its members.
    #[track_caller]
    pub fn new(fields: impl IntoIterator<Item = (impl Into<String>, T)>) -> Self {
        Self::from_expression(NixValue::record(
            fields
                .into_iter()
                .map(|(key, value)| (key, value.as_expression())),
        ))
    }

    /// Select one literal member name, retaining its expression type.
    #[track_caller]
    pub fn get(&self, name: &str) -> T {
        T::from_expression(self.value.clone().select_segments([name]))
    }

    /// Test attribute presence, including an attribute whose value is null.
    #[track_caller]
    pub fn has(&self, name: &str) -> Expr<bool> {
        self.value.clone().has_attr(name).into_expr()
    }

    /// Use the fallback only when the attribute is absent, not when it is null.
    #[track_caller]
    pub fn get_or(&self, name: &str, fallback: impl Into<T>) -> T {
        T::from_expression(
            self.value
                .clone()
                .attr_or(name, fallback.into().as_expression()),
        )
    }

    /// Return null for an absent attribute, retaining the member interface otherwise.
    /// Use has to distinguish an absent attribute from a present null value.
    #[track_caller]
    pub fn get_optional(&self, name: &str) -> NixNullable<T> {
        NixNullable::from_expression(self.value.clone().attr_or(name, NixValue::null()))
    }

    /// Shallow union; values on the right take precedence and remain lazy.
    #[track_caller]
    pub fn merge(self, right: Self) -> Self {
        Self::from_expression(self.value.merge_attrs(right))
    }
}

impl NixAttrs {
    /// Convert a structured Rust record at an attribute-set operation boundary.
    /// This checks Rust structural conversion, not the eventual Nix value type.
    #[track_caller]
    pub fn try_from_record(record: impl IntoRusnixValue) -> Result<Self, ValidationError> {
        Ok(Self::from_expression(record.try_into_nix_value()?))
    }

    /// Access a heterogeneous member through an explicit expected interface.
    #[track_caller]
    pub fn field<T: NixExpression>(&self, name: &str) -> T {
        T::from_expression(self.get(name))
    }
}

/// A lazy Nix list with a known element expression interface, not a Rust Vec.
#[derive(Clone, Debug)]
pub struct NixList<T: NixExpression> {
    value: NixValue,
    _type: PhantomData<(T,)>,
}

expression_handle!(NixList<T>);

impl<T: NixExpression> NixList<T> {
    /// Construct a list, retaining each element's expression and provenance.
    #[track_caller]
    pub fn new(items: impl IntoIterator<Item = T>) -> Self {
        Self::from_expression(NixValue::list(
            items.into_iter().map(|item| item.as_expression()),
        ))
    }

    /// Builtin concatenation, preserving element order and lazy values.
    #[track_caller]
    pub fn concat(lists: impl IntoIterator<Item = Self>) -> Self {
        Self::from_expression(NixValue::concat_lists(
            lists.into_iter().map(|list| list.as_expression()),
        ))
    }

    /// Concatenate through the exact supplied library while retaining element types.
    #[track_caller]
    pub fn concat_with(lib: &NixLibrary, lists: impl IntoIterator<Item = Self>) -> Self {
        lib.concat_lists(lists)
    }

    /// Call the supplied library's optional helper without erasing the element type.
    #[track_caller]
    pub fn optional(lib: &NixLibrary, condition: impl Into<Expr<bool>>, value: T) -> Self {
        lib.optional(condition, value)
    }

    /// Call the supplied library's optionals helper; excluded elements stay unforced.
    #[track_caller]
    pub fn when(self, lib: &NixLibrary, condition: impl Into<Expr<bool>>) -> Self {
        lib.optionals(condition, self)
    }
}

/// A deferred Nix path, distinct from a string spelling a pathname or a package.
/// External wrapping states a path expectation; it performs no file access.
#[derive(Clone, Debug)]
pub struct NixPath {
    value: NixValue,
}

expression_handle!(NixPath);

impl super::ToNixText for NixPath {
    #[track_caller]
    fn to_nix_text(self) -> Expr<String> {
        self.value.to_text().into_expr()
    }
}

/// A deferred value that Nix may evaluate to null or to T.
/// Rust Option chooses a variant during authoring; this interface defers that choice.
#[derive(Clone, Debug)]
pub struct NixNullable<T: NixExpression> {
    value: NixValue,
    _type: PhantomData<(T,)>,
}

expression_handle!(NixNullable<T>);

impl<T: NixExpression> NixNullable<T> {
    /// Describe null without evaluating a potential value.
    #[track_caller]
    pub fn null() -> Self {
        Self::from_expression(NixValue::null())
    }

    /// Retain a value's expression and provenance as the present alternative.
    #[track_caller]
    pub fn some(value: impl Into<T>) -> Self {
        Self::from_expression(value.into().as_expression())
    }

    /// Test for null in Nix when demanded.
    #[track_caller]
    pub fn is_null(&self) -> Expr<bool> {
        self.value.clone().equals(NixValue::null()).into_expr()
    }

    /// Select a fallback only for null; the unused branch remains unforced.
    #[track_caller]
    pub fn unwrap_or(self, fallback: impl Into<T>) -> T {
        self.bind(|value| {
            T::choose(
                value.is_null(),
                fallback.into(),
                T::from_expression(value.value),
            )
        })
    }

    /// Transform only the non-null alternative, preserving null lazily.
    #[track_caller]
    pub fn map<U: NixExpression>(self, transform: impl FnOnce(T) -> U) -> NixNullable<U> {
        self.bind(|value| {
            NixNullable::choose(
                value.is_null(),
                NixNullable::null(),
                NixNullable::some(transform(T::from_expression(value.value))),
            )
        })
    }
}

/// The supplied standard build environment, retaining real nixpkgs builders.
/// Platform schemas remain finite caller-defined views or explicit interop.
#[derive(Clone, Debug)]
pub struct Stdenv {
    value: NixValue,
}

expression_handle!(Stdenv);

impl Stdenv {
    /// Describe mkDerivation, accepting existing supported expression values.
    #[track_caller]
    pub fn mk_derivation(&self, attributes: impl ConfigValue) -> Package {
        self.builder().call(NixValue::literal(attributes))
    }

    /// Accept a structured recipe directly, lowering it inside the builder call.
    #[track_caller]
    pub fn try_mk_derivation(
        &self,
        attributes: impl IntoRusnixValue,
    ) -> Result<Package, ValidationError> {
        self.builder().try_call(attributes)
    }

    /// Obtain the actual supplied builder, including its finalAttrs support.
    #[track_caller]
    pub fn builder(&self) -> NixCallable<Package> {
        NixCallable::from_expression(self.value.clone().select("mkDerivation"))
    }

    /// Retain a finite view of the complete caller-supplied host platform.
    #[track_caller]
    pub fn host_platform(&self) -> super::Platform {
        super::Platform::from_expression(self.value.clone().select("hostPlatform"))
    }

    /// Retain the caller-supplied build platform without projecting an identity.
    #[track_caller]
    pub fn build_platform(&self) -> super::Platform {
        super::Platform::from_expression(self.value.clone().select("buildPlatform"))
    }

    /// Retain the caller-supplied target platform for compiler/toolchain policy.
    #[track_caller]
    pub fn target_platform(&self) -> super::Platform {
        super::Platform::from_expression(self.value.clone().select("targetPlatform"))
    }

    /// Compare complete build/host values without reconstructing platform records.
    #[track_caller]
    pub fn build_host_equal(&self) -> Expr<bool> {
        crate::package::build_host_equal(self.value.clone())
    }
}

impl NixRepresentation for NixLibrary {
    fn from_expression(value: NixValue) -> Self {
        Self::from_value(value)
    }

    fn as_expression(&self) -> NixValue {
        self.value.clone()
    }
}

impl IntoRusnixValue for NixLibrary {
    #[track_caller]
    fn into_value(self) -> RusnixValue {
        RusnixValue::leaf(self.as_expression())
    }
}

impl sealed::Sealed for NixLibrary {}

impl ConfigValue for NixLibrary {
    fn into_node(self, origin: Origin) -> Node {
        self.as_expression().into_node(origin)
    }
}

impl Expr<String> {
    /// Call optionalString on the exact supplied library, retaining text context.
    #[track_caller]
    pub fn when(self, lib: &NixLibrary, condition: impl Into<Expr<bool>>) -> Self {
        lib.optional_text(condition.into(), self)
    }
}

impl From<NixLibrary> for NixValue {
    fn from(value: NixLibrary) -> Self {
        value.as_expression()
    }
}
