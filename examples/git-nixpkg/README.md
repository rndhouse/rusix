# Git nixpkgs package rewrite

This example authors Git **2.47.0** in Rust/Rusnix, replacing the package expression
at `pkgs/applications/version-management/git/default.nix` in nixpkgs revision
**`8b27c1239e5c421a2bbc2c65d52e4a6fbf2ff296`**. The reference is the repository's
checked, vendored nixpkgs archive. This is a **package definition**, complementing
the [PostgreSQL NixOS module](../postgresql-nixos-module/README.md).

- `model.rs`: ordinary Rust feature choices. `Perl` groups SVN/send-email with the
  Perl support they require; platform-dependent defaults remain deferred to Nix.
- `inputs.rs`: a local `#[rusnix::args]` structural view of the deferred native Nix
  argument record. Scalar accessors are typed; packages and functions stay opaque.
- `lowering.rs`: feature/platform policy, the fixed derivation record, readable
  shell templates, metadata and passthru values.
- `main.rs`: composes the model and factory, then prints generated Nix. It evaluates
  no Nix and builds nothing.

```bash
cargo run --locked -p rusnix-nix --example git-nixpkg
```

`args::from_value(arguments)` binds the view to the record supplied by the native
Nix function. Access such as `inputs.stdenv.host_platform.is_darwin()` constructs
a lexical Nix dependency such as `stdenv.hostPlatform.isDarwin`; Rust never reads
the platform flags. Dependent defaults use the same lexical scope without
reconstructing the complete argument record. Declared subtrees
marked `#[rusnix(value)]` expose `as_value()` when the whole opaque record is
needed. Naming follows the same rules as `#[rusnix::options]`. Generic condition/list
helpers use `NixLibrary::from_value(inputs.lib.as_value())`, preserving the
caller's library overrides. Feature checks use `throw_if_not`; their order remains
explicit in the package factory. Other library functions use the visibly dynamic
`lib.as_value().clone().select(...).apply(...)` escape hatch.
Boolean `!` and `Expr<bool>::and` stay symbolic and
independent of library overrides. Only the local native-build predicate and pinned
Git asset paths remain as Inputs helpers.

The generated artifact exposes `factory` and `git`. Ordinary Nix can pass `factory`
to `pkgs.callPackage` with the same explicit framework/Perl-library arguments used
by upstream Git; the resulting package supports `.override` and `.overrideAttrs`.
The compiler artifact uses the pinned source tree staged by Rusnix's evaluator.

## What remains in Nix

Rust supplies the package definition, not a new derivation engine. The real
`stdenv.mkDerivation`, `callPackage`, fetchers, dependency splicing, setup hooks,
platform values, library functions, licenses and maintainers remain opaque Nix
objects. The Rust implementation never imports the original Git package
expression; comparison tests explicitly import it as the reference. Ordinary
nixpkgs dependencies are reused unchanged. Pinned patches and `update.sh` remain
source assets; fetchers construct fixed-output derivations without fetching.

The scoped callback API represents `finalAttrs`: the passthru install-check test
selects `finalAttrs.finalPackage.overrideAttrs`. Rust declares that finite
self-dependency without reading the eventual derivation. Native argument-set
functions preserve dependent defaults and `builtins.functionArgs`, so ordinary
Nix `callPackage` and function-argument overrides work normally.

The upstream expression is 399 lines, with 57 arguments (12 defaults), three
feature assertions, substantial install/check scripts and platform branches.

| Upstream construct | Capability classification |
| --- | --- |
| Fixed derivation fields and shell text | A: existing structural conversion and `nix_text!` |
| Feature/platform conditions and dependency lists | B: existing opaque callbacks and library calls |
| `finalAttrs.finalPackage.overrideAttrs` | B: existing scoped symbolic callback |
| Native argument interface with dependent defaults | C: small general `function_attrs` primitive |
| Patches and update-script paths | C: small general `source_path` primitive |
| Builders, fetchers, hooks and dependency propagation | E: retain nixpkgs authority through opaque calls |
| Metadata and external passthru tests | B: opaque existing ecosystem values |

No significant new machinery (D) was required. No Git-specific core type, raw Nix
source construction, reimplementation of stdenv, or broad recursive frontend was
needed.

## Mechanical proof

[`git.rs`](../../crates/rusnix-nix/tests/git.rs) and the ordinary Nix
[comparison fixture](../../tests/fixtures/git-equivalence.nix) instantiate both
factories in the same package scope. They compare exact `.drv` ATerm recipes,
derivation/output paths, source recipes, dependency identities, outputs, patches,
shell scripts and their string contexts, flags, metadata, update-script contents,
and the final-package install-check recipe. Recipe equality includes builder,
system, environment, input derivations and input sources. Dependency store hashes
retain transitive graph identity; they are not normalized away. Source asset paths
are coerced into store paths to avoid comparing temporary extraction directories.

The matrix covers default/minimal/full Git; Perl/SVN/send-email/PCRE2/manual/Python/
translation/GUI/SSH/libsecret/check choices; Linux and Darwin on x86_64 and ARM;
ARM, musl, MinGW and FreeBSD cross builds; dependent defaults, ordinary overrides,
`overrideAttrs`, laziness, malformed dependencies and all three feature assertions.
There are **40 Git tests**, including **34 full projection comparisons** (one
uses the explicit SunOS branch probe, and two check caller-supplied library
overrides), three feature-assertion rejection cases,
a malformed-dependency case, Solaris rejection, and focused Rust-model/laziness
checks, plus regressions against reconstructed argument records and structural
runtime diagnostic wrappers. The SunOS
make-flags branch has an explicit host-flag probe. A real Solaris cross scope is rejected by this pinned nixpkgs's libc support in both implementations;
the probe is not evidence of a working Solaris toolchain.

These are **evaluation/derivation equivalence** tests. No source is fetched and
Git is not built or executed. Runtime/build success is a separate experiment.

Package assertions recover Rust lowering locations. A malformed dependency that
stdenv rejects while forcing a returned derivation may map only to the Rust
crossing boundary; the original Nix trace is retained. Rust does not inspect
package internals or attribute every upstream failure to an exact Rust field.

The [runtime diagnostic audit](../../docs/runtime-diagnostics.md) keeps opaque-call
boundaries while mapping ordinary symbolic operations through source spans. It
reduces runtime wrappers to 301 contexts. Compact origin IDs bring the generated
Git output to 121,509 bytes; all
2,337 origin comments remain available for inspection.
