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
- `Platform` and `FinalAttrs` provide shared finite record views.
- `NixNullable<T>` distinguishes a deferred null-or-value choice from Rust Option.
- `NixPath` retains actual path expressions until explicit text coercion.
- `IntoNixExpression` and `ToNixText` preserve interfaces at literal and text boundaries.
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

The earlier package-interface migration changed explicit `NixValue` leaves in
the four `inputs.rs` views as follows:

| Example | Before | After |
| --- | ---: | ---: |
| OpenSSL | 13 | 4 |
| curl | 39 | 5 |
| Git | 57 | 8 |
| MariaDB | 47 | 2 |
| Total | 156 | 19 |

These counts include root arguments and nested view fields. Remaining dynamic
leaves primarily describe mixed metadata, test/package scopes and path-like values.

Verification with these stronger interfaces passed 505 workspace tests and
23 doctests. The UI harness checks 74 compile-fail fixtures, including seven new
rejections for callable parameters, bypass attempts, library conditions, text
coercion, nullable fallbacks, paths and external-view contracts. Nineteen typed
evaluator tests cover bindings, record conversion, overrides, caller library
dispatch, excluded dependencies, string context, nullable values, paths and child
diagnostics. NixOS tests also cover reused record views and nullable final options. The existing package suites also pass:
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

`nix_text!` returns `Expr<String>` directly. Named interpolation uses `ToNixText`
for demonstrated Nix-coercible interfaces, retaining dependency contexts and
child origins. Packages remain packages until interpolation; attribute sets and
callables have no implicit text capability. `NixValue` remains an explicit
dynamic escape hatch. Fragment concatenation accepts string expressions, and
`Expr<String>::replace_text` retains that interface through native replacement.

`NixNullable<T>` represents a deferred null-or-value choice. Its `map` and
`unwrap_or` keep excluded branches lazy and retain the contained interface.
`NixAttrs::has` tests presence even for null values; `get_or` defaults only for
missing attributes, while `get_optional` produces null for absence. These semantics
are separate from Rust `Option` and its authoring-time omission policy.

`Nixpkgs::source_path` returns `NixPath`. OpenSSL and MariaDB retain paths and
`NixList<NixPath>` through their patch policy; Git's local patch helpers retain
paths until combination with fetched derivations requires heterogeneous interop.
OpenSSL's configuration argument is `NixNullable<NixValue>` because upstream
accepts different text-coercible file representations. No new filesystem operation
or eager type validation is introduced.

External finite record views and aliases can be declared with
`#[rusnix(expression)]` fields in both argument and option views. The declared
type must implement `NixExpression`; the macro does not inspect its schema.
This allows shared views to live in separate modules without duplicating their
fields or falling back to NixValue.

`Platform` and `FinalAttrs` are finite shared views authored with the existing
argument-view macro. Platform fields are limited to properties used by the four
real packages. They retain complete platform records, including undeclared
fields; `build_host_equal` still compares those complete values. Stdenv exposes
build, host and target views. All four package definitions use the shared platform
view; OpenSSL, curl and Git use `FinalAttrs` for lazy version/final-package access.
Ordinary attribute interop remains available for other fields. Nested selections
may retain parentheses around their lexical source; no renderer optimization is
needed to preserve sharing, laziness, or derivation recipes.

The callable result parameter remains first for compatibility with existing
`NixCallable<R>` external references. Constructed functions infer their parameter
interface; explicit annotations include it as `NixCallable<R, A>`. Flexible Nix
argument records retain the dynamic default. Code using a template or path at an
explicit raw boundary converts there with `.into()` rather than erasing earlier.

Final checks also passed `cargo fmt --all --check`, structural spacing over 177
Rust files, all-target Clippy with warnings denied, strict rustdoc, the complete
fixture workflow and all 15 examples. The PostgreSQL full suite and all 99 Git,
curl, OpenSSL, MariaDB and composed-graph tests pass. Verification artifacts are
retained under `target/typed-operations-*.log`. The nixpkgs pin remains
`8b27c1239e5c421a2bbc2c65d52e4a6fbf2ff296`; all Nix evaluations use fresh isolated
local stores, with no package builds or network fetches.

## Authoring and raw interop

`rusnix_ir::prelude` exports typed authoring, configuration macros and conversion
traits. It excludes `NixValue` and representation access. Import
`interop::raw::{NixValue, NixRepresentation}` only when an adapter needs dynamic
Nix operations or an unchecked external interface expectation.

`NixExpression` supplies lazy `bind`, `choose`, `require`, `asserted` and finite
record binding. Implement `raw::NixRepresentation` for custom symbolic interfaces;
those implementing `Clone` and `IntoRusnixValue` receive `NixExpression` automatically.
The generated argument views follow the same rule. Their raw `as_value` hooks
remain available for interoperability but are hidden from normal rustdoc listings.

Factories themselves implement `ConfigValue`; pass them directly into records,
`Config::set` and callPackage. Factory `.as_value()` has been removed. Library
adapters use `raw::expect::<NixLibrary>` or the raw representation trait rather
than a public raw constructor. Pass Rust structs directly to `try_call_package`,
`try_call`, or `try_mk_derivation`; these receiving interfaces perform conversion
at the boundary. `try_into_nix_value` remains an explicit escape hatch.

This changes import paths and custom trait implementations, while retaining the
same lazy expressions, Nix evaluation and diagnostic origins.
