//! Finite symbolic access to the pinned package arguments; Rust never reads their values.
use rusnix_ir as rusnix;

/// Exact public argument names; names without defaults remain required in Nix.
pub(super) const ARGUMENTS: &[&str] = &[
    "lib",
    "stdenv",
    "fetchurl",
    "darwin",
    "pkg-config",
    "perl",
    "nixosTests",
    "brotliSupport",
    "brotli",
    "c-aresSupport",
    "c-aresMinimal",
    "gnutlsSupport",
    "gnutls",
    "gsaslSupport",
    "gsasl",
    "gssSupport",
    "libkrb5",
    "http2Support",
    "nghttp2",
    "http3Support",
    "nghttp3",
    "ngtcp2",
    "websocketSupport",
    "idnSupport",
    "libidn2",
    "ldapSupport",
    "openldap",
    "opensslSupport",
    "openssl",
    "pslSupport",
    "libpsl",
    "rtmpSupport",
    "rtmpdump",
    "scpSupport",
    "libssh2",
    "wolfsslSupport",
    "wolfssl",
    "rustlsSupport",
    "rustls-ffi",
    "zlibSupport",
    "zlib",
    "zstdSupport",
    "zstd",
    "coeurl",
    "curlpp",
    "haskellPackages",
    "ocamlPackages",
    "phpExtensions",
    "pkgsStatic",
    "python3",
    "tests",
    "testers",
    "fetchpatch",
];

/// Local views describe only the external Nix values used by this compatibility adapter.
#[rusnix::args]
pub(super) mod args {
    use rusnix_ir::interop::NixValue;

