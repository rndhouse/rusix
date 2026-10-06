//! Emit MariaDB's shared factory, family and default server/client result.
mod inputs;

mod lowering;

pub mod model;

mod scripts;

use rusnix_ir::{Config, interop::Nixpkgs};

fn main() {
    let factory = lowering::factory();
    let mariadb = Nixpkgs::new().call_package(&factory, model::Release::V1011.arguments());
    let generated = rusnix_nix::compile(
        &Config::new()
            .set("factory", factory)
            .set("family", lowering::family())
            .set("mariadb", mariadb),
    )
    .unwrap();
    println!("{}", generated.source);
}
