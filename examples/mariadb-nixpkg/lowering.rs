//! Complete pinned client/server family; CMake and builders remain ordinary nixpkgs.
use super::{
    inputs::{ARGUMENTS, Inputs, args},
    model::Release,
    scripts,
};
use rusnix_ir::{
    IntoRusnixValue,
    interop::{NixLibrary, NixValue, Nixpkgs, PackageFunction},
    nix_text, package,
};

pub fn factory() -> PackageFunction {
    PackageFunction::from_function_attrs(ARGUMENTS.iter().copied(), |arguments| {
        let i = args::from_value(arguments);
        let defaults = vec![
            ("withStorageMroonga", true.into()),
            ("withStorageRocks", true.into()),
            ("withEmbedded", false.into()),
            ("withNuma", false.into()),
        ];
        let lib = NixLibrary::from_value(i.lib());
        let body = NixValue::function(|common| {
            let client = client(&i, &lib, &common);
            let server = server(&i, &lib, &common);
            NixValue::function(|server| {
                server
                    .clone()
                    .merge_attrs(NixValue::record([("client", client), ("server", server)]))
            })
            .call(server)
        })
        .call(common(&i, &lib));
        (defaults, body)
    })
}

/// The family uses ordinary Rust iteration, not a package-family framework.
pub fn family() -> NixValue {
    let factory = factory();
    NixValue::function(|factory| {
        NixValue::record(Release::ALL.map(|release| {
            // Each call retains real callPackage's override and dependency-splicing semantics.
            let selected =
                PackageFunction::from_function_attrs(ARGUMENTS.iter().copied(), |arguments| {
                    let defaults = vec![
                        ("withStorageMroonga", true.into()),
                        ("withStorageRocks", true.into()),
                        ("withEmbedded", false.into()),
                        ("withNuma", false.into()),
                    ];
                    (defaults, factory.clone().call(arguments))
                });
            (
                release.attribute(),
                Nixpkgs::new().call_package(&selected, release.arguments()),
            )
        }))
    })
    .call(factory.as_value())
}

#[track_caller]
fn host(i: &Inputs, field: &str) -> NixValue {
    i.stdenv().select("hostPlatform").select(field)
}

#[track_caller]
fn both(a: impl Into<NixValue>, b: impl Into<NixValue>) -> NixValue {
    NixValue::if_else(a, b, false)
}

#[track_caller]
fn concat(parts: impl IntoIterator<Item = NixValue>) -> NixValue {
    NixValue::builtin("concatLists").call(NixValue::list(parts))
}

#[track_caller]
fn at_least(i: &Inputs, version: NixValue, minimum: &str) -> NixValue {
    i.lib()
        .select("versionAtLeast")
        .apply([version, minimum.into()])
}

fn file(name: &str) -> NixValue {
    Nixpkgs::new().source_path(&format!("pkgs/servers/sql/mariadb/patch/{name}"))
}

