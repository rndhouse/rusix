use rusnix_ir::{Config, Expr};
use rusnix_nix::compile;

// Legacy generic escape-hatch/compiler example. The authoring showcase is
// examples/README.md; this intentionally exercises raw option-path lowering.
pub fn config() -> Config {
    Config::new()
        .set("services.openssh.enable", true)
        .set(
            "services.openssh.ports",
            vec![Expr::int(22).in_range(1, 65535, "SSH port must be in 1..=65535")],
        )
        .set("logging.level", "normal")
}

fn main() {
    let generated = compile(&config()).unwrap();
    println!("{}", generated.source);
}
