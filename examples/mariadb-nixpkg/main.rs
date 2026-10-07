//! Emit MariaDB's shared factory, family and default server/client result.
mod inputs;

mod lowering;

pub mod model;

mod scripts;

use rusnix_ir::{
    IntoConfig,
    interop::{NixAttrs, Nixpkgs, Package, PackageFunction},
};

#[derive(IntoConfig)]
struct Output {
    factory: PackageFunction<Package>,
    family: NixAttrs<Package>,
    mariadb: Package,
}

fn main() {
    let factory = lowering::factory();
    let mariadb = Nixpkgs::new()
        .try_call_package(&factory, model::Release::V1011.arguments())
        .expect("fixed authoring arguments");
    let generated = rusnix_nix::compile(Output {
        factory,
        family: lowering::family(),
        mariadb,
    })
    .unwrap();
    println!("{}", generated.source);
}