fn common(i: &Inputs, lib: &NixLibrary) -> NixValue {
    let native = concat([
        NixValue::list([i.cmake(), i.pkg_config()]),
        lib.optional(host(i, "isDarwin"), i.fix_darwin_dylib_names()),
        lib.optional(!host(i, "isDarwin"), i.make_wrapper()),
    ]);
    let inputs = concat([
        NixValue::list([
            i.libiconv(),
            i.ncurses(),
            i.zlib(),
            i.pcre2(),
            i.openssl(),
            i.curl(),
        ]),
        lib.optionals(
            host(i, "isLinux"),
            concat([
                NixValue::list([i.libkrb5(), i.systemd()]),
                NixValue::if_else(
                    i.lib()
                        .select("versionOlder")
                        .apply([i.version(), "10.6".into()]),
                    NixValue::list([i.libaio()]),
                    NixValue::list([i.liburing()]),
                ),
            ]),
        ),
        lib.optionals(
            host(i, "isDarwin"),
            NixValue::list([i.core_services(), i.cctools(), i.perl(), i.libedit()]),
        ),
        lib.optionals(!host(i, "isDarwin"), NixValue::list([i.jemalloc()])),
    ]);
    let cmake_flags = concat([
        NixValue::list(
            [
                "-DBUILD_CONFIG=mysql_release",
                "-DMANUFACTURER=nixos.org",
                "-DDEFAULT_CHARSET=utf8mb4",
                "-DDEFAULT_COLLATION=utf8mb4_unicode_ci",
                "-DSECURITY_HARDENED=ON",
                "-DINSTALL_UNIX_ADDRDIR=/run/mysqld/mysqld.sock",
                "-DINSTALL_BINDIR=bin",
                "-DINSTALL_DOCDIR=share/doc/mysql",
                "-DINSTALL_DOCREADMEDIR=share/doc/mysql",
                "-DINSTALL_INCLUDEDIR=include/mysql",
                "-DINSTALL_LIBDIR=lib",
                "-DINSTALL_PLUGINDIR=lib/mysql/plugin",
                "-DINSTALL_INFODIR=share/mysql/docs",
                "-DINSTALL_MANDIR=share/man",
                "-DINSTALL_MYSQLSHAREDIR=share/mysql",
                "-DINSTALL_SCRIPTDIR=bin",
                "-DINSTALL_SUPPORTFILESDIR=share/doc/mysql",
                "-DINSTALL_MYSQLTESTDIR=OFF",
                "-DINSTALL_SQLBENCHDIR=OFF",
                "-DINSTALL_PAMDIR=share/pam/lib/security",
                "-DINSTALL_PAMDATADIR=share/pam/etc/security",
                "-DWITH_ZLIB=system",
                "-DWITH_SSL=system",
                "-DWITH_PCRE=system",
                "-DWITH_SAFEMALLOC=OFF",
                "-DWITH_UNIT_TESTS=OFF",
                "-DEMBEDDED_LIBRARY=OFF",
            ]
            .map(NixValue::from),
        ),
        lib.optionals(
            host(i, "isDarwin"),
            NixValue::list([
                "-DCONNECT_WITH_JDBC=OFF".into(),
                nix_text!(
                    "-DCURSES_LIBRARY={ncurses}/lib/libncurses.dylib",
                    ncurses = i.ncurses().select("out")
                ),
            ]),
        ),
        lib.optionals(
            both(host(i, "isDarwin"), at_least(i, i.version(), "10.6")),
            NixValue::list(["-Dhave_C__Wl___as_needed=".into()]),
        ),
        lib.optionals(
            !package::build_host_equal(i.stdenv()),
            NixValue::list([
                "-DSTACK_DIRECTION=-1".into(),
                nix_text!(
                    "-DCMAKE_CROSSCOMPILING_EMULATOR={emulator}",
                    emulator = host(i, "emulator").call(i.build_packages())
                ),
            ]),
        ),
    ]);
    let test_version = nix_text!(
        "mariadb_{version}",
        version = NixValue::builtin("replaceStrings").apply([
            NixValue::list([".".into()]),
            NixValue::list(["".into()]),
            i.lib().select("versions.majorMinor").call(i.version())
        ])
    );
    let tests = NixValue::function(|version| {
        NixValue::record(
            [
                "mariadb-galera-rsync",
                "mysql",
                "mysql-autobackup",
                "mysql-backup",
                "mysql-replication",
            ]
            .map(|name| {
                let scope = if name == "mariadb-galera-rsync" {
                    "mariadb-galera"
                } else {
                    name
                };
                (
                    name,
                    NixValue::builtin("getAttr")
                        .apply([version.clone(), i.nixos_tests().select(scope)]),
                )
            }),
        )
    })
    .call(test_version);
    Common {
        version: i.version(),
        src: i.fetchurl().call(NixValue::record([
            (
                "url",
                nix_text!(
                    "https://archive.mariadb.org/mariadb-{version}/source/mariadb-{version}.tar.gz",
                    version = i.version()
                ),
            ),
            ("hash", i.hash()),
        ])),
        outputs: vec!["out", "man"],
        native_build_inputs: native,
        build_inputs: inputs,
        pre_patch: scripts::pre_patch(),
        patches: concat([
            NixValue::list([file("cmake-includedir.patch")]),
            lib.optional(
                both(!host(i, "isLinux"), at_least(i, i.version(), "10.6")),
                file("macos-MDEV-26769-regression-fix.patch"),
            ),
        ]),
        cmake_flags,
        post_install: lib.optional_text(!i.with_embedded(), scripts::post_install_common()),
        post_fixup: lib.optional_text(
            !host(i, "isDarwin"),
            scripts::post_fixup(
                i.lib()
                    .select("makeBinPath")
                    .call(NixValue::list([i.less(), i.ncurses()])),
            ),
        ),
        passthru: NixValue::record([("tests", tests)]),
        meta: NixValue::record([
            (
                "description",
                "Enhanced, drop-in replacement for MySQL".into(),
            ),
            ("homepage", "https://mariadb.org/".into()),
            ("license", i.lib().select("licenses.gpl2Plus")),
            (
                "maintainers",
                concat([
                    NixValue::list([i.lib().select("maintainers.thoughtpolice")]),
                    i.lib().select("teams.helsinki-systems.members"),
                ]),
            ),
            ("platforms", i.lib().select("platforms.all")),
        ]),
    }
    .try_into_nix_value()
    .expect("fixed common MariaDB attributes")
}

