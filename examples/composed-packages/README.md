# Incrementally replacing package authoring

This example connects four Rust-authored package recipes: OpenSSL supplies curl
and Git, and curl supplies MariaDB. A dependency is another package a recipe needs;
the Rust code supplies selected dependencies explicitly and lets nixpkgs supply
the rest. Start with [composition.rs](composition.rs) to change those connections.

Rusix does not require a whole dependency closure to be rewritten. A package can
first be rewritten while all dependencies remain normal pinned nixpkgs. Rewritten
dependency values can then be supplied one by one through explicit callPackage
override records. Nix still evaluates the resulting expressions; nixpkgs supplies
fetchers, stdenv, builders, hooks and the unreplaced dependency graph.

`composition.rs` is the authoring example. It constructs OpenSSL from the Rust
factory, then supplies that deferred value explicitly to both Rust curl and Rust Git.
The resulting Rust curl value is explicitly supplied to Rust MariaDB. It
uses `NixExpression::bind` to share typed `Package` dependencies
lazily in the generated expression. No package factory or source recipe is copied
per dependent edge. Each package's `mod.rs` exposes its ordinary Rust factory.

```rust
let pkgs = Nixpkgs::new();
let openssl: Package = pkgs.call_package(
    &openssl::factory(openssl::model::Release::Preview), arguments(),
);
let curl: Package = pkgs.try_call_package(
    &curl::factory(), curl_arguments(&pkgs, openssl.clone()),
)?;
let git: Package = pkgs.try_call_package(
    &git::factory(), git::arguments().with_openssl(openssl),
)?;
let mariadb: Package = pkgs.call_package(
    &mariadb::factory(),
    NixAttrs::try_from_record(mariadb::model::Release::V1011.arguments())?
        .merge(NixAttrs::new([("curl", curl)])),
);
```

The full assembly in `composition.rs` returns `NixAttrs<Package>` and binds OpenSSL
and curl once, retaining `Package` on the lexical parameters. Typed argument records
lower at calls; neither packages nor factories need `as_value` conversions.
See [typed package authoring](../../docs/typed-package-values.md).

From the repository root, run `cargo run --locked -p rusix --example composed-packages`.
It prints generated Nix with a `packages` field containing the four connected
package expressions; Rust does not evaluate Nix or build the packages.

## Verification

See the [verified report](REPORT.md) for the source matrix, exact derivation
identities, source attribution results and full verification.

`cargo test --locked -p rusix --test composed` compares exact ATerm recipe
bytes, derivation identities and all output paths. Tags attached through the
OpenSSL factory result's normal `overrideAttrs` interface must be observable in
curl's OpenSSL passthru and Git's actual buildInputs;
a curl tag must also appear in MariaDB's actual buildInputs; ordinary pkgs.openssl lacks
these tags. Changing OpenSSL's `withZlib` through `.override` changes curl, Git and both MariaDB
derivations and continues to match the equivalently wired upstream graph.

An ordinary nixpkgs OpenSSL also works in the same assembly function. Excluded
OpenSSL and an unused graph stay lazy; an invalid supplied OpenSSL maps to curl's
Rust consuming operation and retains the original Nix diagnostic. Evaluation is
offline in fresh disposable stores. Nothing is fetched, built or installed.


The default graph uses the normal **pkgs.curl** flavour (IDN, PSL, Zstd and
non-static Brotli) because that is MariaDB's actual upstream dependency. The
independent curl suite still covers curlMinimal's native default recipe. Both
flavours come from the same complete Rust-authored curl factory.

The default graph has three explicit rewritten edges. MariaDB's *other*, direct
OpenSSL input and Git's curl input retain normal nixpkgs injection, as do transitive
dependencies, bootstrap fetchers and existing test graphs. Equal recipes mean
these normal and rewritten instances have identical default store identities.
This is selective edge substitution, rather than a global package-set overlay.

Generated Nix contains one OpenSSL, one curl and one MariaDB factory body/source
recipe; common MariaDB attributes are shared between client and server. The
suite checks source-hash occurrence counts, lexical bindings and absence of
broad deepSeq forcing.

Child failures in OpenSSL's fetcher operation, curl consuming supplied OpenSSL,
and MariaDB demanding a supplied, failing Rust curl retain the relevant package
operation origin and original Nix trace. A literal integer supplied as curl is
a separate backend-validation case: stdenv reports the precise buildInput
index without a generated child frame. Compiler-owned dependency metadata now
recovers the supplied Rust child instead of the outer
`mariadb.drvPath` demand. See the correlation test and saved diagnostic.


The stress suite covers seven distinct boundary configurations:

| Configuration | Authoring boundary |
| --- | --- |
| A | nixpkgs OpenSSL → Rust curl → Rust MariaDB |
| B | Rust OpenSSL → Rust curl → Rust MariaDB |
| C | Rust OpenSSL → Rust Git, with curl/MariaDB unforced |
| D | nixpkgs curl → Rust MariaDB |
| E | Rust OpenSSL → ordinary Nix curl |
| F | Rust curl (itself consuming Rust OpenSSL) → ordinary Nix MariaDB |
| G | Rust OpenSSL → ordinary Nix Git |

The reverse consumers also expose the tags, so fallback is detectable in both
directions. Two OpenSSL releases are crossed with all four MariaDB releases in
the connected graph. Normal MariaDB `.override` remains live. A throwing OpenSSL
node excluded by curl's TLS choice remains unforced even while MariaDB's server
and client recipes are evaluated.

The standalone assembly uses the existing `Nixpkgs::new()` x86_64-linux scope.
Independent factories are tested with caller-provided native/cross scopes. This
experiment does not add a general package-set/platform configuration API.
