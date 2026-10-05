//! Shows PostgreSQL authoring and prints its complete generated NixOS module.
//! Rust supplies both the public option declarations and configuration implementation.
pub(crate) mod lowering;

pub mod model;

mod options;

pub(crate) mod schema;

fn main() {
    // Inputs and implementation remain independent contributions for NixOS merging.
    let module = schema::module()
        .module(lowering::implementation())
        .add(model::model());
    let artifact = rusnix_nix::nixos::compile_module(&module).unwrap();
    println!("{}", artifact.module.source);
}
