use rusnix_ir::{Expr, interop::Nixpkgs};

fn main() {
    let _: Expr<String> = Nixpkgs::new().source_path("default.nix");
}
