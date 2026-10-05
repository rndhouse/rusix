//! Reproduces the pinned Git package recipe using real nixpkgs builders and helpers.
use super::inputs::{ARGUMENTS, Inputs};
use rusnix_ir::{IntoRusnixValue, interop::NixValue, nix_text};

/// A native Nix package function, suitable for ordinary callPackage and overrides.
pub fn factory() -> NixValue {
    NixValue::function_attrs(ARGUMENTS.iter().copied(), |args| {
        let inputs = Inputs { args };
        let defaults = vec![
            ("svnSupport", false.into()),
            ("perlSupport", inputs.native()),
            ("nlsSupport", true.into()),
            ("osxkeychainSupport", inputs.host("isDarwin")),
            ("guiSupport", false.into()),
            ("withManual", true.into()),
            ("pythonSupport", true.into()),
            ("withpcre2", true.into()),
            ("sendEmailSupport", inputs.get("perlSupport")),
            ("withLibsecret", false.into()),
            ("withSsh", false.into()),
            ("doInstallCheck", inputs.not(inputs.host("isDarwin"))),
        ];

        // Keep the public Nix interface's assertions even though the Rust model
        // already rules out the Perl-dependent invalid combinations.
        let derivation =
            inputs
                .get("stdenv.mkDerivation")
                .call(NixValue::function(|final_attrs| {
                    attributes(&inputs, final_attrs)
                }));
        let checks = [
            (
                inputs.lib(
                    "any",
                    [
                        NixValue::function(|x| x),
                        NixValue::list([
                            inputs.not(inputs.get("osxkeychainSupport")),
                            inputs.host("isDarwin"),
                        ]),
                    ],
                ),
                "osxkeychainSupport requires Darwin",
            ),
            (
                inputs.lib(
                    "any",
                    [
                        NixValue::function(|x| x),
                        NixValue::list([
                            inputs.not(inputs.get("sendEmailSupport")),
                            inputs.get("perlSupport"),
                        ]),
                    ],
                ),
                "sendEmailSupport requires perlSupport",
            ),
            (
                inputs.lib(
                    "any",
                    [
                        NixValue::function(|x| x),
                        NixValue::list([
                            inputs.not(inputs.get("svnSupport")),
                            inputs.get("perlSupport"),
                        ]),
                    ],
                ),
                "svnSupport requires perlSupport",
            ),
        ];
        let body = checks
            .into_iter()
            .rev()
            .fold(derivation, |body, (condition, message)| {
                inputs.lib("throwIfNot", [condition, message.into(), body])
            });

        (defaults, body)
    })
}

// Fixed derivation fields use Rust structure; each NixValue keeps its deferred semantics.
#[derive(IntoRusnixValue)]
struct Derivation {
    /// Feature-sensitive package name, including the upstream minimal/SVN suffixes.
    pname: NixValue,
    /// Release pinned by this compatibility implementation.
    version: &'static str,
    /// Fixed-output fetcher derivation, evaluated without fetching its contents.
    src: NixValue,
    /// Documentation is a separate output when enabled.
    outputs: NixValue,
    /// Leaves debug information in stdenv's separate debug output.
    separate_debug_info: bool,
    /// Preserves upstream's format hardening exception.
    hardening_disable: Vec<&'static str>,
    /// Enables the standard builder's parallel make behavior.
    enable_parallel_building: bool,
    /// Pinned local patches and the conditional Darwin GUI fix.
    patches: NixValue,
    /// Applies gettext and optional SSH path substitutions.
    post_patch: NixValue,
    /// Tools selected for the build platform by nixpkgs.
    native_build_inputs: NixValue,
    /// Host dependencies selected by the enabled features and platform.
    build_inputs: NixValue,
    /// Platform-specific linker options retain their standard external name.
    #[rusnix(rename = "NIX_LDFLAGS")]
    nix_ldflags: NixValue,
    /// Configure cache entries include cross-compilation answers.
    configure_flags: NixValue,
    /// Determines the installed Perl library directory.
    pre_build: NixValue,
    /// Git's upstream build switches and interpreter paths.
    make_flags: NixValue,
    /// Cross builds must not retain the build shell package.
    disallowed_references: NixValue,
    /// Builds auxiliary Git programs and optional credential helpers.
    post_build: NixValue,
    /// Avoids hardlinks during installation.
    install_flags: Vec<&'static str>,
    /// Prepares credential-helper symlinks.
    pre_install: NixValue,
    /// Installs, patches, and wraps programs exactly as the pinned recipe does.
    post_install: NixValue,
    /// Upstream runs tests against the installed output instead of the build tree.
    do_check: bool,
    /// Retains the caller's platform-sensitive installed-test choice.
    do_install_check: NixValue,
    /// Make target used for installed tests.
    install_check_target: &'static str,
    /// Uses the build-platform Perl interpreter for tests.
    install_check_flags: NixValue,
    /// Supplies sysctl on systems whose tests require it.
    native_install_check_inputs: NixValue,
    /// Excludes the same unsupported and flaky tests as upstream.
    pre_install_check: NixValue,
    /// Additional directories inspected by stdenv's debug stripping hook.
    strip_debug_list: Vec<&'static str>,
    /// Non-build attributes preserve tests, the shell path, and update script.
    passthru: NixValue,
    /// Standard nixpkgs package metadata remains authoritative in Nix.
    meta: NixValue,
}

