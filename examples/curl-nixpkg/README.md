# curl package rewrite

This example authors the complete curl package function in Rust and calls the real
nixpkgs builders. Its reference is **nixpkgs
`8b27c1239e5c421a2bbc2c65d52e4a6fbf2ff296`**, specifically
[`pkgs/by-name/cu/curlMinimal/package.nix`](https://github.com/NixOS/nixpkgs/blob/8b27c1239e5c421a2bbc2c65d52e4a6fbf2ff296/pkgs/by-name/cu/curlMinimal/package.nix):
227 lines, curl **8.11.0**, 53 arguments (35 required, 18 with defaults).
The checked `vendor/nixpkgs` submodule supplies the reference offline.

- `model.rs` offers a small optional Rust authoring surface. `TlsBackend` makes a
  concrete choice of at most one backend; it does not replace the public Nix API.
- `inputs.rs` declares the finite external interface with `#[rusnix::args]`.
  Accessors describe later Nix lookups; Rust never reads package or platform data.
- `lowering.rs` implements the recipe, lazy defaults, native TLS assertion,
  feature flags, scripts, dependencies, recursive checks and metadata.
- `main.rs` prints a record containing the ordinary factory and an example package.

Start with `model.rs` to change the TLS backend, HTTP/3 or websocket choice.
`main.rs` connects those ordinary Rust choices to the package recipe:

```rust
let choices = model::Curl {
    tls: Some(model::TlsBackend::OpenSsl),
    http3: true,
    websocket: false,
};
let factory = lowering::factory();
let curl = rusnix_ir::interop::Nixpkgs::new()
    .try_call_package(&factory, choices.arguments())
    .expect("fixed authoring arguments");
```

The factory is a Nix function returning a build recipe. `try_call_package` checks
the Rust argument record and describes Nix filling other arguments from its package
set through `callPackage`. The executable prints `factory` and `curl` expressions;
Rust does not evaluate Nix or build curl.

```bash
cargo run --locked -p rusnix-nix --example curl-nixpkg > target/curl.nix
cargo test --locked -p rusnix-nix --test curl
```

The complete `#[rusnix::args]` view supplies `args::argument_names()` to the
factory, so public argument names are declared once. Defaults remain explicit
deferred expressions. Fixed opaque records use `nix_record!`; dynamic feature
arguments remain ordinary Rust collections.

Ordinary Nix code can use `pkgs.callPackage generated.factory { ... }`, inspect
`builtins.functionArgs generated.factory`, and use `.override` and `.overrideAttrs`.
The compiler artifact uses the pinned source tree staged by Rusnix's evaluator.
The factory preserves upstream's four independent TLS booleans and native Nix
assertion. The enum constrains Rust authoring only; Nix still validates external
choices. The candidate never imports the original curl expression.

`stdenv.mkDerivation` is nixpkgs' standard build-recipe constructor. Rusnix supplies
its attributes through a scoped `finalAttrs` callback: version overrides affect
source URLs and changelog, and `finalAttrs.finalPackage` supplies the eventual
package to consuming-package overrides and `withCheck`. Fetchers, setup hooks,
compiler/platform records, package dependencies and build semantics stay in
nixpkgs. This example defines no replacement builder or global package bindings.

Curl participates in bootstrapping `fetchurl`, so source preparation must not
introduce a dependency on `fetchpatch`. The pinned source needs only script
substitutions. `fetchpatch` occurs solely in the existing lazy passthru test graph.

## Pinned-source audit and abstraction decisions

A = existing Rusnix API; B = explicit Nix interop; C = independent evidence for a
deferred candidate; D = missing general operation; E = nixpkgs remains authoritative.

| Source construct | Classification | Implementation / decision |
| --- | --- | --- |
| Native arguments and 18 lazy defaults | A | `function_attrs` and structural argument views; defaults use lexical parameters |
| GSS/SCP defaults and platform feature predicates | A/C | Symbolic booleans and shared `package::build_host_equal` comparison of full platform values |
| Four TLS flags and `lib.count` | B | Call the supplied `lib.count`; the Rust enum is a separate convenience surface |
| Native TLS `assert` | D | Add `NixValue::assert`; using `lib.throwIfNot` would let a caller override change native assertion semantics |
| Builtin release-tag replacement and comparison | D | `replace_text` and builtin comparison remain independent of nixpkgs `lib` |
| Fixed recipe and metadata | A/C | Local derived Rust records and `try_into_nix_value`; no universal mkDerivation or metadata schema |
| Conditional propagated inputs | A/B | Supplied `optional`/`optionals`; native builtin concatenation preserves independence from overridden `lib.concatLists` |
| Configure switches | B | Supplied `enableFeature`, `withFeature`, `withFeatureAs`; no configure DSL or new library bindings |
| Environment workaround | B/C | Supplied `optionalAttrs` with a normal Rust record; no attrset builder |
| Output selection | B/C | `NixLibrary::get_dev` and `get_lib` call the supplied functions, retaining output fallback semantics |
| Shell scripts | A | `nix_text!` and supplied `optional_text`; exact text and Nix store dependency context |
| Recursive source/consuming-package checks | A/E | Scoped callbacks, shared `override_args`/`override_attrs`, real final package |
| Fetchpatch test's shallow record replacement | D | Add `NixValue::merge_attrs` for native `//`; distinct from NixOS definition merging |
| Sources, build tools, frameworks, standard builder | E | Existing caller-supplied nixpkgs objects; evaluation never fetches or builds |

Output helpers use the shared `NixLibrary` API based on Git and curl usage.
`optionalAttrs` and the configure helpers retain ordinary deferred library calls;
metadata records stay local. The shared `package::build_host_equal` predicate
compares full platform records rather than system strings. Boolean disjunction
uses `.or()`; release-tag replacement uses `NixValue::replace_text`.
Curl does **not** call `lib.concatStringsSep`; its text construction uses the
existing generic primitive, so that candidate remains deferred. There is no
ordered multi-check helper or package-validation DSL.

## Equivalence evidence and limits

[`curl.rs`](../../crates/rusnix-nix/tests/curl.rs) compares the candidate with the
pinned expression using the same real `callPackage` dependency scope. It compares
complete derivation recipes (including hash-relevant environment, builder, inputs,
outputs and scripts), exact derivation/output paths, source recipe/URLs, dependency
order, configure flags, shell bytes/string contexts, metadata and passthru.
Nothing affecting a derivation hash is normalized away. Linux's default recipe is
`/nix/store/cb2y179hgas7837a8wnx08gxawn61p1m-curl-8.11.0.drv`.

The suite compares full package projections, the public-function interface and
paired rejection cases. [Generic-operation tests](../../crates/rusnix-nix/tests/nix_operations.rs)
cover builtin access, native assertions and record union, including laziness and
precise source attribution.

The matrix includes both directions of 14 independent feature switches; all five
valid TLS choices and all 11 invalid multi-backend combinations; minimal/full
recipes; zlib-dependent defaults; Linux/ARM/Darwin, musl, MinGW, FreeBSD and Darwin
cross compilation; real static builds; explicit SunOS/Cygwin/Windows/static branch
probes; ordinary overrides; version-sensitive finalAttrs; supplied library
changes; excluded dependencies; and Rust-source diagnostic checks. Full recursive
passthru recipe comparisons run for default and version-overridden curl, including
fetchpatch, the consuming language packages, static curl, metadata checks and both
nginx HTTP/3 tests. A replaced `lib.throwIfNot` cannot bypass the native assertion.

The supplied library controls functions the original package actually calls;
unrelated replacements of `concatLists`, `concatStringsSep` or `replaceStrings`
do not change native package operations. Nix string dependency context survives
interpolation. Native record-union failures retain a narrow runtime diagnostic
boundary because Nix otherwise reports only their containing field. Routine
navigation and the native assertion use generated source spans. No broad forcing
or argument-record reconstruction is added.

All evaluation uses disposable isolated Nix stores with networking and builds
disabled. SunOS branch probes do not claim a working Solaris backend: the pinned
nixpkgs compiler setup rejects the real Solaris target in both implementations.
This proves evaluation/derivation compatibility, not successful curl builds,
protocol behavior, package tests or service execution. Upstream test/dependency
packages remain existing nixpkgs packages, exactly as in the reference recipe.

The Rust factory returns `PackageFunction<Package>`, retaining the instantiated
package interface through nixpkgs dependency injection:

```rust
let factory: PackageFunction<Package> = lowering::factory();
let curl: Package = Nixpkgs::new().try_call_package(&factory, model::model().arguments())?;
```

Argument records implement `IntoRusnixValue` and lower only at the call boundary.
Dependency accessors return `Package`, sources return through `Overridable<NixCallable<Package>>`,
phase text uses `Expr<String>`, and package/flag lists retain their element types.
A typed output root exports the factory and package directly. Dynamic fields remain
available through `as_expression`; Nix validates external expectations lazily.
See [typed package authoring](../../docs/typed-package-values.md).
