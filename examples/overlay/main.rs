//! Print a Rust-authored overlay and the lookup of curl from its extended package set.
mod authoring;

mod inputs;

mod model;

fn main() {
    // An overlay replaces selected packages while keeping the rest of nixpkgs.
    // These views describe deferred lookups; Rust does not load the packages.
    let pkgs = authoring::package_set().view::<inputs::Packages>();
    let curl_metadata = pkgs.curl().view::<inputs::PackageMetadata>();

    // Export the overlay function and the customized curl build-recipe path.
    // Nix can compute that recipe path without building curl.
    let output = model::Output {
        overlay: authoring::overlay(),
        curl_derivation: curl_metadata.drv_path(),
    };

    // Generate and print Nix source. Nix evaluation is a separate step.
    let artifact = rusnix_nix::compile(output).expect("the overlay has valid structural values");
    println!("{}", artifact.source);
}
