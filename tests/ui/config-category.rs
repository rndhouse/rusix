use rusix_ir::{self as rusix, interop::Nixpkgs};

#[rusix::config]
mod config {
    use rusix_ir::interop::PackageRef;

    #[rusix(root)]
    pub struct Machine {
        pub packages: Vec<PackageRef>,
    }
}

fn main() {
    let module = Nixpkgs::new().module("misc/label.nix");
    let _ = config::Machine {
        packages: vec![module],
    };
}
