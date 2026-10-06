//! Finite view of MariaDB generic arguments; dependencies stay caller-owned.
use rusnix_ir as rusnix;

#[rusnix::args]
pub mod args {
    use rusnix_ir::interop::NixValue;

    /// Deferred common and server dependencies plus the four upstream feature defaults.
    #[rusnix(root)]
    struct Inputs {
        /// Pinned `version` argument, evaluated only as demanded by Nix.
        version: NixValue,
        /// Pinned `hash` argument, evaluated only as demanded by Nix.
        hash: NixValue,
        /// Pinned `lib` argument, evaluated only as demanded by Nix.
        lib: NixValue,
        /// Pinned `stdenv` argument, evaluated only as demanded by Nix.
        stdenv: NixValue,
        /// Pinned `fetchurl` argument, evaluated only as demanded by Nix.
        fetchurl: NixValue,
        /// Pinned `nixosTests` argument, evaluated only as demanded by Nix.
        nixos_tests: NixValue,
        /// Pinned `buildPackages` argument, evaluated only as demanded by Nix.
        build_packages: NixValue,
        /// Pinned `bison` argument, evaluated only as demanded by Nix.
        bison: NixValue,
        /// Pinned `boost` argument, evaluated only as demanded by Nix.
        boost: NixValue,
        /// Pinned `cmake` argument, evaluated only as demanded by Nix.
        cmake: NixValue,
        /// Pinned `fixDarwinDylibNames` argument, evaluated only as demanded by Nix.
        fix_darwin_dylib_names: NixValue,
        /// Pinned `flex` argument, evaluated only as demanded by Nix.
        flex: NixValue,
        /// Pinned `makeWrapper` argument, evaluated only as demanded by Nix.
        make_wrapper: NixValue,
        /// Pinned `pkg-config` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "pkg-config")]
        pkg_config: NixValue,
        /// Pinned `curl` argument, evaluated only as demanded by Nix.
        curl: NixValue,
        /// Pinned `libiconv` argument, evaluated only as demanded by Nix.
        libiconv: NixValue,
        /// Pinned `ncurses` argument, evaluated only as demanded by Nix.
        ncurses: NixValue,
        /// Pinned `openssl` argument, evaluated only as demanded by Nix.
        openssl: NixValue,
        /// Pinned `pcre2` argument, evaluated only as demanded by Nix.
        pcre2: NixValue,
        /// Pinned `libkrb5` argument, evaluated only as demanded by Nix.
        libkrb5: NixValue,
        /// Pinned `libaio` argument, evaluated only as demanded by Nix.
        libaio: NixValue,
        /// Pinned `liburing` argument, evaluated only as demanded by Nix.
        liburing: NixValue,
        /// Pinned `systemd` argument, evaluated only as demanded by Nix.
        systemd: NixValue,
        /// Pinned `CoreServices` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "CoreServices")]
        core_services: NixValue,
        /// Pinned `cctools` argument, evaluated only as demanded by Nix.
        cctools: NixValue,
        /// Pinned `perl` argument, evaluated only as demanded by Nix.
        perl: NixValue,
        /// Pinned `jemalloc` argument, evaluated only as demanded by Nix.
        jemalloc: NixValue,
        /// Pinned `less` argument, evaluated only as demanded by Nix.
        less: NixValue,
        /// Pinned `libedit` argument, evaluated only as demanded by Nix.
        libedit: NixValue,
        /// Pinned `bzip2` argument, evaluated only as demanded by Nix.
        bzip2: NixValue,
        /// Pinned `lz4` argument, evaluated only as demanded by Nix.
        lz4: NixValue,
        /// Pinned `lzo` argument, evaluated only as demanded by Nix.
        lzo: NixValue,
        /// Pinned `snappy` argument, evaluated only as demanded by Nix.
        snappy: NixValue,
        /// Pinned `xz` argument, evaluated only as demanded by Nix.
        xz: NixValue,
        /// Pinned `zlib` argument, evaluated only as demanded by Nix.
        zlib: NixValue,
        /// Pinned `zstd` argument, evaluated only as demanded by Nix.
        zstd: NixValue,
        /// Pinned `cracklib` argument, evaluated only as demanded by Nix.
        cracklib: NixValue,
        /// Pinned `judy` argument, evaluated only as demanded by Nix.
        judy: NixValue,
        /// Pinned `libevent` argument, evaluated only as demanded by Nix.
        libevent: NixValue,
        /// Pinned `libxml2` argument, evaluated only as demanded by Nix.
        libxml2: NixValue,
        /// Pinned `linux-pam` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "linux-pam")]
        linux_pam: NixValue,
        /// Pinned `numactl` argument, evaluated only as demanded by Nix.
        numactl: NixValue,
        /// Pinned `fmt_8` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "fmt_8")]
        fmt_8: NixValue,
        /// Pinned `withStorageMroonga` argument, evaluated only as demanded by Nix.
        with_storage_mroonga: bool,
        /// Pinned `kytea` argument, evaluated only as demanded by Nix.
        kytea: NixValue,
        /// Pinned `libsodium` argument, evaluated only as demanded by Nix.
        libsodium: NixValue,
        /// Pinned `msgpack` argument, evaluated only as demanded by Nix.
        msgpack: NixValue,
        /// Pinned `zeromq` argument, evaluated only as demanded by Nix.
        zeromq: NixValue,
        /// Pinned `withStorageRocks` argument, evaluated only as demanded by Nix.
        with_storage_rocks: bool,
        /// Pinned `withEmbedded` argument, evaluated only as demanded by Nix.
        with_embedded: bool,
        /// Pinned `withNuma` argument, evaluated only as demanded by Nix.
        with_numa: bool,
    }
}

pub use args::Inputs;
