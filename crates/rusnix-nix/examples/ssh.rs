//! Demonstrates the generic escape hatch rather than the preferred typed authoring API.
//! Dotted paths reach arbitrary options; Expr constraints remain deferred to Nix.
use rusnix_ir::{Config, Expr};
use rusnix_nix::compile;

// Legacy generic escape-hatch/compiler example. The authoring showcase is
// examples/README.md; this intentionally exercises raw option-path lowering.
pub fn config() -> Config {
    // Config::set builds explicit path/value bindings; it does not declare a NixOS schema.
    // The range check runs only when Nix demands this integer and retains its Rust origin.
    Config::new()
        .set("services.openssh.enable", true)
        .set(
            "services.openssh.ports",
            vec![Expr::int(22).in_range(1, 65535, "SSH port must be in 1..=65535")],
        )
        .set("logging.level", "normal")
}

fn main() {
    // Compilation checks generic binding invariants and emits source without invoking Nix.
    let generated = compile(&config()).unwrap();
    println!("{}", generated.source);
}
