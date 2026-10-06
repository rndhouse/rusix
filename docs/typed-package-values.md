# Typed deferred package authoring

Rusnix preserves useful Rust interfaces until operations lower them into the
existing semantic IR. These handles describe computations for Nix; constructing
or binding them does not evaluate Nix, fetch sources, or build packages.

- `Package` represents an instantiated package, whether authored in Rust or Nix.
- `PackageFunction<R>` describes named package arguments and a result interface.
  The default result remains `NixValue`; families can return `NixAttrs<Package>`.
- `NixCallable<R>` represents an external, constructed, or bound callable.
- `NixAttrs<T>` and `NixList<T>` preserve member and element interfaces.
- `Stdenv` retains the supplied builder and exact build/host comparison.
- `Expr<bool>`, `Expr<String>`, and `Expr<i64>` retain scalar expectations.

Core handles implement `ConfigValue` for direct placement and `IntoRusnixValue`
for nesting in user-defined records. `ConfigValue` remains sealed. User-defined
records use `IntoRusnixValue`; `try_call`, `try_call_package`, and
`try_mk_derivation` return structural conversion errors at the receiving boundary.
Explicit conversion to `NixValue` is not required before those calls.

`NixExpression` describes symbolic interfaces that can survive a lexical binding.
Its `bind` method provides a parameter with the original Rust interface and
preserves the callback result interface. It lowers to the existing lazy Nix
function application, sharing the dependency expression once. `choose`,
`asserted`, and `require` retain that interface through conditionals and guards.
There are no new evaluator operations or eager checks.

Argument views accept core handles as leaves. A subtree marked `#[rusnix(value)]`
in an argument view also implements `NixExpression`, so finite caller-defined
record views can survive binding. Root views still restrict whole-root access.

External expressions enter typed code through the explicit
`NixExpression::from_expression` expectation boundary. This does not prove their
actual Nix types, package output names, ABI, or override capabilities. Nix checks
those properties when demanded. Dynamic fields and unusual package behavior
remain accessible through `as_expression`; original child provenance remains in
that expression. Arbitrary `NixValue` interop remains supported.

Typed package overrides call real nixpkgs `override` and `overrideAttrs` methods.
Output selection does not stringify packages. Text expressions keep Nix string
context; list and attribute-set operations keep excluded dependencies lazy.
