# Rusix

Author Nix packages and NixOS configuration in Rust. Use your own types and
functions, compile them into Nix, and compose them with the existing Nix ecosystem.

- **Your types define your model.** Rust's type checking, enums and reusable
  libraries help you express valid configuration.
- **Nix evaluates the result.** Deferred expressions preserve lazy evaluation,
  package dependencies and NixOS module semantics.
- **Adopt it incrementally.** Rust-authored packages and modules can use existing
  nixpkgs values and ordinary Nix definitions.
- **Trace errors back to Rust.** Source mappings connect Nix evaluation failures
  to the Rust code that produced them, where an origin can be recovered.

```rust
use rusix::{IntoConfig, compile};

#[derive(IntoConfig)]
struct Service {
    /// Whether the service should run.
    enable: bool,
    /// Port on which the service listens.
    port: u16,
}

fn main() {
    let generated = compile(Service { enable: true, port: 8080 })
        .expect("valid configuration");
    println!("{}", generated.source);
}
```

Structs become Nix attribute sets. Rust constructs the description; deferred
expressions are evaluated later by Nix. Generated Nix is compiler output—edit Rust.

Use `rusix` as a library in your own crate. It re-exports the authoring macros,
compilation and evaluation APIs; no separate macro dependency is needed.
Compilation requires only Rust. The optional evaluator runs offline in a
disposable Nix store and never builds packages or activates a system.

**Experimental.** Rust checks your model; NixOS still validates options and
combines module definitions. Some backend errors cannot be mapped precisely to Rust.

- [Examples](examples/README.md): configuration models, package definitions and NixOS modules.
- [Library usage](crates/rusix/README.md): dependencies, evaluation and local nixpkgs setup.
- [Development and design](docs/development.md): running the project, verification and limitations.
