//! Emit the connected Rust-authored region, using the ordinary Nix backend.
pub mod composition;

use rusnix_ir::{
    IntoConfig,
    interop::{NixAttrs, Package},
};

#[derive(IntoConfig)]
struct Output {
    /// Connected package set: OpenSSL feeds curl and Git; curl feeds MariaDB.
    /// Each package remains a deferred recipe that Nix evaluates when demanded.
    packages: NixAttrs<Package>,
}

fn main() {
    let generated = rusnix_nix::compile(Output {
        packages: composition::packages(),
    })
    .unwrap();
    println!("{}", generated.source);
}