    /// Deferred package dependencies and features, resolved by callPackage or an ordinary caller.
    #[rusnix(root)]
    struct Inputs {
        /// Utility functions and metadata from this exact package caller.
        lib: Lib,
        /// Standard builder, compiler, and build/host platform records.
        stdenv: Stdenv,
        /// Bootstrap source fetcher; constructing its derivation does not fetch.
        fetchurl: NixValue,
        /// Apple framework packages used by the Darwin dependency branch.
        darwin: Darwin,
        /// Build-platform tool that supplies dependency flags.
        #[rusnix(rename = "pkg-config")]
        pkg_config: NixValue,
        /// Build-platform interpreter used by curl scripts.
        perl: NixValue,
        /// Existing NixOS tests retained as passthru references.
        nixos_tests: NixosTests,
        /// Enables Brotli response decompression.
        brotli_support: bool,
        /// Brotli library propagated when that decompression feature is selected.
        brotli: NixValue,
        /// Selects asynchronous DNS resolution through c-ares.
        #[rusnix(rename = "c-aresSupport")]
        c_ares_support: bool,
        /// Minimal c-ares package used by the asynchronous DNS branch.
        #[rusnix(rename = "c-aresMinimal")]
        c_ares_minimal: NixValue,
        /// Selects GnuTLS; the native Nix assertion excludes other simultaneous TLS backends.
        gnutls_support: bool,
        /// GnuTLS package used for encrypted connections and compatibility library links.
        gnutls: NixValue,
        /// Enables authentication through GNU SASL.
        gsasl_support: bool,
        /// GNU SASL implementation propagated when its authentication feature is selected.
        gsasl: NixValue,
        /// Selects GSS authentication; its default depends on platform and cross-build state.
        gss_support: bool,
        /// Kerberos implementation and development output used by GSS authentication.
        libkrb5: NixValue,
        /// Enables the HTTP/2 library and defaults to true in Nix.
        http2_support: bool,
        /// HTTP/2 library propagated to libcurl consumers.
        nghttp2: NixValue,
        /// Enables both HTTP/3 and QUIC dependencies and their configure switches.
        http3_support: bool,
        /// HTTP/3 protocol library selected by http3Support.
        nghttp3: NixValue,
        /// QUIC transport library selected alongside nghttp3.
        ngtcp2: NixValue,
        /// Enables curl's experimental websocket protocol support.
        websocket_support: bool,
        /// Enables internationalized domain-name handling.
        idn_support: bool,
        /// Internationalized domain-name library and configure-time development output.
        libidn2: NixValue,
        /// Enables both LDAP and LDAPS protocol configure switches.
        ldap_support: bool,
        /// LDAP library propagated when LDAP support is enabled.
        openldap: NixValue,
        /// Selects OpenSSL; its lazy default follows zlibSupport.
        openssl_support: bool,
        /// Caller-selected OpenSSL package, also exposed unchanged in passthru.
        openssl: NixValue,
        /// Enables public-suffix handling, including the static linker workaround.
        psl_support: bool,
        /// Public suffix library propagated when PSL support is enabled.
        libpsl: NixValue,
        /// Enables RTMP streaming protocol support.
        rtmp_support: bool,
        /// RTMP implementation selected by the librtmp configure switch.
        rtmpdump: NixValue,
        /// Selects SCP; its default follows zlib and excludes SunOS/Cygwin.
        scp_support: bool,
        /// SSH library supplying SCP support and separate development/library outputs.
        libssh2: NixValue,
        /// Selects wolfSSL as one of the mutually exclusive TLS backends.
        wolfssl_support: bool,
        /// Caller-selected wolfSSL implementation and development output.
        wolfssl: NixValue,
        /// Selects Rustls and the platform-specific CA-bundle configure switch.
        rustls_support: bool,
        /// Existing Rustls FFI package; its implementation stays in nixpkgs.
        #[rusnix(rename = "rustls-ffi")]
        rustls_ffi: NixValue,
        /// Enables zlib and controls the dependent OpenSSL/SCP defaults.
        zlib_support: bool,
        /// Compression library propagated when zlib support is selected.
        zlib: NixValue,
        /// Enables Zstandard response decompression.
        zstd_support: bool,
        /// Zstandard library propagated when that decompression feature is selected.
        zstd: NixValue,
        /// Existing consuming package overridden to use the eventual curl in a passthru check.
        coeurl: NixValue,
        /// Existing C++ curl wrapper overridden to consume the eventual package.
        curlpp: NixValue,
        /// Package scope containing the Haskell curl binding checked recursively.
        haskell_packages: HaskellPackages,
        /// Package scope containing the OCaml curly binding checked recursively.
        ocaml_packages: OcamlPackages,
        /// Extension scope containing the PHP curl binding checked recursively.
        php_extensions: PhpExtensions,
        /// Static package scope supplying the pinned static-curl passthru reference.
        pkgs_static: StaticPackages,
        /// Python package scope containing the pycurl binding checked recursively.
        python3: Python,
        /// nixpkgs regression-test scope containing the fetchpatch consumer check.
        tests: Tests,
        /// nixpkgs test helpers used to validate the final package's pkg-config metadata.
        testers: Testers,
        /// Required passthru dependency; never used to fetch curl source or patches.
        fetchpatch: NixValue,
    }

    /// Existing builder and the finite compiler/platform properties this recipe needs.
    /// Whole-subtree access retains the entire supplied stdenv for build/host equality.
    #[rusnix(value)]
    struct Stdenv {
        /// Real mkDerivation function, including recursive finalAttrs and overrides.
        mk_derivation: NixValue,
        /// Platform where the resulting curl runs.
        host_platform: Platform,
        /// Compiler metadata for the explicit C++ command names.
        cc: Compiler,
    }

    /// Known platform predicates; this is not a generated global nixpkgs platform schema.
    #[rusnix(value)]
    struct Platform {
        /// Adds Darwin frameworks and platform-specific scripts/configure flags.
        is_darwin: bool,
        /// Enables stdenv's separate debug output.
        is_linux: bool,
        /// Disables the default GSS feature on Windows targets.
        is_windows: bool,
        /// Controls static linker workarounds and broken-package metadata.
        is_static: bool,
        /// Disables the default SCP feature on SunOS.
        #[rusnix(rename = "isSunOS")]
        is_sun_os: bool,
        /// Disables the default SCP feature on Cygwin.
        is_cygwin: bool,
        /// Excludes an upstream test with different resolver behavior.
        is_musl: bool,
        /// Target-specific filename suffixes supplied by nixpkgs.
        extensions: Extensions,
    }

