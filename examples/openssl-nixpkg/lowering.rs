//! Complete OpenSSL family policy; nixpkgs retains fetchers, stdenv and overrides.
use super::{
    inputs::{Inputs, args},
    model::Release,
    scripts,
};
use rusnix_ir::{
    IntoRusnixValue,
    interop::{NixLibrary, NixValue, Nixpkgs, PackageFunction},
    nix_record, nix_text, package,
};

/// The default nixpkgs OpenSSL is the 3.3 preview release at this pin.
pub fn factory(release: Release) -> PackageFunction {
    PackageFunction::from_function_attrs(args::argument_names().iter().copied(), |arguments| {
        let i = args::from_value(arguments);
        let defaults = defaults(&i);
        (defaults, common(&i, release))
    })
}

/// Preserve upstream's multi-result family interface without duplicating recipe policy.
pub fn family_factory() -> PackageFunction {
    PackageFunction::from_function_attrs(args::argument_names().iter().copied(), |arguments| {
        let i = args::from_value(arguments);
        (
            defaults(&i),
            NixValue::record(
                Release::ALL.map(|release| (release.attribute(), common(&i, release))),
            ),
        )
    })
}

fn defaults(i: &Inputs) -> Vec<(&'static str, NixValue)> {
    vec![
        ("withCryptodev", false.into()),
        ("withZlib", false.into()),
        ("enableSSL2", false.into()),
        ("enableSSL3", false.into()),
        ("enableMD2", false.into()),
        ("enableKTLS", host(i, "isLinux")),
        ("static", host(i, "isStatic")),
        ("conf", NixValue::null()),
    ]
}

#[track_caller]
fn host(i: &Inputs, field: &str) -> NixValue {
    i.stdenv().select("hostPlatform").select(field)
}

fn common(i: &Inputs, release: Release) -> NixValue {
    let lib = NixLibrary::from_value(i.lib());
    let version: NixValue = release.version().into();
    let path = |file: &str| {
        Nixpkgs::new().source_path(&format!("pkgs/development/libraries/openssl/{file}"))
    };
    let mut patches = vec![path(match release {
        Release::Legacy => "1.1/nix-ssl-cert-file.patch",
        _ => "3.0/nix-ssl-cert-file.patch",
    })];
    if !matches!(release, Release::Legacy) {
        patches.push(path("3.0/openssl-disable-kernel-detection.patch"));
    }
    patches.push(NixValue::if_else(
        host(i, "isDarwin"),
        path(if matches!(release, Release::Preview) {
            "3.3/use-etc-ssl-certs-darwin.patch"
        } else {
            "use-etc-ssl-certs-darwin.patch"
        }),
        path(if matches!(release, Release::Preview) {
            "3.3/use-etc-ssl-certs.patch"
        } else {
            "use-etc-ssl-certs.patch"
        }),
    ));
    let extra_meta = match release {
        Release::Legacy => nix_record! {
            "knownVulnerabilities": NixValue::list([
                "OpenSSL 1.1 is reaching its end of life on 2023/09/11 and cannot be supported through the NixOS 23.11 release cycle. https://www.openssl.org/blog/blog/2023/03/28/1.1.1-EOL/".into(),
            ]),
        },
        _ => nix_record! { "license": i.lib().select("licenses.asl20") },
    };
    // Upstream's version/hash are lexically captured by common, not finalAttrs.
    i.stdenv()
        .select("mkDerivation")
        .call(NixValue::function(|final_attrs| {
            attributes(
                i,
                &lib,
                version,
                release.hash().into(),
                NixValue::list(patches),
                true.into(),
                extra_meta,
                final_attrs,
            )
        }))
}

