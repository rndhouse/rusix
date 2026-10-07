# Typed deferred package authoring

Rusnix preserves useful Rust interfaces until operations lower them into the
existing semantic IR. These handles describe computations for Nix; constructing
or binding them does not evaluate Nix, fetch sources, or build packages.

- `Package` represents an instantiated package, whether authored in Rust or Nix.
- `PackageFunction<R>` describes named package arguments and a result interface.
  The default result remains `NixValue`; families can return `NixAttrs<Package>`.
- `NixCallable<R, A>` retains both result and parameter interfaces for an external,
  constructed, or bound callable. Its default `A = NixValue` explicitly permits
  dynamic inputs; structured `try_call` is available only on that interface.
  `NixFunction::signature` attaches a known external contract.
- `NixAttrs<T>` and `NixList<T>` preserve member and element interfaces.
- `Overridable<T>` explicitly adds the expected argument-override capability to
  external callables. Plain `NixCallable<R>` does not implement `NixOverridable`.
  `Package` and `Overridable<T>` implement that trait, retaining their interfaces.
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
record views can survive binding. `try_bind_record` converts a structural Rust
record at that boundary and reports conversion errors before constructing its
callback. `as_attrs()` retains the subtree for shallow union without erasing it
to `NixValue`. Root views still restrict whole-root access.

External expressions enter typed code through the explicit
`NixExpression::from_expression` expectation boundary. This does not prove their
actual Nix types, package output names, ABI, or override capabilities. Nix checks
those properties when demanded. Dynamic fields and unusual package behavior
remain accessible through `as_expression`; original child provenance remains in
that expression. Arbitrary `NixValue` interop remains supported.

Typed package overrides call real nixpkgs `override` and `overrideAttrs` methods.
Output selection does not stringify packages. Text expressions keep Nix string
context; list and attribute-set operations keep excluded dependencies lazy.

For example, a family can share a typed factory through a lexical parameter:

```rust
let family = factory.bind(|factory| {
    NixAttrs::new([
        ("default", pkgs.call_package(&factory, default_arguments)),
        ("variant", pkgs.call_package(&factory, variant_arguments)),
    ])
});
```

The callback's factory retains `PackageFunction<Package>` and the family retains
`NixAttrs<Package>`. MariaDB's complete release-family implementation also uses
thin named-argument forwarders so real callPackage override behavior survives.

The OpenSSL, curl, Git and MariaDB examples retain package dependencies and source
results as `Package`, phase text as `Expr<String>`, and main dependency/flag lists
with their element interfaces. Git/curl argument records and MariaDB release
arguments stay as Rust structs until `try_call_package`. MariaDB's shared common
attributes stay a derived Rust record until a finite-view binding. The composed
graph takes/returns typed packages and shares each rewritten dependency once.
These migrations add no package-specific core types or builder replacement.

`NixList::concat` describes native concatenation; `concat_with` dispatches through
the supplied library's `concatLists`. `optional` and `when` also use the exact
supplied library. Keeping this distinction preserves caller overrides and upstream
recipes. `NixLibrary::get_dev` and `get_lib` retain packages and real nixpkgs output
fallbacks; `version_at_least`, `version_older`, `all` and `optional_text` retain
scalar results. Typed overrides and native list concatenation delegate to the
shared core operations, preserving their behavior and diagnostic boundaries.

Raw interop remains useful inside dynamic selectors, mixed metadata and test
scopes, patch/path values, backend platform records and curried helpers whose full
interfaces are unspecified. Deferred family member names, output availability,
function schemas and override methods still receive their authoritative checks in
Nix. The types describe authoring contracts rather than proving external schemas.

Across the four `inputs.rs` views, explicit `NixValue` leaves changed as follows:

| Example | Before | After |
| --- | ---: | ---: |
| OpenSSL | 13 | 4 |
| curl | 39 | 5 |
| Git | 57 | 8 |
| MariaDB | 47 | 2 |
| Total | 156 | 19 |

These counts include root arguments and nested view fields. Remaining dynamic
leaves primarily describe mixed metadata, test/package scopes and path-like values.

Verification after integration with the shared core helpers passed 496 workspace
tests and 23 doctests. The
UI harness checks 67 compile-fail fixtures, including five new typed-interface
rejections. Twelve new evaluator tests cover typed bindings, record conversion,
package overrides, library dispatch/output fallbacks, excluded dependencies,
string context and child diagnostics. The existing package suites also pass:
OpenSSL 8, curl 23, Git 44, MariaDB 8 and composition 16 tests. Those suites include
many configurations per test and compare exact derivation recipes and identities.

Formatting, structural spacing, Clippy with warnings denied, strict rustdoc, the
complete fixture workflow and every example passed. Generated graph checks retain
one definition for each shared rewritten dependency and prohibit broad `deepSeq`
forcing. All Nix evaluation uses fresh disposable isolated stores against the
unchanged repository nixpkgs pin; no packages were built or fetched.

Shared library operations retain their contracts throughout calls: boolean
conditions, string messages and version inputs, typed optional lists and
concatenation, and string replacement results. `IntoNixExpression` associates
literals with their natural symbolic interfaces while symbolic values keep their
existing interfaces. Thus `lib.optional(true, 42_i64)` returns
`NixList<Expr<i64>>`; `throw_if_not` preserves the guarded interface. External
values still require explicit expectation boundaries.
