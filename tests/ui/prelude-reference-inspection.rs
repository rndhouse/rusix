use rusnix_ir::prelude::*;

fn main() {
    let _ = Nixpkgs::new().get("openssl").reference();
}