fn attributes(i: &Inputs, final_attrs: NixValue) -> NixValue {
    let minimal = i.all(
        [
            "svnSupport",
            "guiSupport",
            "sendEmailSupport",
            "withManual",
            "pythonSupport",
            "withpcre2",
        ]
        .map(|name| i.not(i.get(name))),
    );
    let svn = i
        .get("subversionClient.override")
        .call(NixValue::record([("perlBindings", i.get("perlSupport"))]));
    let perl = i.get("perlPackages.perl");
    let patches = i.lists([
        NixValue::list(["docbook2texi.patch", "git-sh-i18n.patch", "git-send-email-honor-PATH.patch", "installCheck-path.patch"].map(|p| i.file(p))),
        i.optional(i.get("withSsh"), i.file("ssh-path.patch")),
        i.optional(i.all([i.get("guiSupport"), i.host("isDarwin")]), i.get("fetchpatch").call(NixValue::record([
            ("name", "gitk_check_main_window_visibility_before_waiting_for_it_to_show.patch".into()),
            ("url", "https://github.com/git/git/commit/1db62e44b7ec93b6654271ef34065b31496cd02e.patch".into()),
            ("hash", "sha256-ntvnrYFFsJ1Ebzc6vM9/AMFLHMS1THts73PIOG5DkQo=".into()),
        ]))),
    ]);

    let native_build_inputs = i.lists([
        NixValue::list(
            [
                "deterministic-host-uname",
                "gettext",
                "perlPackages.perl",
                "makeWrapper",
                "pkg-config",
            ]
            .map(|p| i.get(p)),
        ),
        i.optionals(
            i.get("withManual"),
            NixValue::list(
                [
                    "asciidoc",
                    "texinfo",
                    "xmlto",
                    "docbook2x",
                    "docbook_xsl",
                    "docbook_xml_dtd_45",
                    "libxslt",
                ]
                .map(|p| i.get(p)),
            ),
        ),
    ]);
    let build_inputs = i.lists([
        NixValue::list([
            i.get("curl"),
            i.get("openssl"),
            i.get("zlib"),
            i.get("expat"),
            i.get("cpio"),
            NixValue::if_else(
                i.host("isFreeBSD"),
                i.get("libiconvReal"),
                i.get("libiconv"),
            ),
            i.get("bash"),
        ]),
        i.optional(i.get("perlSupport"), perl.clone()),
        i.optionals(
            i.get("guiSupport"),
            NixValue::list([i.get("tcl"), i.get("tk")]),
        ),
        i.optional(i.get("withpcre2"), i.get("pcre2")),
        i.optionals(
            i.host("isDarwin"),
            NixValue::list([i.get("Security"), i.get("CoreServices")]),
        ),
        i.optionals(
            i.get("withLibsecret"),
            NixValue::list([i.get("glib"), i.get("libsecret")]),
        ),
    ]);

    let make_flags = i.lists([
        NixValue::list(["prefix=${out}".into()]),
        i.optional(
            i.native(),
            nix_text!("SHELL_PATH={shell}", shell = i.get("stdenv.shell")),
        ),
        NixValue::if_else(
            i.get("perlSupport"),
            NixValue::list([nix_text!("PERL_PATH={perl}/bin/perl", perl = perl.clone())]),
            NixValue::list(["NO_PERL=1".into()]),
        ),
        NixValue::if_else(
            i.get("pythonSupport"),
            NixValue::list([nix_text!(
                "PYTHON_PATH={python}/bin/python",
                python = i.get("python3")
            )]),
            NixValue::list(["NO_PYTHON=1".into()]),
        ),
        i.optionals(
            i.host("isSunOS"),
            NixValue::list(
                ["INSTALL=install", "NO_INET_NTOP=", "NO_INET_PTON="].map(NixValue::from),
            ),
        ),
        NixValue::if_else(
            i.host("isDarwin"),
            NixValue::list(["NO_APPLE_COMMON_CRYPTO=1".into()]),
            NixValue::list(["sysconfdir=/etc".into()]),
        ),
        i.optionals(
            i.host("isMusl"),
            NixValue::list(["NO_SYS_POLL_H=1", "NO_GETTEXT=YesPlease"].map(NixValue::from)),
        ),
        i.optional(i.get("withpcre2"), "USE_LIBPCRE2=1".into()),
        i.optional(i.not(i.get("nlsSupport")), "NO_GETTEXT=1".into()),
        i.optional(i.host("isDarwin"), "TKFRAMEWORK=/nonexistent".into()),
    ]);

    // This finite self-reference is evaluated by mkDerivation, not inspected in Rust.
    // overrideAttrs must see the eventual package, including later ordinary Nix overrides.
    let installed_test = final_attrs
        .select("finalPackage.overrideAttrs")
        .call(NixValue::function(|_| {
            NixValue::record([("doInstallCheck", true.into())])
        }));
    let passthru_tests = i.lib(
        "mergeAttrs",
        [
            NixValue::record([
                ("withInstallCheck", installed_test),
                ("buildbot-integration", i.get("nixosTests.buildbot")),
            ]),
            i.get("tests.fetchgit"),
        ],
    );

    Derivation {
        pname: NixValue::concat_text([
            "git".into(),
            i.optional_text(i.get("svnSupport"), "-with-svn".into()),
            i.optional_text(minimal, "-minimal".into()),
        ]),
        version: "2.47.0",
        src: i.get("fetchurl").call(NixValue::record([
            (
                "url",
                "https://www.kernel.org/pub/software/scm/git/git-2.47.0.tar.xz".into(),
            ),
            (
                "hash",
                "sha256-HOEU2ohwQnG0PgJ8UeBNk5n4yI6e91Qtrnrrrn2HvE4=".into(),
            ),
        ])),
        outputs: i.lists([
            NixValue::list(["out".into()]),
            i.optional(i.get("withManual"), "doc".into()),
        ]),
        separate_debug_info: true,
        hardening_disable: vec!["format"],
        enable_parallel_building: true,
        patches,
        post_patch: post_patch(i),
        native_build_inputs,
        build_inputs,
        nix_ldflags: NixValue::concat_text([
            i.optional_text(
                i.all([i.get("stdenv.cc.isGNU"), i.host("libc").equals("glibc")]),
                "-lgcc_s".into(),
            ),
            i.optional_text(i.host("isFreeBSD"), "-lthr".into()),
        ]),
        configure_flags: i.lists([
            NixValue::list([nix_text!(
                "ac_cv_prog_CURL_CONFIG={curl}/bin/curl-config",
                curl = i.lib("getDev", [i.get("curl")])
            )]),
            i.optionals(
                i.not(i.native()),
                NixValue::list(
                    [
                        "ac_cv_fread_reads_directories=yes",
                        "ac_cv_snprintf_returns_bogus=no",
                        "ac_cv_iconv_omits_bom=no",
                    ]
                    .map(NixValue::from),
                ),
            ),
        ]),
        pre_build: pre_build(),
        make_flags,
        disallowed_references: i.optional(i.not(i.native()), i.get("stdenv.shellPackage")),
        post_build: post_build(i),
        install_flags: vec!["NO_INSTALL_HARDLINKS=1"],
        pre_install: pre_install(i),
        post_install: post_install(i, svn),
        do_check: false,
        do_install_check: i.get("doInstallCheck"),
        install_check_target: "test",
        install_check_flags: NixValue::list([
            "DEFAULT_TEST_TARGET=prove".into(),
            nix_text!(
                "PERL_PATH={perl}/bin/perl",
                perl = i.get("buildPackages.perl")
            ),
        ]),
        native_install_check_inputs: i.optional(
            i.lib(
                "any",
                [
                    NixValue::function(|x| x),
                    NixValue::list([i.host("isDarwin"), i.host("isFreeBSD")]),
                ],
            ),
            i.get("sysctl"),
        ),
        pre_install_check: pre_install_check(i),
        strip_debug_list: vec![
            "lib",
            "libexec",
            "bin",
            "share/git/contrib/credential/libsecret",
        ],
        passthru: NixValue::record([
            ("shellPath", "/bin/git-shell".into()),
            ("tests", passthru_tests),
            ("updateScript", i.file("update.sh")),
        ]),
        meta: metadata(i),
    }
    .into_value()
    .into_nix_value()
    .unwrap()
}

