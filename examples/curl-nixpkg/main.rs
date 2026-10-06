//! Prints the complete pinned curl factory and a Rust-authored package choice.
mod inputs;

mod lowering;

pub mod model;

use rusnix_ir::{
    Config,
    interop::{Nixpkgs, PackageFunction},
};

fn main() {
    let factory: PackageFunction = lowering::factory();
    let curl = Nixpkgs::new().call_package(&factory, model::model().arguments());
    let artifact = rusnix_nix::compile(&Config::new().set("factory", factory).set("curl", curl))
        .expect("the example has valid structural values");
    println!("{}", artifact.source);
}
