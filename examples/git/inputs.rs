//! External dependencies stay opaque; the caller supplies the normal nixpkgs scope.
use rusnix_ir::interop::{NixValue, Nixpkgs};

/// The pinned package's public function arguments, including required dependencies.
pub(super) const ARGUMENTS: &[&str] = &[
    "fetchurl",
    "fetchpatch",
    "lib",
    "stdenv",
    "buildPackages",
    "curl",
    "openssl",
    "zlib",
    "expat",
    "perlPackages",
    "python3",
    "gettext",
    "cpio",
    "gnugrep",
    "gnused",
    "gawk",
    "coreutils",
    "openssh",
    "pcre2",
    "bash",
    "asciidoc",
    "texinfo",
    "xmlto",
    "docbook2x",
    "docbook_xsl",
    "docbook_xml_dtd_45",
    "libxslt",
    "tcl",
    "tk",
    "makeWrapper",
    "libiconv",
    "libiconvReal",
    "svnSupport",
    "subversionClient",
    "perlLibs",
    "smtpPerlLibs",
    "perlSupport",
    "nlsSupport",
    "osxkeychainSupport",
    "guiSupport",
    "withManual",
    "pythonSupport",
    "withpcre2",
    "sendEmailSupport",
    "Security",
    "CoreServices",
    "nixosTests",
    "withLibsecret",
    "pkg-config",
    "glib",
    "libsecret",
    "gzip",
    "withSsh",
    "sysctl",
    "deterministic-host-uname",
    "doInstallCheck",
    "tests",
];

/// A finite package interface, not Rust bindings for the dependency internals.
pub(super) struct Inputs {
    /// Resolved native Nix arguments, including dependent defaults.
    pub(super) args: NixValue,
}

impl Inputs {
    #[track_caller]
    pub(super) fn get(&self, name: &str) -> NixValue {
        self.args.clone().select(name)
    }

    #[track_caller]
    pub(super) fn lib(&self, name: &str, args: impl IntoIterator<Item = NixValue>) -> NixValue {
        self.get("lib").select(name).apply(args)
    }

    #[track_caller]
    pub(super) fn host(&self, name: &str) -> NixValue {
        self.get("stdenv").select("hostPlatform").select(name)
    }

    #[track_caller]
    pub(super) fn native(&self) -> NixValue {
        self.get("stdenv.buildPlatform")
            .equals(self.get("stdenv.hostPlatform"))
    }

    /// Local patches and the update script are paths in the checked pinned archive.
    #[track_caller]
    pub(super) fn file(&self, name: &str) -> NixValue {
        Nixpkgs::new().source_path(&format!("pkgs/applications/version-management/git/{name}"))
    }

    #[track_caller]
    pub(super) fn optional(&self, condition: NixValue, value: NixValue) -> NixValue {
        self.lib("optional", [condition, value])
    }

    #[track_caller]
    pub(super) fn optionals(&self, condition: NixValue, values: NixValue) -> NixValue {
        self.lib("optionals", [condition, values])
    }

    #[track_caller]
    pub(super) fn optional_text(&self, condition: NixValue, text: NixValue) -> NixValue {
        self.lib("optionalString", [condition, text])
    }

    #[track_caller]
    pub(super) fn not(&self, condition: NixValue) -> NixValue {
        NixValue::if_else(condition, false, true)
    }

    #[track_caller]
    pub(super) fn all(&self, conditions: impl IntoIterator<Item = NixValue>) -> NixValue {
        self.lib(
            "all",
            [NixValue::function(|x| x), NixValue::list(conditions)],
        )
    }

    #[track_caller]
    pub(super) fn lists(&self, lists: impl IntoIterator<Item = NixValue>) -> NixValue {
        self.lib("concatLists", [NixValue::list(lists)])
    }
}

/// Supplies Git's explicit dependencies while callPackage selects the remaining scope.
/// The factory and result remain deferred Nix values; no build is run.
pub(super) fn instantiate(factory: NixValue, model: super::model::Git) -> NixValue {
    let mut fields = model.arguments();

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

    pkgs.package_function("callPackage")
        .apply([factory, NixValue::record(fields)])
}
