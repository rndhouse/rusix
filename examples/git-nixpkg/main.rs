//! Authors the pinned Git package in Rust and prints its deferred build expression.
mod inputs;

mod lowering;

pub mod model;

use rusnix_ir as rusnix;
use rusnix_ir::{IntoConfig, interop::NixValue};

#[rusnix::config]
mod output {
    use super::NixValue;

    /// Exports both the reusable native Nix factory and this example's chosen package.
    #[rusnix(root)]
    pub(super) struct Package {
        /// Ordinary Nix callers may use callPackage to supply their own package scope.
        pub(super) factory: NixValue,
        /// A real mkDerivation result; printing compiler output does not build Git.
        pub(super) git: NixValue,
    }
}

fn main() {
    let factory = lowering::factory();
    let git = inputs::instantiate(factory.clone(), model::model());
    let artifact = rusnix_nix::compile(&output::Package { factory, git }.into_config()).unwrap();
    println!("{}", artifact.source);
}
