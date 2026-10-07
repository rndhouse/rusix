//! Print a Rust-authored overlay and the lookup of curl from its extended package set.
mod authoring;

use rusnix_ir::{Config, Expr, interop::Package};

fn main() {
    let pkgs = authoring::package_set();
    let curl: Package = pkgs.get("curl").into();
    let artifact = rusnix_nix::compile(
        &Config::new()
            .set("overlay", authoring::overlay())
            .set("curlDerivation", curl.field::<Expr<String>>("drvPath")),
    )
    .expect("the overlay has valid structural values");
    println!("{}", artifact.source);
}
