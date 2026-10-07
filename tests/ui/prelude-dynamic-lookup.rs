use rusix_ir::prelude::*;

fn main() {
    let _ = Nixpkgs::new().value("openssl");
}
