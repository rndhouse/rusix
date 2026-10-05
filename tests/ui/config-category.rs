use rusnix_ir::{self as rusnix, interop::Nixpkgs};

#[rusnix::config]
mod config {
    use rusnix_ir::interop::PackageRef;

    #[rusnix(root)]
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
