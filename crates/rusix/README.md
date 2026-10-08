# Rusix

Use ordinary Rust types and functions to describe Nix configuration and packages.
Rusix compiles those descriptions into Nix source, then optionally evaluates them
offline in a disposable Nix store. It never builds packages or activates a system.
This is an experimental library.

Add one dependency to your own library or executable:

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

Compilation requires no Nix process. The default `evaluation` feature provides
`NixSession`; evaluation is supported on Linux with Nix 2.34.8. It requires `nix`
and `nix-instantiate` on PATH. Rust 1.88 or newer is required.

For authoring and compilation alone, including on Windows:

```toml
[dependencies]
rusix = { version = "0.1", default-features = false }
```

For expressions using nixpkgs or NixOS, supply an
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

The `dev-cli` feature enables the repository's diagnostic fixture executable.
It runs fixed examples for repository verification. For example, `cargo run --features dev-cli -- check good --out target/demo`
evaluates a fixture and saves its artifacts.

The public IR and compiler AST APIs support compiler extensions. Rusix is
experimental: minor releases before 1.0 may change APIs. Saved diagnostics and
source maps should be read with the same Rusix version that produced them.

The repository contains runnable examples, comparison fixtures and full
development instructions: <https://github.com/rndhouse/rusix>.
