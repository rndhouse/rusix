//! Print a Rust-authored overlay and the lookup of curl from its extended package set.
mod authoring;

mod inputs;

mod model;

fn main() {
    let pkgs = authoring::package_set().view::<inputs::Packages>();
    let curl = pkgs.curl().view::<inputs::PackageMetadata>();
    let output = model::Output {
        overlay: authoring::overlay(),
        curl_derivation: curl.drv_path(),
    };
    let artifact = rusnix_nix::compile(output).expect("the overlay has valid structural values");
    println!("{}", artifact.source);
}
