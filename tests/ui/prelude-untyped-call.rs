use rusnix_ir::prelude::*;

fn main() {
    let _ = Nixpkgs::new().function("id").call(true);
}
