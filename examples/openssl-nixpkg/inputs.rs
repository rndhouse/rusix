//! Finite view of the pinned public package interface.
use rusnix_ir as rusnix;

#[rusnix::args]
#[allow(dead_code)] // Upstream retains unused coreutils/writeShellScript parameters.
pub mod args {
    use rusnix_ir::interop::{NixCallable, NixLibrary, NixNullable, NixValue, Package, Stdenv};

    /// Caller-owned dependencies and lazy policy arguments.
    #[rusnix(root)]
    struct Inputs {
        /// Pinned `lib` argument; resolved by Nix when demanded.
        lib: NixLibrary,
        /// Pinned `stdenv` argument; resolved by Nix when demanded.
        stdenv: Stdenv,
        /// Pinned `fetchurl` argument; resolved by Nix when demanded.
        fetchurl: NixCallable<Package>,
        /// Pinned `buildPackages` argument; resolved by Nix when demanded.
        build_packages: NixValue,
        /// Pinned `perl` argument; resolved by Nix when demanded.
        perl: Package,
        /// Pinned `coreutils` argument; resolved by Nix when demanded.
        #[allow(dead_code)]
        coreutils: Package,
        /// Pinned `writeShellScript` argument; resolved by Nix when demanded.
        #[allow(dead_code)]
        write_shell_script: NixValue,
        /// Pinned `makeBinaryWrapper` argument; resolved by Nix when demanded.
        make_binary_wrapper: Package,
        /// Pinned `withCryptodev` argument; resolved by Nix when demanded.
        with_cryptodev: bool,
        /// Pinned `cryptodev` argument; resolved by Nix when demanded.
        cryptodev: Package,
        /// Pinned `withZlib` argument; resolved by Nix when demanded.
        with_zlib: bool,
        /// Pinned `zlib` argument; resolved by Nix when demanded.
        zlib: Package,
        /// Pinned `enableSSL2` argument; resolved by Nix when demanded.
        #[rusnix(rename = "enableSSL2")]
        enable_ssl2: bool,
        /// Pinned `enableSSL3` argument; resolved by Nix when demanded.
        #[rusnix(rename = "enableSSL3")]
        enable_ssl3: bool,
        /// Pinned `enableMD2` argument; resolved by Nix when demanded.
        #[rusnix(rename = "enableMD2")]
        enable_md2: bool,
        /// Pinned `enableKTLS` argument; resolved by Nix when demanded.
        #[rusnix(rename = "enableKTLS")]
        enable_ktls: bool,
        /// Pinned `static` argument; resolved by Nix when demanded.
        #[rusnix(rename = "static")]
        static_build: bool,
        /// Pinned `conf` argument; resolved by Nix when demanded.
        conf: NixNullable<NixValue>,
        /// Pinned `removeReferencesTo` argument; resolved by Nix when demanded.
        remove_references_to: Package,
        /// Pinned `testers` argument; resolved by Nix when demanded.
        testers: NixValue,
    }
}

pub use args::Inputs;