    /// Only the shared-library suffix is required by the GnuTLS compatibility links.
    struct Extensions {
        /// Includes the target's leading dot, such as .so or .dylib.
        shared_library: String,
    }

    /// Compiler command naming remains controlled by stdenv.
    struct Compiler {
        /// Prefix before c++/c++ -E, including cross-toolchain names.
        target_prefix: String,
    }

    /// Caller-supplied library dispatch and package metadata.
    #[rusnix(value)]
    struct Lib {
        /// The pinned package license record from nixpkgs.
        licenses: Licenses,
        /// Existing maintainer records, not Rust-maintained contact data.
        maintainers: Maintainers,
        /// Existing supported-platform lists.
        platforms: Platforms,
    }

    /// License metadata needed by curl.
    struct Licenses {
        /// curl's permissive license as described by the caller's library.
        curl: NixValue,
    }

    /// The upstream maintainer identity stays opaque.
    struct Maintainers {
        /// Maintainer listed by the pinned package.
        lovek323: NixValue,
    }

    /// Platform metadata stays owned by nixpkgs.
    struct Platforms {
        /// The same complete platform list used by upstream.
        all: NixValue,
    }

    /// Darwin package hierarchy used for framework dependencies.
    struct Darwin {
        /// The SDK selected by this package caller.
        #[rusnix(rename = "apple_sdk")]
        apple_sdk: AppleSdk,
    }

    /// Only these SDK frameworks are required by curl.
    struct AppleSdk {
        /// Three propagated dependencies needed on Darwin.
        frameworks: Frameworks,
    }

    /// Framework packages remain Nix values rather than Rust SDK bindings.
    struct Frameworks {
        /// Core Foundation dependency.
        #[rusnix(rename = "CoreFoundation")]
        core_foundation: NixValue,
        /// Core Services dependency.
        #[rusnix(rename = "CoreServices")]
        core_services: NixValue,
        /// System configuration dependency.
        #[rusnix(rename = "SystemConfiguration")]
        system_configuration: NixValue,
    }

    /// The HTTP/3 NixOS test is referenced, never executed by this example.
    struct NixosTests {
        /// Upstream cannot override this test; retain the original test unchanged.
        #[rusnix(rename = "nginx-http3")]
        nginx_http3: NixValue,
    }

    /// Haskell package that tests consuming the final curl.
    struct HaskellPackages {
        /// Haskell curl binding, overridden with the final package in passthru.
        curl: NixValue,
    }

    /// OCaml package that tests consuming the final curl.
    struct OcamlPackages {
        /// OCaml curly binding, overridden with the final package in passthru.
        curly: NixValue,
    }

    /// PHP extension that tests consuming the final curl.
    struct PhpExtensions {
        /// PHP curl extension, overridden with the final package in passthru.
        curl: NixValue,
    }

    /// Separately maintained static package referenced by upstream's tests.
    struct StaticPackages {
        /// Existing static curl; it is not replaced with the package being authored.
        curl: NixValue,
    }

    /// Python packages used by curl's consuming-package test.
    struct Python {
        /// Packages associated with the caller's Python interpreter.
        pkgs: PythonPackages,
    }

    /// Only pycurl is needed from this package set.
    struct PythonPackages {
        /// Python binding overridden to use finalAttrs.finalPackage.
        pycurl: NixValue,
    }

    /// Fetcher test hierarchy supplied by the caller.
    struct Tests {
        /// Existing fetchpatch test variants.
        fetchpatch: FetchpatchTests,
    }

    /// Only the simple fetchpatch test participates in curl's passthru.
    struct FetchpatchTests {
        /// Test overridden with a fetchpatch that uses this curl through fetchurl.
        simple: NixValue,
    }

    /// Generic package test constructors remain opaque functions in nixpkgs.
    struct Testers {
        /// Checks libcurl's pkg-config metadata against finalAttrs.finalPackage.
        test_meta_pkg_config: NixValue,
    }
}

pub(super) use args::Inputs;
