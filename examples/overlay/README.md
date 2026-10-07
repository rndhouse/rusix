# A normal nixpkgs overlay authored in Rust

An overlay customizes named packages while reusing the rest of nixpkgs' package
set. It is a Nix function `final: prev: { ... }`: `prev` provides the packages
before this overlay, and `final` refers to the set after all overlays. References
to `final` let packages depend on each other's final versions; Nix resolves only
the values an evaluation needs.

This example authors the same function in Rust/Rusnix. It modifies ordinary
pinned nixpkgs `curl` with `prev.curl.overrideAttrs`, appending `--disable-dict`
to its existing configure flags to disable the DICT protocol. Curl's source,
dependencies and upstream package definition stay in nixpkgs. The remaining
package set stays ordinary nixpkgs.

Start with [authoring.rs](authoring.rs) to change the curl customization.
[main.rs](main.rs) composes it with source generation:

```rust
let pkgs = authoring::package_set();
let output = rusnix_ir::Config::new()
    .set("overlay", authoring::overlay())
    .set("curlDerivation", pkgs.select("curl.drvPath"));
```

Both calls construct deferred expressions in Rust. The executable prints the
overlay function and an expression selecting the modified curl's build-recipe
path (`drvPath`); Nix applies the overlay only when that output is evaluated.

The equivalent handwritten overlay is:

```nix
final: prev: {
  curl = prev.curl.overrideAttrs (old: {
    configureFlags = old.configureFlags ++ [ "--disable-dict" ];
  });
}
```

[authoring.rs](authoring.rs) explicitly imports `interop::raw::{NixValue, NixFunctionExt}`
and uses nested `NixValue::function` callbacks for
`final` and `prev`, the existing `override_attrs` operation, and `nix_record!`.
`Nixpkgs::new().pkgs_function("extend").call(overlay())` asks ordinary nixpkgs to
apply the function. Selecting `curl` from the returned set observes the modified
package. The same Rust-authored function can be passed in an import's `overlays`
list.

- Rusnix package examples replace package definitions written in Nixlang.
- This overlay example replaces overlay/customization code written in Nixlang.

From the repository root:

```bash
cargo run --locked -p rusnix-nix --example overlay
cargo test --locked -p rusnix-nix --test overlay
```

The executable prints generated Nix without invoking Nix. Tests evaluate the
unchanged pin `8b27c1239e5c421a2bbc2c65d52e4a6fbf2ff296` for `x86_64-linux`,
offline in disposable stores; they never build packages. They compare complete
curl and downstream `curlpp` recipes and identities against the handwritten
overlay, check the exact configure-flags change, preserve `hello`'s identity,
and verify ordered `prev`, fixed-point `final`, and unused failing attributes.
A controlled error inside `overrideAttrs` maps to its Rust division operation;
upstream package internals retain their original Nix diagnostic frames.
Reviewable generated code, source maps, results and the failing Nix diagnostic
are saved under `target/overlay-equivalence/`.
