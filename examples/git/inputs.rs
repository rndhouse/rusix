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

/// Symbolic access to the arguments supplied to Git's native Nix package function.
/// Helpers construct deferred lookups and calls; Rust never reads the argument values.
pub(super) struct Inputs {
    /// The argument record Nix resolves when calling the factory, including defaults.
    pub(super) args: NixValue,
}

impl Inputs {
    /// Select a supplied argument or dotted child path, such as `perlPackages.perl`.
    /// This follows the caller's dependencies and overrides, not a separate package set.
    #[track_caller]
    pub(super) fn get(&self, name: &str) -> NixValue {
        self.args.clone().select(name)
    }

    /// Call a function from the caller's `lib`, applying arguments left to right.
    /// Nix performs the curried application and validates the function's arguments.
    #[track_caller]
    pub(super) fn lib(&self, name: &str, args: impl IntoIterator<Item = NixValue>) -> NixValue {
        self.get("lib").select(name).apply(args)
    }

    /// Select a property of `stdenv.hostPlatform`, where the resulting Git will run.
    /// For cross builds this can differ from the platform running the build tools.
    #[track_caller]
    pub(super) fn host(&self, name: &str) -> NixValue {
        self.get("stdenv").select("hostPlatform").select(name)
    }

    /// Defer the upstream native-build test: build and host platform records are equal.
    /// The result is a Nix boolean expression, not a boolean Rust can inspect.
    #[track_caller]
    pub(super) fn native(&self) -> NixValue {
        self.get("stdenv.buildPlatform")
            .equals(self.get("stdenv.hostPlatform"))
    }

    /// Refer to an asset in the pinned Git source directory, such as a patch or updater.
    /// Returns a Nix path without reading the file or fetching anything.
    #[track_caller]
    pub(super) fn file(&self, name: &str) -> NixValue {
        Nixpkgs::new().source_path(&format!("pkgs/applications/version-management/git/{name}"))
    }

    /// Produce a one-element list when the Nix condition is true, otherwise `[]`.
    /// The excluded value stays unforced when the condition is false.
    #[track_caller]
    pub(super) fn optional(&self, condition: NixValue, value: NixValue) -> NixValue {
        self.lib("optional", [condition, value])
    }

    /// Keep an entire deferred list when the Nix condition is true, otherwise `[]`.
    /// Unlike `optional`, this does not wrap the supplied list in another list.
    #[track_caller]
    pub(super) fn optionals(&self, condition: NixValue, values: NixValue) -> NixValue {
        self.lib("optionals", [condition, values])
    }

    /// Keep symbolic text when the Nix condition is true, otherwise an empty string.
    /// Selected text retains its store dependencies; excluded text stays unforced.
    #[track_caller]
    pub(super) fn optional_text(&self, condition: NixValue, text: NixValue) -> NixValue {
        self.lib("optionalString", [condition, text])
    }

    /// Negate a deferred boolean; Nix checks its type and chooses the result later.
    #[track_caller]
    pub(super) fn not(&self, condition: NixValue) -> NixValue {
        NixValue::if_else(condition, false, true)
    }

    /// Defer a conjunction of conditions using `lib.all`, stopping at the first false.
    /// An empty collection is true, matching the Nix library's behavior.
    #[track_caller]
    pub(super) fn all(&self, conditions: impl IntoIterator<Item = NixValue>) -> NixValue {
        self.lib(
            "all",
            [NixValue::function(|x| x), NixValue::list(conditions)],
        )
    }

    /// Flatten one level of deferred lists in order, for composing dependency groups.
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
