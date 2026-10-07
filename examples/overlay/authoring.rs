//! Customizes nixpkgs' curl package by appending a configure flag that disables the DICT protocol.
//! An overlay returns replacement package definitions while reusing the rest of the package set.
use rusnix_ir::{
    Expr,
    interop::{NixAttrs, NixList, Nixpkgs, Overlay, Package},
};

/// Describes an overlay that appends `--disable-dict` to curl's existing configure flags.
/// In Nix, `prev` is the package set before this overlay and `final` includes all overlays.
/// Rust constructs the callback once; Nix applies it when a customized package is needed.
pub fn overlay() -> Overlay {
    Overlay::from_function(|_final_pkgs, prev_pkgs| {
        // Start from the previous curl recipe; old supplies its current build attributes.
        let curl: Package = prev_pkgs.field("curl");
        let curl = curl.override_attrs(|old| {
            let flags: NixList<Expr<String>> = old.field("configureFlags");
            let flags = NixList::concat([flags, NixList::new(["--disable-dict".into()])]);
            NixAttrs::new([("configureFlags", flags.into())])
        });
        NixAttrs::new([("curl", curl.into())])
    })
}

/// Describes nixpkgs with the overlay applied, so `get("curl")` selects the modified recipe.
/// The returned handle stays deferred: Rust does not evaluate the package set or build curl.
pub fn package_set() -> Nixpkgs {
    Nixpkgs::new().with_overlay(overlay())
}
