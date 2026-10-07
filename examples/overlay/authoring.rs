//! Customizes nixpkgs' curl package by appending a configure flag that disables the DICT protocol.
//! An overlay returns replacement package definitions while reusing the rest of the package set.
use super::{
    inputs::{BuildAttrs, Packages},
    model::{Changes, ConfigureChanges},
};
use rusnix_ir::interop::{NixList, Nixpkgs, Overlay};

/// Describes an overlay using the declared package view and ordinary Rust result structs.
/// In Nix, prev is the package set before this overlay and final includes all overlays.
/// Rust constructs each callback once; Nix applies it when a customized package is needed.
pub fn overlay() -> Overlay {
    Overlay::try_from_function(|_final_pkgs: Packages, prev: Packages| {
        let curl = prev
            .curl()
            .try_override_attrs(|old: BuildAttrs| ConfigureChanges {
                configure_flags: NixList::concat([
                    old.configure_flags(),
                    NixList::new(["--disable-dict".into()]),
                ]),
            })
            .expect("the configure-flags replacement is a valid Rust record");

        Changes { curl }
    })
    .expect("the overlay changes are a valid Rust record")
}

/// Describes nixpkgs with the overlay applied, preserving the rest of the package set.
/// The returned handle stays deferred: Rust does not evaluate the package set or build curl.
pub fn package_set() -> Nixpkgs {
    Nixpkgs::new().with_overlay(overlay())
}
