//! Prints the complete pinned curl factory and a Rust-authored package choice.
mod inputs;

mod lowering;

pub mod model;

use rusnix_ir::{
    IntoConfig,
    interop::{Nixpkgs, Package, PackageFunction},
};

#[derive(IntoConfig)]
struct Output {
    /// Reusable Nix function accepting curl's dependencies and feature arguments.
    factory: PackageFunction<Package>,
    /// Selected curl package instantiated with this example's feature choices.
    curl: Package,
}

fn main() {
    let factory: PackageFunction<Package> = lowering::factory();
    let curl = Nixpkgs::new()
        .try_call_package(&factory, model::model().arguments())
        .expect("fixed authoring arguments");
    let artifact = rusnix_nix::compile(Output { factory, curl })
        .expect("the example has valid structural values");
    println!("{}", artifact.source);
}