/// The common recipe parameters mirror the upstream shared function, not a core schema.
#[allow(clippy::too_many_arguments)]
fn attributes(
    i: &Inputs,
    lib: &NixLibrary,
    version: NixValue,
    hash: NixValue,
    patches: NixValue,
    with_docs: NixValue,
    extra_meta: NixValue,
    final_attrs: NixValue,
) -> NixValue {
    let modern = lib.version_at_least(version.clone(), "1.1.1");
    let v3 = lib.version_at_least(version.clone(), "3.0.0");
    let old = lib.version_older(version.clone(), "3.0");
    let fixed = version.clone().replace_text([(".", "_")]);
    let source = i.fetchurl().call(nix_record! {
        "url": NixValue::if_else(
            old,
            nix_text!(
                "https://github.com/openssl/openssl/releases/download/OpenSSL_{fixed}/openssl-{version}.tar.gz",
                fixed = fixed,
                version = version.clone(),
            ),
            nix_text!(
                "https://github.com/openssl/openssl/releases/download/openssl-{version}/openssl-{version}.tar.gz",
                version = version.clone(),
            ),
        ),
        "hash": hash,
    });
    let flags = NixValue::concat_lists([
        NixValue::list([
            "shared".into(),
            "--libdir=lib".into(),
            NixValue::if_else(
                i.static_build(),
                "--openssldir=/.$(etc)/etc/ssl",
                "--openssldir=etc/ssl",
            ),
        ]),
        lib.optionals(
            i.with_cryptodev(),
            NixValue::list(["-DHAVE_CRYPTODEV".into(), "-DUSE_CRYPTODEV_DIGESTS".into()]),
        ),
        lib.optional(i.enable_md2(), "enable-md2"),
        lib.optional(i.enable_ssl2(), "enable-ssl2"),
        lib.optional(i.enable_ssl3(), "enable-ssl3"),
        lib.optional(v3.clone().and(i.enable_ktls()), "enable-ktls"),
        lib.optional(modern.clone().and(host(i, "isAarch64")), "no-afalgeng"),
        lib.optional(modern.clone().and(i.static_build()), "no-shared"),
        lib.optional(v3.and(i.static_build()), "no-module"),
        lib.optional(i.static_build(), "no-ct"),
        lib.optional(i.with_zlib(), "zlib"),
        lib.optional(host(i, "isOpenBSD"), "no-devcryptoeng"),
        lib.optionals(
            host(i, "isMips").and(
                i.stdenv()
                    .select("hostPlatform")
                    .has_attr("gcc")
                    .and(host(i, "gcc").has_attr("arch")),
            ),
            NixValue::list([nix_text!(
                "CFLAGS=-march={arch}",
                arch = host(i, "gcc.arch")
            )]),
        ),
    ]);
    Recipe {
        pname: "openssl",
        version: version.clone(),
        src: source,
        patches,
        post_patch: NixValue::concat_text([
            scripts::patch_configure(),
            lib.optional_text(!modern.clone(), scripts::patch_old_tests()),
            lib.optional_text(
                modern.clone(),
                scripts::patch_env(i.build_packages().select("coreutils")),
            ),
            lib.optional_text(modern.and(host(i, "isMusl")), scripts::patch_musl()),
            lib.optional_text(i.static_build(), scripts::patch_static_engines()),
        ]),
        outputs: NixValue::concat_lists([
            NixValue::list(["bin".into(), "dev".into(), "out".into(), "man".into()]),
            lib.optional(with_docs, "doc"),
            lib.optional(i.static_build(), "etc"),
        ]),
        set_output_flags: false,
        separate_debug_info: (!host(i, "isDarwin")).and(
            (!i.stdenv().select("hostPlatform").attr_or("useLLVM", false))
                .and(i.stdenv().select("cc.isGNU")),
        ),
        native_build_inputs: NixValue::concat_lists([
            lib.optional(!host(i, "isWindows"), i.make_binary_wrapper()),
            NixValue::list([i.perl()]),
            lib.optionals(i.static_build(), NixValue::list([i.remove_references_to()])),
        ]),
        build_inputs: NixValue::concat_lists([
            lib.optional(i.with_cryptodev(), i.cryptodev()),
            lib.optional(i.with_zlib(), i.zlib()),
        ]),
        configure_platforms: Vec::<NixValue>::new(),
        configure_script: configure_script(i, lib, &version),
        dont_add_static_configure_flags: true,
        configure_flags: flags,
        make_flags: vec!["MANDIR=$(man)/share/man", "MANSUFFIX=ssl"],
        enable_parallel_building: true,
        post_install: NixValue::concat_text([
            NixValue::if_else(
                i.static_build(),
                scripts::install_static(),
                scripts::install_shared(),
            ),
            scripts::install_bin(),
            lib.optional_text(!host(i, "isWindows"), scripts::install_rehash()),
            scripts::install_dev(),
            lib.optional_text(
                !i.conf().equals(NixValue::null()),
                scripts::install_conf(i.conf()),
            ),
        ]),
        post_fixup: NixValue::concat_text([
            lib.optional_text(
                !host(i, "isWindows"),
                scripts::fixup_perl(i.build_packages().select("perl")),
            ),
            lib.optional_text(
                lib.version_at_least(version.clone(), "3.3.0"),
                scripts::fixup_cmake(),
            ),
        ]),
        passthru: nix_record! {
            "tests": nix_record! {
                "pkg-config": i.testers()
                    .select("testMetaPkgConfig")
                    .call(final_attrs.select("finalPackage")),
            },
        },
        meta: nix_record! {
            "homepage": "https://www.openssl.org/",
            "changelog": nix_text!(
                "https://github.com/openssl/openssl/blob/openssl-{version}/CHANGES.md",
                version = version,
            ),
            "description": "Cryptographic library that implements the SSL and TLS protocols",
            "license": i.lib().select("licenses.openssl"),
            "mainProgram": "openssl",
            "maintainers": NixValue::concat_lists([
                NixValue::list([i.lib().select("maintainers.thillux")]),
                i.lib().select("teams.stridtech.members"),
            ]),
            "pkgConfigModules": NixValue::list([
                "libcrypto".into(), "libssl".into(), "openssl".into(),
            ]),
            "platforms": i.lib().select("platforms.all"),
        }
        .merge_attrs(extra_meta),
    }
    .try_into_nix_value()
    .expect("fixed OpenSSL recipe")
}

