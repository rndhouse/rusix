# Incrementally replacing package authoring

Rusnix does not require a whole dependency closure to be rewritten. A package can
first be rewritten while all dependencies remain normal pinned nixpkgs. Rewritten
dependency values can then be supplied one by one through explicit callPackage
override records. Nix still evaluates the resulting expressions; nixpkgs supplies
fetchers, stdenv, builders, hooks and the unreplaced dependency graph.

`graph.rs` is the authoring example. It constructs OpenSSL from the Rust factory,
then supplies that deferred value explicitly to both Rust curl and Rust Git. It
uses the existing `NixValue::function` callback bindings to share dependency values
lazily in the generated expression. No package factory or source recipe is copied
per dependent edge. Each package's `mod.rs` exposes its ordinary Rust factory.

```rust
let pkgs = Nixpkgs::new();
let openssl = pkgs.call_package(&openssl::factory(Release::Preview), arguments());
let curl = pkgs.call_package(
    &curl::factory(),
    NixValue::record([("openssl", openssl.clone())]),
);
let git = pkgs.call_package(
    &git::factory(),
    git::arguments().merge_attrs(NixValue::record([("openssl", openssl)])),
);
```

`cargo test --locked -p rusnix-nix --test composed` compares exact ATerm recipe
bytes, derivation identities and all output paths. Tags attached through the
OpenSSL factory result's normal `overrideAttrs` interface must be observable in
curl's OpenSSL passthru and Git's actual buildInputs; ordinary pkgs.openssl lacks
these tags. Changing OpenSSL's `withZlib` through `.override` changes both consumer
derivations and continues to match the equivalently wired upstream graph.

An ordinary nixpkgs OpenSSL also works in the same assembly function. Excluded
OpenSSL and an unused graph stay lazy; an invalid supplied OpenSSL maps to curl's
Rust consuming operation and retains the original Nix diagnostic. Evaluation is
offline in fresh disposable stores. Nothing is fetched, built or installed.
