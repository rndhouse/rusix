//! External dependencies stay opaque; the caller supplies the normal nixpkgs scope.
use rusnix_ir::{
    self as rusnix, IntoRusnixValue, RusnixValue,
    interop::{NixValue, Nixpkgs, Package},
};

/// Finite navigation over the external Nix arguments this package implementation uses.
#[rusnix::args]
pub(super) mod args {
    use rusnix_ir::{
        Expr,
        interop::{NixCallable, NixList, NixValue, Package},
    };

    /// Resolved arguments stay deferred; accessors record each lookup's Rust call site.
    #[rusnix(root)]
    struct Inputs {
        /// Source fetcher; Nix constructs the fixed-output derivation.
        fetchurl: NixCallable<Package>,
        /// Patch fetcher with nixpkgs normalization semantics.
        fetchpatch: NixCallable<Package>,
        /// Caller-supplied library functions and package metadata.
        lib: Lib,
        /// Builder and platform properties supplied by the caller.
        stdenv: Stdenv,
        /// Tools that run on the build platform, including cross builds.
        build_packages: BuildPackages,
        /// Opaque nixpkgs dependency; its internals remain authoritative in Nix.
        curl: Package,
        /// Opaque nixpkgs dependency; its internals remain authoritative in Nix.
        openssl: Package,
        /// Opaque nixpkgs dependency; its internals remain authoritative in Nix.
        zlib: Package,
        /// Opaque nixpkgs dependency; its internals remain authoritative in Nix.
        expat: Package,
        /// Opaque nixpkgs dependency; its internals remain authoritative in Nix.
        python3: Package,
        /// Opaque nixpkgs dependency; its internals remain authoritative in Nix.
        gettext: Package,
        /// Opaque nixpkgs dependency; its internals remain authoritative in Nix.
        cpio: Package,
        /// Opaque nixpkgs dependency; its internals remain authoritative in Nix.
        gnugrep: Package,
        /// Opaque nixpkgs dependency; its internals remain authoritative in Nix.
        gnused: Package,
        /// Opaque nixpkgs dependency; its internals remain authoritative in Nix.
        gawk: Package,
        /// Opaque nixpkgs dependency; its internals remain authoritative in Nix.
        coreutils: Package,
        /// Opaque nixpkgs dependency; its internals remain authoritative in Nix.
        openssh: Package,
        /// Opaque nixpkgs dependency; its internals remain authoritative in Nix.
        pcre2: Package,
        /// Opaque nixpkgs dependency; its internals remain authoritative in Nix.
        bash: Package,
        /// Opaque nixpkgs dependency; its internals remain authoritative in Nix.
        asciidoc: Package,
        /// Opaque nixpkgs dependency; its internals remain authoritative in Nix.
        texinfo: Package,
        /// Opaque nixpkgs dependency; its internals remain authoritative in Nix.
        xmlto: Package,
        /// Opaque nixpkgs dependency; its internals remain authoritative in Nix.
        docbook2x: Package,
        /// Opaque nixpkgs dependency; its internals remain authoritative in Nix.
        #[rusnix(rename = "docbook_xsl")]
        docbook_xsl: Package,
        /// Opaque nixpkgs dependency; its internals remain authoritative in Nix.
        #[rusnix(rename = "docbook_xml_dtd_45")]
        docbook_xml_dtd_45: Package,
        /// Opaque nixpkgs dependency; its internals remain authoritative in Nix.
        libxslt: Package,
        /// Opaque nixpkgs dependency; its internals remain authoritative in Nix.
        tcl: Package,
        /// Opaque nixpkgs dependency; its internals remain authoritative in Nix.
        tk: Package,
        /// Opaque nixpkgs dependency; its internals remain authoritative in Nix.
        make_wrapper: Package,
        /// Opaque nixpkgs dependency; its internals remain authoritative in Nix.
        libiconv: Package,
        /// Opaque nixpkgs dependency; its internals remain authoritative in Nix.
        libiconv_real: Package,
        /// Perl interpreter, library-path helper and gitweb dependencies.
        perl_packages: PerlPackages,
        /// Deferred feature choice, including Nix-resolved argument defaults.
        svn_support: bool,
        /// Deferred feature choice, including Nix-resolved argument defaults.
        perl_support: bool,
        /// Deferred feature choice, including Nix-resolved argument defaults.
        nls_support: bool,
        /// Deferred feature choice, including Nix-resolved argument defaults.
        osxkeychain_support: bool,
        /// Deferred feature choice, including Nix-resolved argument defaults.
        gui_support: bool,
        /// Deferred feature choice, including Nix-resolved argument defaults.
        with_manual: bool,
        /// Deferred feature choice, including Nix-resolved argument defaults.
        python_support: bool,
        /// Deferred feature choice, including Nix-resolved argument defaults.
        withpcre2: bool,
        /// Deferred feature choice, including Nix-resolved argument defaults.
        send_email_support: bool,
        /// Deferred feature choice, including Nix-resolved argument defaults.
        with_libsecret: bool,
        /// Deferred feature choice, including Nix-resolved argument defaults.
        with_ssh: bool,
        /// Deferred feature choice, including Nix-resolved argument defaults.
        do_install_check: bool,
        /// Caller-supplied dependency or test value; Nix owns its semantics.
        subversion_client: Package,
        /// Caller-supplied dependency or test value; Nix owns its semantics.
        perl_libs: NixList<Package>,
        /// Caller-supplied dependency or test value; Nix owns its semantics.
        smtp_perl_libs: NixList<Package>,
        /// Caller-supplied dependency or test value; Nix owns its semantics.
        #[rusnix(rename = "Security")]
        security: Package,
        /// Caller-supplied dependency or test value; Nix owns its semantics.
        #[rusnix(rename = "CoreServices")]
        core_services: Package,
        /// Caller-supplied dependency or test value; Nix owns its semantics.
        nixos_tests: NixosTests,
        /// Caller-supplied dependency or test value; Nix owns its semantics.
        #[rusnix(rename = "pkg-config")]
        pkg_config: Package,
        /// Caller-supplied dependency or test value; Nix owns its semantics.
        glib: Package,
        /// Caller-supplied dependency or test value; Nix owns its semantics.
        libsecret: Package,
        /// Caller-supplied dependency or test value; Nix owns its semantics.
        gzip: Package,
        /// Caller-supplied dependency or test value; Nix owns its semantics.
        sysctl: Package,
        /// Caller-supplied dependency or test value; Nix owns its semantics.
        #[rusnix(rename = "deterministic-host-uname")]
        deterministic_host_uname: Package,
        /// Caller-supplied dependency or test value; Nix owns its semantics.
        tests: Tests,
    }

