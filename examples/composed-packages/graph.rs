//! Explicit Rust-authored edges; all other dependencies stay in pinned nixpkgs.
#[path = "../openssl-nixpkg/mod.rs"]
pub mod openssl;

#[path = "../curl-nixpkg/mod.rs"]
pub mod curl;

#[path = "../git-nixpkg/mod.rs"]
pub mod git;

use rusnix_ir::interop::{NixValue, Nixpkgs};

pub fn arguments() -> NixValue {
    NixValue::record([] as [(&str, NixValue); 0])
}

pub fn graph() -> NixValue {
    let pkgs = Nixpkgs::new();
    let openssl = pkgs.call_package(
        &openssl::factory(openssl::model::Release::Preview),
        arguments(),
    );
    with_openssl(openssl)
}

/// Accepting an ordinary package value keeps the replacement boundary movable.
pub fn with_openssl(openssl: NixValue) -> NixValue {
    let pkgs = Nixpkgs::new();
    // Existing callbacks supply lazy lexical bindings, sharing each dependency once.
    NixValue::function(|openssl| {
        let curl = pkgs.call_package(
            &curl::factory(),
            NixValue::record([("openssl", openssl.clone())]),
        );
        let git = pkgs.call_package(
            &git::factory(),
            git::arguments().merge_attrs(NixValue::record([("openssl", openssl.clone())])),
        );
        NixValue::function(|curl| {
            NixValue::record([("openssl", openssl), ("curl", curl), ("git", git)])
        })
        .call(curl)
    })
    .call(openssl)
}
