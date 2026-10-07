use rusnix_ir::prelude::*;

fn main() {
    let _ = Nixpkgs::new().value("openssl");
}
