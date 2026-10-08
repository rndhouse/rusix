# Rusix

Rusix is a Rust library for defining packages and NixOS configuration in ordinary
Rust, as an alternative to the Nix language. It compiles these definitions into Nix
expressions that can use existing Nix packages and modules. Existing Nix code can
import and use the generated definitions.

- Model configuration with your own Rust types and compose definitions through
  ordinary Rust functions.
- Generated expressions preserve Nix's lazy evaluation. NixOS handles option
  validation and module merging.
- Source mappings help trace Nix evaluation errors back to the Rust code that
  produced them.

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
