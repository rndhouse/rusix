//! Author an ordinary nixpkgs overlay; keep the upstream curl package definition.
use rusnix_ir::{
    interop::{
        Nixpkgs,
        raw::{NixFunctionExt, NixValue},
    },
    nix_record,
};

/// Equivalent to `final: prev: { curl = prev.curl.overrideAttrs (...); }`.
pub fn overlay() -> NixValue {
    NixValue::function(|_final_pkgs| {
        NixValue::function(|prev_pkgs| {
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

/// Let ordinary nixpkgs apply the overlay and evaluate its package-set fixed point.
pub fn package_set() -> NixValue {
    Nixpkgs::new().pkgs_function("extend").call(overlay())
}