fn metadata(i: &Inputs) -> NixValue {
    NixValue::record([
        ("homepage", "https://git-scm.com/".into()),
        ("description", "Distributed version control system".into()),
        ("license", i.get("lib.licenses.gpl2")),
        ("changelog", "https://github.com/git/git/blob/v2.47.0/Documentation/RelNotes/2.47.0.txt".into()),
        ("longDescription", "Git, a popular distributed version control system designed to\nhandle very large projects with speed and efficiency.\n".into()),
        ("platforms", i.get("lib.platforms.all")),
        ("maintainers", NixValue::list(["primeos", "wmertens", "globin", "kashw2"].map(|m| i.get(&format!("lib.maintainers.{m}"))))),
        ("mainProgram", "git".into()),
    ])
}

fn gettext_patch(i: &Inputs) -> NixValue {
    nix_text!(
        r#"
            # Fix references to gettext introduced by ./git-sh-i18n.patch
            substituteInPlace git-sh-i18n.sh \
                --subst-var-by gettext {gettext}

            # ensure we are using the correct shell when executing the test scripts
            patchShebangs t/*.sh
        "#,
        gettext = i.get("gettext")
    )
}

fn ssh_patch(i: &Inputs) -> NixValue {
    nix_text!(
        r#"
            for x in connect.c git-gui/lib/remote_add.tcl ; do
              substituteInPlace "$x" \
                --subst-var-by ssh "{openssh}/bin/ssh"
            done
        "#,
        openssh = i.get("openssh")
    )
}

fn pre_build() -> NixValue {
    nix_text!(
        r#"
            makeFlagsArray+=( perllibdir=$out/$(perl -MConfig -wle 'print substr $Config{{installsitelib}}, 1 + length $Config{{siteprefixexp}}') )
        "#
    )
}

fn subtree_build() -> NixValue {
    nix_text!(
        r#"
            make -C contrib/subtree
        "#
    )
}

fn diff_highlight_build() -> NixValue {
    nix_text!(
        r#"
            make -C contrib/diff-highlight
        "#
    )
}

fn keychain_build() -> NixValue {
    nix_text!(
        r#"
            make -C contrib/credential/osxkeychain
        "#
    )
}

fn secret_build() -> NixValue {
    nix_text!(
        r#"
            make -C contrib/credential/libsecret
        "#
    )
}

fn keychain_install() -> NixValue {
    nix_text!(
        r#"
            mkdir -p $out/bin
            ln -s $out/share/git/contrib/credential/osxkeychain/git-credential-osxkeychain $out/bin/
            rm -f $PWD/contrib/credential/osxkeychain/git-credential-osxkeychain.o
        "#
    )
}

fn secret_install() -> NixValue {
    nix_text!(
        r#"
            mkdir -p $out/bin
            ln -s $out/share/git/contrib/credential/libsecret/git-credential-libsecret $out/bin/
            rm -f $PWD/contrib/credential/libsecret/git-credential-libsecret.o
        "#
    )
}

fn base_install(i: &Inputs) -> NixValue {
    nix_text!(
        r#"
            notSupported() {{
              unlink $1 || true
            }}

            # Install git-subtree.
            make -C contrib/subtree install {install_doc}
            rm -rf contrib/subtree

            # Install contrib stuff.
            mkdir -p $out/share/git
            cp -a contrib $out/share/git/
            mkdir -p $out/share/bash-completion/completions
            ln -s $out/share/git/contrib/completion/git-completion.bash $out/share/bash-completion/completions/git
            ln -s $out/share/git/contrib/completion/git-prompt.sh $out/share/bash-completion/completions/
            # only readme, developed in another repo
            rm -r contrib/hooks/multimail
            mkdir -p $out/share/git-core/contrib
            cp -a contrib/hooks/ $out/share/git-core/contrib/
            substituteInPlace $out/share/git-core/contrib/hooks/pre-auto-gc-battery \
              --replace ' grep' ' {grep}/bin/grep' \

            # grep is a runtime dependency, need to patch so that it's found
            substituteInPlace $out/libexec/git-core/git-sh-setup \
                --replace ' grep' ' {grep}/bin/grep' \
                --replace ' egrep' ' {grep}/bin/egrep'

            # Fix references to the perl, sed, awk and various coreutil binaries used by
            # shell scripts that git calls (e.g. filter-branch)
            SCRIPT="$(cat <<'EOS'
              BEGIN{{
                @a=(
                  '{grep}/bin/grep', '{sed}/bin/sed', '{awk}/bin/awk',
                  '{coreutils}/bin/cut', '{coreutils}/bin/basename', '{coreutils}/bin/dirname',
                  '{coreutils}/bin/wc', '{coreutils}/bin/tr'
                  {perl_program}
                );
              }}
              foreach $c (@a) {{
                $n=(split("/", $c))[-1];
                s|(?<=[^#][^/.-])\b${{n}}(?=\s)|${{c}}|g
              }}
            EOS
            )"
            perl -0777 -i -pe "$SCRIPT" \
              $out/libexec/git-core/git-{{sh-setup,filter-branch,merge-octopus,mergetool,quiltimport,request-pull,submodule,subtree,web--browse}}


            # Also put git-http-backend into $PATH, so that we can use smart
            # HTTP(s) transports for pushing
            ln -s $out/libexec/git-core/git-http-backend $out/bin/git-http-backend
            ln -s $out/share/git/contrib/git-jump/git-jump $out/bin/git-jump
        "#,
        install_doc = i.optional_text(i.get("withManual"), "install-doc".into()),
        grep = i.get("gnugrep"),
        sed = i.get("gnused"),
        awk = i.get("gawk"),
        coreutils = i.get("coreutils"),
        perl_program = i.optional_text(
            i.get("perlSupport"),
            nix_text!(", '{perl}/bin/perl'", perl = i.get("perlPackages.perl"))
        )
    )
}

fn perl_install(i: &Inputs) -> NixValue {
    nix_text!(
        r#"
            # wrap perl commands
            makeWrapper "$out/share/git/contrib/credential/netrc/git-credential-netrc.perl" $out/bin/git-credential-netrc \
                        --set PERL5LIB   "$out/{perl_prefix}:{perl_path}"
            wrapProgram $out/libexec/git-core/git-cvsimport \
                        --set GITPERLLIB "$out/{perl_prefix}:{perl_path}"
            wrapProgram $out/libexec/git-core/git-archimport \
                        --set GITPERLLIB "$out/{perl_prefix}:{perl_path}"
            wrapProgram $out/libexec/git-core/git-instaweb \
                        --set GITPERLLIB "$out/{perl_prefix}:{perl_path}"
            wrapProgram $out/libexec/git-core/git-cvsexportcommit \
                        --set GITPERLLIB "$out/{perl_prefix}:{perl_path}"

            # gzip (and optionally bzip2, xz, zip) are runtime dependencies for
            # gitweb.cgi, need to patch so that it's found
            sed -i -e "s|'compressor' => \['gzip'|'compressor' => ['{gzip}/bin/gzip'|" \
                $out/share/gitweb/gitweb.cgi
            # Give access to CGI.pm and friends (was removed from perl core in 5.22)
            for p in {gitweb_libs}; do
                sed -i -e "/use CGI /i use lib \"$p/{perl_prefix}\";" \
                    "$out/share/gitweb/gitweb.cgi"
            done
        "#,
        perl_prefix = i.get("perlPackages.perl.libPrefix"),
        perl_path = i.get("perlPackages.makePerlPath").call(i.get("perlLibs")),
        gzip = i.get("gzip"),
        gitweb_libs = i.lib(
            "concatStringsSep",
            [
                " ".into(),
                NixValue::list(
                    [
                        "CGI",
                        "HTMLParser",
                        "CGIFast",
                        "FCGI",
                        "FCGIProcManager",
                        "HTMLTagCloud"
                    ]
                    .map(|p| i.get(&format!("perlPackages.{p}")))
                )
            ]
        )
    )
}

fn svn_install(i: &Inputs, svn: NixValue) -> NixValue {
    nix_text!(
        r#"
            # wrap git-svn
            wrapProgram $out/libexec/git-core/git-svn \
              --set GITPERLLIB "$out/{perl_prefix}:{perl_path}" \
              --prefix PATH : "{svn}/bin"
        "#,
        perl_prefix = i.get("perlPackages.perl.libPrefix"),
        perl_path = i.get("perlPackages.makePerlPath").call(i.lists([
            i.get("perlLibs"),
            NixValue::list([svn.clone().select("out")])
        ])),
        svn = svn.select("out")
    )
}

fn no_svn_install() -> NixValue {
    nix_text!(
        r#"
            # replace git-svn by notification script
            notSupported $out/libexec/git-core/git-svn
        "#
    )
}

fn email_install(i: &Inputs) -> NixValue {
    nix_text!(
        r#"
            # wrap git-send-email
            wrapProgram $out/libexec/git-core/git-send-email \
                         --set GITPERLLIB "$out/{perl_prefix}:{smtp_path}"
        "#,
        perl_prefix = i.get("perlPackages.perl.libPrefix"),
        smtp_path = i
            .get("perlPackages.makePerlPath")
            .call(i.get("smtpPerlLibs"))
    )
}

fn no_email_install() -> NixValue {
    nix_text!(
        r#"
            # replace git-send-email by notification script
            notSupported $out/libexec/git-core/git-send-email
        "#
    )
}

fn manual_install(i: &Inputs) -> NixValue {
    nix_text!(
        r#"
            # Install man pages
            make -j $NIX_BUILD_CORES PERL_PATH="{perl}/bin/perl" cmd-list.made install install-html \
              -C Documentation
        "#,
        perl = i.get("buildPackages.perl")
    )
}

fn gui_install(i: &Inputs) -> NixValue {
    nix_text!(
        r#"
            # Wrap Tcl/Tk programs
            for prog in bin/gitk libexec/git-core/{{git-gui,git-citool,git-gui--askpass}}; do
              sed -i -e "s|exec 'wish'|exec '{tk}/bin/wish'|g" \
                     -e "s|exec wish|exec '{tk}/bin/wish'|g" \
                     "$out/$prog"
            done
            ln -s $out/share/git/contrib/completion/git-completion.bash $out/share/bash-completion/completions/gitk
        "#,
        tk = i.get("tk")
    )
}

fn no_gui_install() -> NixValue {
    nix_text!(
        r#"
            # Don't wrap Tcl/Tk, replace them by notification scripts
            for prog in bin/gitk libexec/git-core/git-gui; do
              notSupported "$out/$prog"
            done
        "#
    )
}

fn keychain_config() -> NixValue {
    nix_text!(
        r#"
            # enable git-credential-osxkeychain on darwin if desired (default)
            mkdir -p $out/etc
            cat > $out/etc/gitconfig << EOF
            [credential]
              helper = osxkeychain
            EOF
        "#
    )
}

fn base_check(i: &Inputs) -> NixValue {
    nix_text!(
        r#"
            installCheckFlagsArray+=(
              GIT_PROVE_OPTS="--jobs $NIX_BUILD_CORES --failures --state=failed,save"
              GIT_TEST_INSTALLED=$out/bin
              {no_svn}
            )

            function disable_test {{
              local test=$1 pattern=$2
              if [ $# -eq 1 ]; then
                mv t/{{,skip-}}$test.sh || true
              else
                sed -i t/$test.sh \
                  -e "/^\s*test_expect_.*$pattern/,/^\s*' *\$/{{s/^/: #/}}"
              fi
            }}

            # Shared permissions are forbidden in sandbox builds:
            substituteInPlace t/test-lib.sh \
              --replace "test_set_prereq POSIXPERM" ""
            # TODO: Investigate while these still fail (without POSIXPERM):
            # Tested to fail: 2.46.0
            disable_test t0001-init 'shared overrides system'
            # Tested to fail: 2.46.0
            disable_test t0001-init 'init honors global core.sharedRepository'
            # Tested to fail: 2.46.0
            disable_test t1301-shared-repo
            # /build/git-2.44.0/contrib/completion/git-completion.bash: line 452: compgen: command not found
            disable_test t9902-completion

            # Our patched gettext never fallbacks
            disable_test t0201-gettext-fallbacks
        "#,
        no_svn = i.optional_text(i.not(i.get("svnSupport")), "NO_SVN_TESTS=y".into())
    )
}

fn no_email_check() -> NixValue {
    nix_text!(
        r#"
            # Disable sendmail tests
            disable_test t9001-send-email
        "#
    )
}

fn common_check() -> NixValue {
    nix_text!(
        r#"
            # Flaky tests:
            disable_test t6421-merge-partial-clone

            # Fails reproducibly on ZFS on Linux with formD normalization
            disable_test t0021-conversion
            disable_test t3910-mac-os-precompose
        "#
    )
}

fn darwin_check() -> NixValue {
    nix_text!(
        r#"
            # XXX: Some tests added in 2.24.0 fail.
            # Please try to re-enable on the next release.
            disable_test t7816-grep-binary-pattern
            # fail (as of 2.33.0)
            #===(   18623;1208  8/?  224/?  2/? )= =fatal: Not a valid object name refs/tags/signed-empty
            disable_test t6300-for-each-ref
            # not ok 1 - populate workdir (with 2.33.1 on x86_64-darwin)
            disable_test t5003-archive-zip
        "#
    )
}

fn darwin_arm_check() -> NixValue {
    nix_text!(
        r#"
            disable_test t7527-builtin-fsmonitor
        "#
    )
}

fn musl_check() -> NixValue {
    nix_text!(
        r#"
            # Test fails (as of 2.17.0, musl 1.1.19)
            disable_test t3900-i18n-commit
            # Fails largely due to assumptions about BOM
            # Tested to fail: 2.18.0
            disable_test t0028-working-tree-encoding
        "#
    )
}

fn post_patch(i: &Inputs) -> NixValue {
    NixValue::concat_text([
        gettext_patch(i),
        i.optional_text(i.get("withSsh"), ssh_patch(i)),
    ])
}

fn post_build(i: &Inputs) -> NixValue {
    NixValue::concat_text([
        subtree_build(),
        i.optional_text(i.get("perlSupport"), diff_highlight_build()),
        i.optional_text(i.get("osxkeychainSupport"), keychain_build()),
        i.optional_text(i.get("withLibsecret"), secret_build()),
    ])
}

fn pre_install(i: &Inputs) -> NixValue {
    NixValue::concat_text([
        i.optional_text(i.get("osxkeychainSupport"), keychain_install()),
        i.optional_text(i.get("withLibsecret"), secret_install()),
    ])
}

fn post_install(i: &Inputs, svn: NixValue) -> NixValue {
    NixValue::concat_text([
        base_install(i),
        i.optional_text(i.get("perlSupport"), perl_install(i)),
        NixValue::if_else(i.get("svnSupport"), svn_install(i, svn), no_svn_install()),
        NixValue::if_else(
            i.get("sendEmailSupport"),
            email_install(i),
            no_email_install(),
        ),
        i.optional_text(i.get("withManual"), manual_install(i)),
        NixValue::if_else(i.get("guiSupport"), gui_install(i), no_gui_install()),
        i.optional_text(i.get("osxkeychainSupport"), keychain_config()),
    ])
}

fn pre_install_check(i: &Inputs) -> NixValue {
    NixValue::concat_text([
        base_check(i),
        i.optional_text(i.not(i.get("sendEmailSupport")), no_email_check()),
        common_check(),
        i.optional_text(i.host("isDarwin"), darwin_check()),
        i.optional_text(
            i.all([i.host("isDarwin"), i.host("isAarch64")]),
            darwin_arm_check(),
        ),
        i.optional_text(i.host("isMusl"), musl_check()),
    ])
}
