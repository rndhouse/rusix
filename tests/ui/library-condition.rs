use rusnix_ir::{Expr, interop::Nixpkgs};

fn main() {
    let _ = Nixpkgs::new().library().optional(Expr::int(3), "text");
}
