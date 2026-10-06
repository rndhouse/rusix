//! Explicit Rust-authored edges; all other dependencies stay in pinned nixpkgs.
#[path = "../openssl-nixpkg/mod.rs"]
pub mod openssl;

#[path = "../curl-nixpkg/mod.rs"]
pub mod curl;

#[path = "../git-nixpkg/mod.rs"]
pub mod git;

#[path = "../mariadb-nixpkg/mod.rs"]
pub mod mariadb;

use rusnix_ir::interop::{NixValue, Nixpkgs};
use rusnix_ir::nix_record;

pub fn arguments() -> NixValue {
    nix_record! {}
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
    compose(
        openssl,
        |pkgs, openssl| pkgs.call_package(&curl::factory(), curl_arguments(pkgs, openssl.clone())),
        mariadb::model::Release::V1011.arguments(),
    )
}

/// Match the normal pkgs.curl flavour selected by MariaDB, rather than curlMinimal.
pub fn curl_arguments(pkgs: &Nixpkgs, openssl: NixValue) -> NixValue {
    nix_record! {
        "openssl": openssl,
        "idnSupport": true,
        "pslSupport": true,
        "zstdSupport": true,
    }
    .merge_attrs(NixValue::if_else(
        !pkgs.value("stdenv.hostPlatform.isStatic"),
        nix_record! { "brotliSupport": true },
        arguments(),
    ))
}

/// Ordinary Rust callbacks allow replacing either side of the authoring boundary.
pub fn compose(
    openssl: NixValue,
    make_curl: impl FnOnce(&Nixpkgs, &NixValue) -> NixValue,
    mariadb_arguments: NixValue,
) -> NixValue {
    let pkgs = Nixpkgs::new();
    // Existing callbacks supply lazy lexical bindings, sharing each dependency once.
    NixValue::function(|openssl| {
        let curl = make_curl(&pkgs, &openssl);
        let git = pkgs.call_package(
            &git::factory(),
            git::arguments().merge_attrs(nix_record! { "openssl": openssl.clone() }),
        );
        NixValue::function(|curl| {
            let mariadb = pkgs.call_package(
                &mariadb::factory(),
                mariadb_arguments.merge_attrs(nix_record! { "curl": curl.clone() }),
            );
            nix_record! {
                "openssl": openssl,
                "curl": curl,
                "git": git,
                "mariadb": mariadb,
            }
        })
        .call(curl)
    })
    .call(openssl)
}
