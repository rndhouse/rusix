//! Emit the connected Rust-authored region, using the ordinary Nix backend.
pub mod graph;

fn main() {
    let generated =
        rusnix_nix::compile(&rusnix_ir::Config::new().set("graph", graph::graph())).unwrap();
    println!("{}", generated.source);
}