fn client(i: &Inputs, lib: &NixLibrary, common: &NixValue) -> NixValue {
    i.stdenv()
        .select("mkDerivation")
        .call(common.clone().merge_attrs(NixValue::record([
            ("pname", "mariadb-client".into()),
            (
                "patches",
                concat([
                    common.clone().select("patches"),
                    NixValue::list([file("cmake-plugin-includedir.patch")]),
                ]),
            ),
            (
                "buildInputs",
                concat([
                    common.clone().select("buildInputs"),
                    lib.optionals(
                        at_least(i, common.clone().select("version"), "10.7"),
                        NixValue::list([i.fmt_8()]),
                    ),
                ]),
            ),
            (
                "cmakeFlags",
                concat([
                    common.clone().select("cmakeFlags"),
                    NixValue::list([
                        "-DPLUGIN_AUTH_PAM=NO".into(),
                        "-DWITHOUT_SERVER=ON".into(),
                        "-DWITH_WSREP=OFF".into(),
                        "-DINSTALL_MYSQLSHAREDIR=share/mysql-client".into(),
                    ]),
                ]),
            ),
            (
                "postInstall",
                NixValue::concat_text([
                    common.clone().select("postInstall"),
                    scripts::post_install_client(host(i, "extensions.sharedLibrary")),
                ]),
            ),
        ])))
}

