//! Finite view of MariaDB generic arguments; dependencies stay caller-owned.
use rusnix_ir as rusnix;

pub const ARGUMENTS: &[&str] = &[
    "version",
    "hash",
    "lib",
    "stdenv",
    "fetchurl",
    "nixosTests",
    "buildPackages",
    "bison",
    "boost",
    "cmake",
    "fixDarwinDylibNames",
    "flex",
    "makeWrapper",
    "pkg-config",
    "curl",
    "libiconv",
    "ncurses",
    "openssl",
    "pcre2",
    "libkrb5",
    "libaio",
    "liburing",
    "systemd",
    "CoreServices",
    "cctools",
    "perl",
    "jemalloc",
    "less",
    "libedit",
    "bzip2",
    "lz4",
    "lzo",
    "snappy",
    "xz",
    "zlib",
    "zstd",
    "cracklib",
    "judy",
    "libevent",
    "libxml2",
    "linux-pam",
    "numactl",
    "fmt_8",
    "withStorageMroonga",
    "kytea",
    "libsodium",
    "msgpack",
    "zeromq",
    "withStorageRocks",
    "withEmbedded",
    "withNuma",
];

#[rusnix::args]
pub mod args {
    use rusnix_ir::interop::NixValue;

    /// Deferred common and server dependencies plus the four upstream feature defaults.
    #[rusnix(root)]
    struct Inputs {
        /// Pinned `version` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "version")]
        version: NixValue,
        /// Pinned `hash` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "hash")]
        hash: NixValue,
        /// Pinned `lib` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "lib")]
        lib: NixValue,
        /// Pinned `stdenv` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "stdenv")]
        stdenv: NixValue,
        /// Pinned `fetchurl` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "fetchurl")]
        fetchurl: NixValue,
        /// Pinned `nixosTests` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "nixosTests")]
        nixos_tests: NixValue,
        /// Pinned `buildPackages` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "buildPackages")]
        build_packages: NixValue,
        /// Pinned `bison` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "bison")]
        bison: NixValue,
        /// Pinned `boost` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "boost")]
        boost: NixValue,
        /// Pinned `cmake` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "cmake")]
        cmake: NixValue,
        /// Pinned `fixDarwinDylibNames` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "fixDarwinDylibNames")]
        fix_darwin_dylib_names: NixValue,
        /// Pinned `flex` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "flex")]
        flex: NixValue,
        /// Pinned `makeWrapper` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "makeWrapper")]
        make_wrapper: NixValue,
        /// Pinned `pkg-config` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "pkg-config")]
        pkg_config: NixValue,
        /// Pinned `curl` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "curl")]
        curl: NixValue,
        /// Pinned `libiconv` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "libiconv")]
        libiconv: NixValue,
        /// Pinned `ncurses` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "ncurses")]
        ncurses: NixValue,
        /// Pinned `openssl` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "openssl")]
        openssl: NixValue,
        /// Pinned `pcre2` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "pcre2")]
        pcre2: NixValue,
        /// Pinned `libkrb5` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "libkrb5")]
        libkrb5: NixValue,
        /// Pinned `libaio` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "libaio")]
        libaio: NixValue,
        /// Pinned `liburing` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "liburing")]
        liburing: NixValue,
        /// Pinned `systemd` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "systemd")]
        systemd: NixValue,
        /// Pinned `CoreServices` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "CoreServices")]
        core_services: NixValue,
        /// Pinned `cctools` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "cctools")]
        cctools: NixValue,
        /// Pinned `perl` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "perl")]
        perl: NixValue,
        /// Pinned `jemalloc` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "jemalloc")]
        jemalloc: NixValue,
        /// Pinned `less` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "less")]
        less: NixValue,
        /// Pinned `libedit` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "libedit")]
        libedit: NixValue,
        /// Pinned `bzip2` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "bzip2")]
        bzip2: NixValue,
        /// Pinned `lz4` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "lz4")]
        lz4: NixValue,
        /// Pinned `lzo` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "lzo")]
        lzo: NixValue,
        /// Pinned `snappy` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "snappy")]
        snappy: NixValue,
        /// Pinned `xz` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "xz")]
        xz: NixValue,
        /// Pinned `zlib` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "zlib")]
        zlib: NixValue,
        /// Pinned `zstd` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "zstd")]
        zstd: NixValue,
        /// Pinned `cracklib` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "cracklib")]
        cracklib: NixValue,
        /// Pinned `judy` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "judy")]
        judy: NixValue,
        /// Pinned `libevent` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "libevent")]
        libevent: NixValue,
        /// Pinned `libxml2` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "libxml2")]
        libxml2: NixValue,
        /// Pinned `linux-pam` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "linux-pam")]
        linux_pam: NixValue,
        /// Pinned `numactl` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "numactl")]
        numactl: NixValue,
        /// Pinned `fmt_8` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "fmt_8")]
        fmt_8: NixValue,
        /// Pinned `withStorageMroonga` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "withStorageMroonga")]
        with_storage_mroonga: bool,
        /// Pinned `kytea` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "kytea")]
        kytea: NixValue,
        /// Pinned `libsodium` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "libsodium")]
        libsodium: NixValue,
        /// Pinned `msgpack` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "msgpack")]
        msgpack: NixValue,
        /// Pinned `zeromq` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "zeromq")]
        zeromq: NixValue,
        /// Pinned `withStorageRocks` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "withStorageRocks")]
        with_storage_rocks: bool,
        /// Pinned `withEmbedded` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "withEmbedded")]
        with_embedded: bool,
        /// Pinned `withNuma` argument, evaluated only as demanded by Nix.
        #[rusnix(rename = "withNuma")]
        with_numa: bool,
    }
}

pub use args::Inputs;
