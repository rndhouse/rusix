# Git nixpkgs package rewrite

This example authors Git **2.47.0** in Rust/Rusnix, replacing the package expression
at `pkgs/applications/version-management/git/default.nix` in nixpkgs revision
**`8b27c1239e5c421a2bbc2c65d52e4a6fbf2ff296`**. The reference is the repository's
checked, pinned `vendor/nixpkgs` submodule. This is a **package definition**, complementing
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
caller's library overrides. Feature checks use native `NixValue::assert`; their
order remains explicit in the package factory, and an overridden `lib.throwIfNot`
cannot bypass them. Other library functions use the visibly dynamic
`lib.as_value().clone().select(...).apply(...)` escape hatch.
Boolean `!`, `.and()`, `.or()` and `.implies()` stay symbolic and
independent of library overrides. Passthru merging uses `merge_attrs`, so replacing `lib.any` or
`lib.mergeAttrs` does not change those native operations. Pinned Git asset paths
remain an Inputs helper.
Output selection uses `NixLibrary::get_dev`, retaining the supplied library's
fallback behavior. `override_args` and `override_attrs` call the existing package
functions, including the eventual final package's recursive test override.

The complete `Inputs` declaration also supplies `args::argument_names()` to the
factory; there is no separate list of its 57 parameters. Defaults remain explicit
and lazy in the factory. Fixed opaque records use `nix_record!`, while dynamic
feature argument lists remain ordinary Rust collections.

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
Full projection comparisons include the explicit SunOS branch probe and
caller-supplied library overrides. Rejection cases cover feature assertions,
malformed dependencies and Solaris. Focused Rust-model/laziness checks include
regressions against reconstructed argument records and structural runtime
diagnostic wrappers. A paired rendering test also compares the complete
projection with and without inspection comments. The SunOS
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
records the earlier runtime-context measurements. The [pretty-printer](../../docs/generated-nix-layout.md)
uses a 100-character target, formatting functions, dependency lists and fields
while recording their source spans. Normal output has no fine-grained origin
comments. For manual inspection,
`rusnix_nix::compile_with_options` accepts `RenderOptions { origin_comments: true }`,
adding origin comments. Both modes retain the same mapped origins; each source
map has offsets for its own text. Historical sizes are recorded in the renderer
audits and vary as the authored recipe changes.
[Precedence-aware rendering](../../docs/nix-expression-rendering.md) removes blanket
application/selection parentheses while preserving explicit AST groups.
Quoted shell fragments remain unchanged and can exceed the target width.

The Rust factory now returns `PackageFunction`, identifying its native named
argument interface. Instantiation uses the explicit nixpkgs operation:

```rust
let factory: PackageFunction = lowering::factory();
let git = Nixpkgs::new().call_package(&factory, inputs::arguments(model::model()));
```

The result remains `NixValue`; Nix validates the arguments and package body.
`Config::set("factory", factory)` accepts the function directly. Explicit
`factory.as_value()` or conversion to `NixValue` permits ordinary Nix function
calls and inspection, preserving lazy defaults and provenance.
