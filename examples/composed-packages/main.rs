//! Emit the connected Rust-authored region, using the ordinary Nix backend.
pub mod graph;

use rusnix_ir::{
    IntoConfig,
    interop::{NixAttrs, Package},
};

#[derive(IntoConfig)]
struct Output {
    graph: NixAttrs<Package>,
}

fn main() {
    let generated = rusnix_nix::compile(Output {
        graph: graph::graph(),
    })
    .unwrap();
    println!("{}", generated.source);
}