    /// The caller's standard builder and its finite platform/tool dependencies.
    /// Whole-subtree access retains the entire supplied stdenv for build/host equality.
    #[rusnix(value)]
    struct Stdenv {
        /// Existing mkDerivation function; Rusnix does not reimplement the builder.
        mk_derivation: NixCallable<Package>,
        /// Platform where Git will run.
        host_platform: Platform,
        /// Compiler metadata needed for the glibc linker workaround.
        cc: Compiler,
        /// Shell executable selected by stdenv.
        shell: String,
        /// Shell package excluded from cross-built runtime references.
        shell_package: Package,
    }

    /// Finite platform properties, plus whole-record equality for the native-build test.
    #[rusnix(value)]
    struct Platform {
        /// Selects macOS-specific dependencies, scripts and test exclusions.
        is_darwin: bool,
        /// Selects FreeBSD threading flags and iconv dependency.
        #[rusnix(rename = "isFreeBSD")]
        is_free_bsd: bool,
        /// Selects the SunOS make flags.
        #[rusnix(rename = "isSunOS")]
        is_sun_os: bool,
        /// Selects musl compatibility flags and test exclusions.
        is_musl: bool,
        /// Selects the Apple Silicon test exclusion together with isDarwin.
        is_aarch64: bool,
        /// Names the host C library for the GNU/glibc linker workaround.
        libc: String,
    }

    /// The compiler property used by the pinned Git recipe.
    struct Compiler {
        /// Indicates whether the selected compiler is GNU GCC.
        #[rusnix(rename = "isGNU")]
        is_gnu: bool,
    }

    /// Only the build-platform interpreter is needed from this package set.
    struct BuildPackages {
        /// Interpreter used for documentation and installed checks in cross builds.
        perl: Package,
    }