fn configure_script(i: &Inputs, lib: &NixLibrary, version: &NixValue) -> NixValue {
    let bits = host(i, "parsed.cpu.bits");
    let bsd = NixValue::if_else(
        host(i, "isx86_64"),
        "./Configure BSD-x86_64",
        NixValue::if_else(
            host(i, "isx86_32"),
            nix_text!(
                "./Configure BSD-x86{elf}",
                elf = lib.optional_text(host(i, "isElf"), "-elf")
            ),
            nix_text!("./Configure BSD-generic{bits}", bits = bits.clone()),
        ),
    );
    let linux = NixValue::if_else(
        host(i, "isx86_64"),
        "./Configure linux-x86_64",
        NixValue::if_else(
            host(i, "isMicroBlaze"),
            "./Configure linux-latomic",
            NixValue::if_else(
                host(i, "isMips32"),
                "./Configure linux-mips32",
                NixValue::if_else(
                    host(i, "isMips64n32"),
                    "./Configure linux-mips64",
                    NixValue::if_else(
                        host(i, "isMips64n64"),
                        "./Configure linux64-mips64",
                        nix_text!("./Configure linux-generic{bits}", bits = bits.clone()),
                    ),
                ),
            ),
        ),
    );
    let fallback = NixValue::if_else(
        package::build_host_equal(i.stdenv()),
        "./config",
        NixValue::if_else(
            host(i, "isBSD"),
            bsd,
            NixValue::if_else(
                host(i, "isMinGW"),
                nix_text!(
                    "./Configure mingw{bits}",
                    bits = lib.optional_text(!bits.clone().equals(32_i64), bits.clone().to_text())
                ),
                NixValue::if_else(
                    host(i, "isLinux"),
                    linux,
                    NixValue::if_else(
                        host(i, "isiOS"),
                        nix_text!("./Configure ios{bits}-cross", bits = bits),
                        NixValue::builtin("throw").call(nix_text!(
                            "Not sure what configuration to use for {config}",
                            config = host(i, "config")
                        )),
                    ),
                ),
            ),
        ),
    );
    let mut targets: Vec<(&str, NixValue)> = [
        ("armv5tel-linux", "./Configure linux-armv4 -march=armv5te"),
        ("armv6l-linux", "./Configure linux-armv4 -march=armv6"),
        ("armv7l-linux", "./Configure linux-armv4 -march=armv7-a"),
        ("x86_64-darwin", "./Configure darwin64-x86_64-cc"),
        ("aarch64-darwin", "./Configure darwin64-arm64-cc"),
        ("x86_64-linux", "./Configure linux-x86_64"),
        ("x86_64-solaris", "./Configure solaris64-x86_64-gcc"),
        ("powerpc64-linux", "./Configure linux-ppc64"),
        ("riscv64-linux", "./Configure linux64-riscv64"),
    ]
    .into_iter()
    .map(|(k, v)| (k, v.into()))
    .collect();
    targets.push((
        "riscv32-linux",
        NixValue::if_else(
            lib.version_at_least(version.clone(), "3.2"),
            "./Configure linux32-riscv32",
            "./Configure linux-latomic",
        ),
    ));
    // Dynamic attribute lookup/fallback is appropriate NixValue interop; no platform schema.
    NixValue::record(targets).attr_or(host(i, "system"), fallback)
}

#[derive(IntoRusnixValue)]
struct Recipe {
    pname: &'static str,
    version: NixValue,
    src: NixValue,
    patches: NixValue,
    post_patch: NixValue,
    outputs: NixValue,
    set_output_flags: bool,
    separate_debug_info: NixValue,
    native_build_inputs: NixValue,
    build_inputs: NixValue,
    configure_platforms: Vec<NixValue>,
    configure_script: NixValue,
    dont_add_static_configure_flags: bool,
    configure_flags: NixValue,
    make_flags: Vec<&'static str>,
    enable_parallel_building: bool,
    post_install: NixValue,
    post_fixup: NixValue,
    passthru: NixValue,
    meta: NixValue,
}
