//! Finite view of MariaDB generic arguments; dependencies stay caller-owned.
use rusnix_ir as rusnix;

#[rusnix::args]
pub mod args {
    use rusnix_ir::interop::{NixCallable, NixLibrary, Package, Stdenv, raw::NixValue};

    /// Deferred common and server dependencies plus the four upstream feature defaults.
    #[rusnix(root)]
    struct Inputs {
        /// Pinned `version` argument, evaluated only as demanded by Nix.
        version: String,
        /// Pinned `hash` argument, evaluated only as demanded by Nix.
        hash: String,
        /// Pinned `lib` argument, evaluated only as demanded by Nix.
        lib: NixLibrary,
        /// Pinned `stdenv` argument, evaluated only as demanded by Nix.
        stdenv: Stdenv,
        /// Pinned `fetchurl` argument, evaluated only as demanded by Nix.
        fetchurl: NixCallable<Package>,
        /// Pinned `nixosTests` argument, evaluated only as demanded by Nix.
        nixos_tests: NixValue,
        /// Pinned `buildPackages` argument, evaluated only as demanded by Nix.
        build_packages: NixValue,
        /// Pinned `bison` argument, evaluated only as demanded by Nix.
        bison: Package,
        /// Pinned `boost` argument, evaluated only as demanded by Nix.
        boost: Package,
        /// Pinned `cmake` argument, evaluated only as demanded by Nix.
        cmake: Package,
        /// Pinned `fixDarwinDylibNames` argument, evaluated only as demanded by Nix.
        fix_darwin_dylib_names: Package,
        /// Pinned `flex` argument, evaluated only as demanded by Nix.
        flex: Package,
        /// Pinned `makeWrapper` argument, evaluated only as demanded by Nix.
        make_wrapper: Package,
        /// Pinned `pkg-config` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "pkg-config")]
        pkg_config: Package,
        /// Pinned `curl` argument, evaluated only as demanded by Nix.
        curl: Package,
        /// Pinned `libiconv` argument, evaluated only as demanded by Nix.
        libiconv: Package,
        /// Pinned `ncurses` argument, evaluated only as demanded by Nix.
        ncurses: Package,
        /// Pinned `openssl` argument, evaluated only as demanded by Nix.
        openssl: Package,
        /// Pinned `pcre2` argument, evaluated only as demanded by Nix.
        pcre2: Package,
        /// Pinned `libkrb5` argument, evaluated only as demanded by Nix.
        libkrb5: Package,
        /// Pinned `libaio` argument, evaluated only as demanded by Nix.
        libaio: Package,
        /// Pinned `liburing` argument, evaluated only as demanded by Nix.
        liburing: Package,
        /// Pinned `systemd` argument, evaluated only as demanded by Nix.
        systemd: Package,
        /// Pinned `CoreServices` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "CoreServices")]
        core_services: Package,
        /// Pinned `cctools` argument, evaluated only as demanded by Nix.
        cctools: Package,
        /// Pinned `perl` argument, evaluated only as demanded by Nix.
        perl: Package,
        /// Pinned `jemalloc` argument, evaluated only as demanded by Nix.
        jemalloc: Package,
        /// Pinned `less` argument, evaluated only as demanded by Nix.
        less: Package,
        /// Pinned `libedit` argument, evaluated only as demanded by Nix.
        libedit: Package,
        /// Pinned `bzip2` argument, evaluated only as demanded by Nix.
        bzip2: Package,
        /// Pinned `lz4` argument, evaluated only as demanded by Nix.
        lz4: Package,
        /// Pinned `lzo` argument, evaluated only as demanded by Nix.
        lzo: Package,
        /// Pinned `snappy` argument, evaluated only as demanded by Nix.
        snappy: Package,
        /// Pinned `xz` argument, evaluated only as demanded by Nix.
        xz: Package,
        /// Pinned `zlib` argument, evaluated only as demanded by Nix.
        zlib: Package,
        /// Pinned `zstd` argument, evaluated only as demanded by Nix.
        zstd: Package,
        /// Pinned `cracklib` argument, evaluated only as demanded by Nix.
        cracklib: Package,
        /// Pinned `judy` argument, evaluated only as demanded by Nix.
        judy: Package,
        /// Pinned `libevent` argument, evaluated only as demanded by Nix.
        libevent: Package,
        /// Pinned `libxml2` argument, evaluated only as demanded by Nix.
        libxml2: Package,
        /// Pinned `linux-pam` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "linux-pam")]
        linux_pam: Package,
        /// Pinned `numactl` argument, evaluated only as demanded by Nix.
        numactl: Package,
        /// Pinned `fmt_8` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "fmt_8")]
        fmt_8: Package,
        /// Pinned `withStorageMroonga` argument, evaluated only as demanded by Nix.
        with_storage_mroonga: bool,
        /// Pinned `kytea` argument, evaluated only as demanded by Nix.
        kytea: Package,
        /// Pinned `libsodium` argument, evaluated only as demanded by Nix.
        libsodium: Package,
        /// Pinned `msgpack` argument, evaluated only as demanded by Nix.
        msgpack: Package,
        /// Pinned `zeromq` argument, evaluated only as demanded by Nix.
        zeromq: Package,
        /// Pinned `withStorageRocks` argument, evaluated only as demanded by Nix.
        with_storage_rocks: bool,
        /// Pinned `withEmbedded` argument, evaluated only as demanded by Nix.
        with_embedded: bool,
        /// Pinned `withNuma` argument, evaluated only as demanded by Nix.
        with_numa: bool,
    }
}

pub use args::Inputs;
