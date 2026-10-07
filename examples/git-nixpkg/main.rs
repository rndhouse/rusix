//! Authors the pinned Git package in Rust and prints its deferred build expression.
mod inputs;

mod lowering;

pub mod model;

pub use inputs::Arguments;

use rusnix_ir as rusnix;
use rusnix_ir::interop::{Nixpkgs, Package as PackageValue, PackageFunction};

#[rusnix::config]
mod output {
    use super::{PackageFunction, PackageValue};

    /// Exports both the reusable native Nix factory and this example's chosen package.
    #[rusnix(root)]
    pub(super) struct Package {
        /// Ordinary Nix callers may use callPackage to supply their own package scope.
        pub(super) factory: PackageFunction<PackageValue>,
        /// A real mkDerivation result; printing compiler output does not build Git.
        pub(super) git: PackageValue,
    }
}

fn main() {
    let factory: PackageFunction<PackageValue> = lowering::factory();
    let git = Nixpkgs::new()
        .try_call_package(&factory, inputs::arguments(model::model()))
        .expect("fixed authoring arguments");
    let artifact = rusnix_nix::compile(output::Package { factory, git }).unwrap();
    println!("{}", artifact.source);
}
