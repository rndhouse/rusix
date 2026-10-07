//! Finite shared views demonstrated by several independent package definitions.
//! The complete underlying records remain authoritative and are never reconstructed.
#[crate::args]
mod views {
    use crate::Expr;
    use crate::interop::{NixCallable, Package};

    #[rusnix(root)]
    struct Inputs {
        platform: Platform,
        final_attrs: FinalAttrs,
    }

    /// Finite nixpkgs platform properties used by OpenSSL, curl, Git and MariaDB.
    /// Fields are lazy expectations; no architecture or platform schema is recreated.
    #[rusnix(value)]
    struct Platform {
        system: String,
        config: String,
        libc: String,
        is_linux: bool,
        is_darwin: bool,
        is_windows: bool,
        is_static: bool,
        is_musl: bool,
        is_cygwin: bool,
        #[rusnix(rename = "isFreeBSD")]
        is_free_bsd: bool,
        #[rusnix(rename = "isOpenBSD")]
        is_open_bsd: bool,
        #[rusnix(rename = "isSunOS")]
        is_sun_os: bool,
        #[rusnix(rename = "isBSD")]
        is_bsd: bool,
        #[rusnix(rename = "isMinGW")]
        is_min_gw: bool,
        #[rusnix(rename = "isiOS")]
        is_ios: bool,
        #[rusnix(rename = "isElf")]
        is_elf: bool,
        #[rusnix(rename = "isAarch64")]
        is_aarch64: bool,
        #[rusnix(rename = "isi686")]
        is_i686: bool,
        #[rusnix(rename = "isx86_64")]
        is_x86_64: bool,
        #[rusnix(rename = "isx86_32")]
        is_x86_32: bool,
        #[rusnix(rename = "isMips")]
        is_mips: bool,
        #[rusnix(rename = "isMips32")]
        is_mips32: bool,
        #[rusnix(rename = "isMips64n32")]
        is_mips64n32: bool,
        #[rusnix(rename = "isMips64n64")]
        is_mips64n64: bool,
        #[rusnix(rename = "isMicroBlaze")]
        is_micro_blaze: bool,
        parsed: Parsed,
        extensions: Extensions,
        gcc: Gcc,
        emulator: NixCallable<Expr<String>>,
    }

    struct Parsed {
        cpu: Cpu,
    }

    struct Cpu {
        bits: i64,
    }

    struct Extensions {
        shared_library: String,
    }

    #[rusnix(value)]
    struct Gcc {
        arch: String,
    }

    /// Shared fields in stdenv's lazy recursive finalAttrs argument.
    /// Other fields remain accessible explicitly through as_attrs.
    #[rusnix(value)]
    struct FinalAttrs {
        final_package: Package,
        version: String,
    }
}

pub use views::{FinalAttrs, Platform};