fn server(i: &Inputs, lib: &NixLibrary, common: &NixValue) -> NixValue {
    // Perl's withPackages and its package scope remain backend values, excluded on Darwin.
    let mytop = i
        .build_packages()
        .select("perl.withPackages")
        .call(NixValue::function(|p| {
            NixValue::list([
                p.clone().select("DBDmysql"),
                p.clone().select("DBI"),
                p.select("TermReadKey"),
            ])
        }));
    let inputs = concat([
        common.clone().select("buildInputs"),
        NixValue::list([
            i.bzip2(),
            i.lz4(),
            i.lzo(),
            i.snappy(),
            i.xz(),
            i.zstd(),
            i.cracklib(),
            i.judy(),
            i.libevent(),
            i.libxml2(),
        ]),
        lib.optional(i.with_numa(), i.numactl()),
        lib.optionals(host(i, "isLinux"), NixValue::list([i.linux_pam()])),
        lib.optional(!host(i, "isDarwin"), mytop),
        lib.optionals(
            i.with_storage_mroonga(),
            NixValue::list([i.kytea(), i.libsodium(), i.msgpack(), i.zeromq()]),
        ),
        lib.optionals(
            at_least(i, common.clone().select("version"), "10.7"),
            NixValue::list([i.fmt_8()]),
        ),
    ]);
    let flags = concat([
        common.clone().select("cmakeFlags"),
        NixValue::list([
            "-DMYSQL_DATADIR=/var/lib/mysql".into(),
            "-DENABLED_LOCAL_INFILE=OFF".into(),
            "-DWITH_READLINE=ON".into(),
            "-DWITH_EXTRA_CHARSETS=all".into(),
            nix_text!(
                "-DWITH_EMBEDDED_SERVER={enabled}",
                enabled = NixValue::if_else(i.with_embedded(), "ON", "OFF")
            ),
            "-DWITH_UNIT_TESTS=OFF".into(),
            "-DWITH_WSREP=ON".into(),
            "-DWITH_INNODB_DISALLOW_WRITES=ON".into(),
            "-DWITHOUT_EXAMPLE=1".into(),
            "-DWITHOUT_FEDERATED=1".into(),
            "-DWITHOUT_TOKUDB=1".into(),
        ]),
        lib.optionals(i.with_numa(), NixValue::list(["-DWITH_NUMA=ON".into()])),
        lib.optionals(
            !i.with_storage_mroonga(),
            NixValue::list(["-DWITHOUT_MROONGA=1".into()]),
        ),
        lib.optionals(
            !i.with_storage_rocks(),
            NixValue::list(["-DWITHOUT_ROCKSDB=1".into()]),
        ),
        lib.optionals(
            both(!host(i, "isDarwin"), i.with_storage_rocks()),
            NixValue::list(["-DWITH_ROCKSDB_JEMALLOC=ON".into()]),
        ),
        lib.optionals(
            !host(i, "isDarwin"),
            NixValue::list(["-DWITH_JEMALLOC=yes".into()]),
        ),
        lib.optionals(
            host(i, "isDarwin"),
            NixValue::list([
                "-DPLUGIN_AUTH_PAM=NO".into(),
                "-DPLUGIN_AUTH_PAM_V1=NO".into(),
                "-DWITHOUT_OQGRAPH=1".into(),
                "-DWITHOUT_PLUGIN_S3=1".into(),
            ]),
        ),
    ]);
    i.stdenv()
        .select("mkDerivation")
        .call(common.clone().merge_attrs(NixValue::record([
            ("pname", "mariadb-server".into()),
            (
                "nativeBuildInputs",
                concat([
                    common.clone().select("nativeBuildInputs"),
                    NixValue::list([i.bison(), i.boost().select("dev"), i.flex()]),
                ]),
            ),
            ("buildInputs", inputs),
            (
                "propagatedBuildInputs",
                lib.optional(i.with_numa(), i.numactl()),
            ),
            ("postPatch", scripts::post_patch_server()),
            ("cmakeFlags", flags),
            (
                "preConfigure",
                lib.optional_text(!host(i, "isDarwin"), scripts::pre_configure()),
            ),
            (
                "postInstall",
                NixValue::concat_text([
                    common.clone().select("postInstall"),
                    scripts::post_install_server(),
                    lib.optional_text(i.with_storage_mroonga(), scripts::install_mroonga()),
                    lib.optional_text(
                        both(
                            !host(i, "isDarwin"),
                            at_least(i, common.clone().select("version"), "10.4"),
                        ),
                        scripts::install_pam(),
                    ),
                ]),
            ),
            (
                "CXXFLAGS",
                lib.optional_text(host(i, "isi686"), "-fpermissive"),
            ),
        ])))
}

#[derive(IntoRusnixValue)]
struct Common {
    version: NixValue,
    src: NixValue,
    outputs: Vec<&'static str>,
    native_build_inputs: NixValue,
    build_inputs: NixValue,
    pre_patch: NixValue,
    patches: NixValue,
    cmake_flags: NixValue,
    post_install: NixValue,
    post_fixup: NixValue,
    passthru: NixValue,
    meta: NixValue,
}
