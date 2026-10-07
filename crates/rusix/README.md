# Rusix

Use ordinary Rust types and functions to describe Nix configuration and packages.
Rusix compiles those descriptions into Nix source, then optionally evaluates them
offline in a disposable Nix store. It never builds packages or activates a system.
This is an experimental library.

After publication, add one dependency to your own library or executable:

```toml
[dependencies]
rusix = "0.1"
```

Configuration macros and derives are re-exported by `rusix`; Cargo brings in
`rusix-derive` automatically. You can also rename the dependency in Cargo.toml.

```rust
use rusix::{Expr, IntoConfig, NixSession, compile};

#[derive(IntoConfig)]
struct Output {
    /// Integer computation deferred until Nix evaluates this field.
    answer: Expr<i64>,
}

let generated = compile(Output { answer: Expr::int(84).divide(Expr::int(2)) })
    .expect("valid Rust configuration");
let session = NixSession::new().expect("temporary evaluation workspace");
let result = session.evaluate(&generated).expect("successful Nix evaluation");
assert_eq!(result.value["answer"], 42);
```

Compilation requires no Nix process. Evaluation requires `nix` and
`nix-instantiate` on PATH. For expressions using nixpkgs or NixOS, supply an
existing clean local checkout matching `rusix::nixos::NIXPKGS_REVISION`:

```rust
let session = rusix::NixSession::with_nixpkgs("/path/to/nixpkgs")
    .expect("clean pinned nixpkgs checkout");
```

Rusix verifies the checkout without fetching anything. Keep it unchanged for the
lifetime of the session. Repository development uses the pinned `vendor/nixpkgs`
submodule automatically; registry consumers supply their own checkout.

Start with `rusix::prelude` for authoring, `rusix::interop` for existing Nix values,
and `rusix::nixos` for system configuration. `rusix::ir` and
`rusix::compiler::ast` support explicit inspection and compiler extensions.

The same package contains the `rusix` executable. Its current commands are an
experimental fixture harness, for example `rusix check good --out target/demo`.
After publication, install it with `cargo install rusix`.
For NixOS fixtures outside the repository, use
`rusix --nixpkgs /path/to/nixpkgs check-nixos good --out target/demo`.

The repository contains runnable examples, comparison fixtures and full
development instructions: <https://github.com/rndhouse/rusix>.
