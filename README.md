# Rusix

Rusix lets you define Nix packages and NixOS configuration in Rust.
It compiles those definitions into Nix expressions.

- Define configuration with your own Rust types.
- Nix evaluates the generated expressions when their values are needed.
  NixOS combines module definitions during evaluation.
- Rust definitions can use existing nixpkgs packages and Nix modules.
- Source mappings can connect Nix errors to their Rust source locations.

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

Structs become Nix attribute sets. In this example, `Service` produces
`enable` and `port` attributes. Edit the Rust source to change the output.

Use `rusix` as a library in your own crate. It includes authoring macros and
the compiler. It also provides an optional Nix evaluator.
Generating Nix requires only Rust. Evaluation requires Nix.
The evaluator runs offline in a temporary store.

Rusix is experimental. NixOS checks option values during evaluation.
Some Nix errors cannot be mapped to a Rust source location.

- [Examples](examples/README.md)
- [Library usage](crates/rusix/README.md)
- [Development and design](docs/development.md)
