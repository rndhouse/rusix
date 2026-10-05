//! Shows PostgreSQL authoring and prints its generated configuration implementation.
//! The equivalence tests pair this output with the upstream option declarations / public schema.
pub(crate) mod lowering;

pub mod model;

mod options;

fn main() {
    // Inputs and implementation remain independent contributions for NixOS merging.
    let module = lowering::implementation().add(model::model());
    let artifact = rusnix_nix::nixos::compile_module(&module).unwrap();
    println!("{}", artifact.module.source);
}