    /// Perl package categories remain opaque; only the needed children are declared.
    struct PerlPackages {
        /// Interpreter and the library-directory metadata needed for wrapper scripts.
        perl: Perl,
        /// Existing helper that formats Perl package library paths.
        make_perl_path: NixCallable<Expr<String>>,
        /// CGI dependencies used when wrapping gitweb.
        #[rusnix(rename = "CGI")]
        cgi: Package,
        /// HTML parser used by gitweb.
        #[rusnix(rename = "HTMLParser")]
        html_parser: Package,
        /// FastCGI library used by gitweb.
        #[rusnix(rename = "CGIFast")]
        cgi_fast: Package,
        /// FastCGI bindings used by gitweb.
        #[rusnix(rename = "FCGI")]
        fcgi: Package,
        /// FastCGI process manager used by gitweb.
        #[rusnix(rename = "FCGIProcManager")]
        fcgi_proc_manager: Package,
        /// Tag-cloud library used by gitweb.
        #[rusnix(rename = "HTMLTagCloud")]
        html_tag_cloud: Package,
    }

    /// The Perl derivation remains opaque when used as a dependency or store path.
    #[rusnix(value)]
    struct Perl {
        /// Relative library directory used by installed Perl wrappers.
        lib_prefix: String,
    }

    /// Nix library dispatch stays dynamic; metadata dependencies are declared explicitly.
    #[rusnix(value)]
    struct Lib {
        /// License metadata included in the package.
        licenses: Licenses,
        /// Supported-platform metadata included in the package.
        platforms: Platforms,
        /// Maintainer records included in the package.
        maintainers: Maintainers,
    }

    /// The pinned Git license record.
    struct Licenses {
        /// GPL version 2 metadata from the caller's Nix library.
        gpl2: NixValue,
    }

    /// The supported-platform list from the caller's Nix library.
    struct Platforms {
        /// All platforms listed by nixpkgs, retained exactly as upstream.
        all: NixValue,
    }

    /// The pinned package's maintainers; these records remain opaque.
    struct Maintainers {
        /// Maintainer metadata from nixpkgs.
        primeos: NixValue,
        /// Maintainer metadata from nixpkgs.
        wmertens: NixValue,
        /// Maintainer metadata from nixpkgs.
        globin: NixValue,
        /// Maintainer metadata from nixpkgs.
        kashw2: NixValue,
    }

    /// NixOS integration test referenced by Git's passthru.
    struct NixosTests {
        /// Existing buildbot test; not executed by this example.
        buildbot: NixValue,
    }

    /// Package test record used by Git's passthru.
    struct Tests {
        /// Existing fetchgit tests merged with Git's own tests.
        fetchgit: NixValue,
    }
}

pub(super) use args::Inputs;

impl Inputs {
    /// Refer to an asset in the pinned Git source directory, such as a patch or updater.
    /// Returns a Nix path without reading the file or fetching anything.
    #[track_caller]
    pub(super) fn file(&self, name: &str) -> NixValue {
        Nixpkgs::new().source_path(&format!("pkgs/applications/version-management/git/{name}"))
    }
}

/// Supplies Git's explicit dependencies while callPackage selects the remaining scope.
pub(super) fn arguments(model: super::model::Git) -> Arguments {
    Arguments {
        model,
        openssl: None,
    }
}

/// Named authoring inputs retain the model and supplied dependency until lowering.
pub struct Arguments {
    model: super::model::Git,
    openssl: Option<Package>,
}

impl Arguments {
    pub fn with_openssl(mut self, openssl: Package) -> Self {
        self.openssl = Some(openssl);
        self
    }
}

impl IntoRusnixValue for Arguments {
    #[track_caller]
    fn into_value(self) -> RusnixValue {
        let mut fields = self.model.arguments();

        // These explicit arguments reproduce Git's callPackage wiring in all-packages.nix.
        // Other dependencies are selected by callPackage, including cross-build splicing.
        let pkgs = Nixpkgs::new();
        fields.extend([
            (
                "Security",
                pkgs.value("darwin.apple_sdk.frameworks.Security"),
            ),
            (
                "CoreServices",
                pkgs.value("darwin.apple_sdk.frameworks.CoreServices"),
            ),
            (
                "perlLibs",
                NixValue::list(
                    ["LWP", "URI", "TermReadKey"]
                        .map(|p| pkgs.get(&format!("perlPackages.{p}")).into()),
                ),
            ),
            (
                "smtpPerlLibs",
                NixValue::list(
                    [
                        "libnet",
                        "NetSMTPSSL",
                        "IOSocketSSL",
                        "NetSSLeay",
                        "AuthenSASL",
                        "DigestHMAC",
                    ]
                    .map(|p| pkgs.get(&format!("perlPackages.{p}")).into()),
                ),
            ),
        ]);

        if let Some(openssl) = self.openssl {
            fields.push(("openssl", openssl.into()));
        }
        RusnixValue::leaf(NixValue::record(fields))
    }
}
