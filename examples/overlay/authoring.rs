//! Customizes nixpkgs' curl package by appending a configure flag that disables the DICT protocol.
//! An overlay returns replacement package definitions while reusing the rest of the package set.
use rusnix_ir::{
    interop::{
        Nixpkgs,
        raw::{NixFunctionExt, NixValue},
    },
    nix_record,
};

/// Describes an overlay that appends `--disable-dict` to curl's existing configure flags.
/// In Nix, `prev` is the package set before this overlay and `final` includes all overlays.
/// Rust callbacks construct those Nix functions; Nix applies them when the output is evaluated.
/// The generated shape is `final: prev: { curl = prev.curl.overrideAttrs (...); }`.
pub fn overlay() -> NixValue {
    NixValue::function(|_final_pkgs| {
        NixValue::function(|prev_pkgs| {
            // Start from the previous curl recipe; old supplies its current build attributes.
            let curl = prev_pkgs
                .select("curl")
                .override_attrs(NixValue::function(|old| {
                    let flags = NixValue::concat_lists([
                        old.select("configureFlags"),
                        NixValue::list(["--disable-dict".into()]),
                    ]);
                    nix_record! { "configureFlags": flags }
                }));
            nix_record! { "curl": curl }
        })
    })
}

/// Describes applying the overlay to nixpkgs so a later `curl` lookup selects the modified recipe.
/// The returned expression stays deferred: Rust does not evaluate the package set or build curl.
pub fn package_set() -> NixValue {
    Nixpkgs::new().pkgs_function("extend").call(overlay())
}
