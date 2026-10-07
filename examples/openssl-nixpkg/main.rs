//! Emit the complete family and the default package without building or fetching.
mod inputs;

mod lowering;

mod model;

mod scripts;

use rusnix_ir::{
    IntoConfig,
    interop::{NixAttrs, Nixpkgs, Package, PackageFunction},
    nix_record,
};

#[derive(IntoConfig)]
struct Output {
    family: PackageFunction<NixAttrs<Package>>,
    openssl: Package,
}

fn main() {
    let factory = lowering::factory(model::Release::Preview);
    let openssl = Nixpkgs::new().call_package(&factory, nix_record! {});
    let generated = rusnix_nix::compile(Output {
        family: lowering::family_factory(),
        openssl,
    })
    .unwrap();
    println!("{}", generated.source);
}
