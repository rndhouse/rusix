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
    /// Reusable Nix function accepting a release, dependencies and feature arguments.
    factory: PackageFunction<Package>,
    /// All four pinned release packages, each exposing client and server members.
    family: NixAttrs<Package>,
    /// Selected MariaDB 10.11 package exposing its client and server members.
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
