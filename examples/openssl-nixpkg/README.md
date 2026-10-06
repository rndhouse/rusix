# OpenSSL authored in Rusnix

The semantic reference is `pkgs/development/libraries/openssl/default.nix`
(337 lines, 20 public arguments, eight lazy defaults) at nixpkgs
`8b27c1239e5c421a2bbc2c65d52e4a6fbf2ff296`.
`model.rs` preserves the three releases: 1.1.1w, 3.0.15 and 3.3.2.
`lowering.rs` shares the complete recipe, selects version patches and metadata,
and exports both a family factory and individually overridable package factories.
`inputs.rs` describes the finite argument interface; `scripts.rs` preserves exact
shell phase text using `nix_text!`. `main.rs` emits the family and default package.
The complete argument view supplies `args::argument_names()` to both factories,
including unused upstream parameters. Naming annotations cover exceptions such
as `enableSSL2`; ordinary names use the default lowerCamelCase mapping.
Fixed opaque records use `nix_record!`, with iterators retained for release families
and dynamic target tables.

All package policy is Rust authored. The backend remains the caller's nixpkgs
library, fetchurl, stdenv.mkDerivation, outputs, wrappers, setup hooks and tests.
Unused upstream arguments remain in the public interface. Shared recipe parameters
stay lexically captured: overriding `version` through `overrideAttrs` does not
change the upstream source URL or changelog. The recursive pkg-config test does
observe `finalAttrs.finalPackage`.

OpenSSL bootstraps fetchurl. Patches therefore use checked-in files from the pin;
introducing fetchpatch here would create a bootstrap dependency cycle. No source
or patch is downloaded and no package is built by this experiment.

Run `cargo test --locked -p rusnix-nix --test openssl`. The suite compares exact
`.drv` identities and recipe bytes, output paths, source recipes, patch bytes and
store paths, phase text and string contexts, flags, metadata, family membership,
argument defaults and recursive pkg-config tests. It covers every release,
individual feature switches, configuration files, static builds, native Linux
and Darwin variants, cross Linux/musl/MinGW/FreeBSD/RISC-V/MIPS, attribute overrides,
excluded dependency laziness and Rust provenance for a failing fetcher call.
Artifacts are saved under `target/openssl-equivalence/`.

Patch source paths refer to either the checked checkout or its session symlink;
comparisons inspect their actual bytes and Nix store paths rather than temporary
filesystem spelling. Metadata's source `position` is excluded because the package
is now authored in Rust. Neither exclusion changes or normalizes derivation recipes.

```rust
use rusnix_ir::{interop::Nixpkgs, nix_record};

let pkgs = Nixpkgs::new();
let openssl = pkgs.call_package(
    &lowering::factory(model::Release::Preview),
    nix_record! {},
);
```
