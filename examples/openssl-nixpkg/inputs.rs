//! Finite view of the pinned public package interface.
use rusnix_ir as rusnix;

pub const ARGUMENTS: &[&str] = &[
    "lib",
    "stdenv",
    "fetchurl",
    "buildPackages",
    "perl",
    "coreutils",
    "writeShellScript",
    "makeBinaryWrapper",
    "withCryptodev",
    "cryptodev",
    "withZlib",
    "zlib",
    "enableSSL2",
    "enableSSL3",
    "enableMD2",
    "enableKTLS",
    "static",
    "conf",
    "removeReferencesTo",
    "testers",
];

#[rusnix::args]
#[allow(dead_code)] // Upstream retains unused coreutils/writeShellScript parameters.
pub mod args {
    use rusnix_ir::interop::NixValue;

    /// Caller-owned dependencies and lazy policy arguments.
    #[rusnix(root)]
    struct Inputs {
        /// Pinned `lib` argument; resolved by Nix when demanded.
        #[rusnix(rename = "lib")]
        lib: NixValue,
        /// Pinned `stdenv` argument; resolved by Nix when demanded.
        #[rusnix(rename = "stdenv")]
        stdenv: NixValue,
        /// Pinned `fetchurl` argument; resolved by Nix when demanded.
        #[rusnix(rename = "fetchurl")]
        fetchurl: NixValue,
        /// Pinned `buildPackages` argument; resolved by Nix when demanded.
        #[rusnix(rename = "buildPackages")]
        build_packages: NixValue,
        /// Pinned `perl` argument; resolved by Nix when demanded.
        #[rusnix(rename = "perl")]
        perl: NixValue,
        /// Pinned `coreutils` argument; resolved by Nix when demanded.
        #[rusnix(rename = "coreutils")]
        #[allow(dead_code)]
        coreutils: NixValue,
        /// Pinned `writeShellScript` argument; resolved by Nix when demanded.
        #[rusnix(rename = "writeShellScript")]
        #[allow(dead_code)]
        write_shell_script: NixValue,
        /// Pinned `makeBinaryWrapper` argument; resolved by Nix when demanded.
        #[rusnix(rename = "makeBinaryWrapper")]
        make_binary_wrapper: NixValue,
        /// Pinned `withCryptodev` argument; resolved by Nix when demanded.
        #[rusnix(rename = "withCryptodev")]
        with_cryptodev: bool,
        /// Pinned `cryptodev` argument; resolved by Nix when demanded.
        #[rusnix(rename = "cryptodev")]
        cryptodev: NixValue,
        /// Pinned `withZlib` argument; resolved by Nix when demanded.
        #[rusnix(rename = "withZlib")]
        with_zlib: bool,
        /// Pinned `zlib` argument; resolved by Nix when demanded.
        #[rusnix(rename = "zlib")]
        zlib: NixValue,
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
        #[rusnix(rename = "conf")]
        conf: NixValue,
        /// Pinned `removeReferencesTo` argument; resolved by Nix when demanded.
        #[rusnix(rename = "removeReferencesTo")]
        remove_references_to: NixValue,
        /// Pinned `testers` argument; resolved by Nix when demanded.
        #[rusnix(rename = "testers")]
        testers: NixValue,
    }
}

pub use args::Inputs;
