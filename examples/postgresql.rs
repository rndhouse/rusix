//! Replaces the pinned PostgreSQL module's implementation, retaining its NixOS schema.
//! Rust models provisioning; finite symbolic dependencies preserve ordinary Nix overrides.
use rusnix_ir::{self as rusnix, Config, IntoConfig};
use rusnix_ir::{
    interop::{NixValue, Nixpkgs, PackageRef},
    nixos::{NixosModule, OptionRef},
};
use std::collections::BTreeMap;

// Preserve leaves existing privileges alone, rather than revoking them.
pub enum Clause {
    Preserve,
    Enable,
    Disable,
}

impl Clause {
    fn value(self) -> NixValue {
        match self {
            Self::Preserve => NixValue::null(),
            Self::Enable => true.into(),
            Self::Disable => false.into(),
        }
    }
}

pub struct Role {
    pub name: String,
    pub clauses: BTreeMap<String, Clause>,
}

// An owned database creates the matching role; their names cannot disagree.
pub enum Database {
    Unowned(String),
    Owned {
        name: String,
        clauses: BTreeMap<String, Clause>,
    },
}

// None leaves an upstream default alone. Package and settings schemas stay in Nix.
#[derive(Default)]
pub struct Postgresql {
    pub enable: bool,
    pub package: Option<PackageRef>,
    pub enable_jit: Option<bool>,
    pub enable_tcpip: Option<bool>,
    pub check_config: Option<bool>,
    pub extensions: Vec<String>,
    pub data_dir: Option<String>,
    pub settings: BTreeMap<String, NixValue>,
    pub authentication: Option<String>,
    pub ident_map: Option<String>,
    pub initdb_args: Vec<String>,
    pub initial_script: Option<NixValue>,
    pub recovery_config: Option<String>,
    pub databases: Vec<Database>,
    pub roles: Vec<Role>,
}

impl IntoConfig for Postgresql {
    #[track_caller]
    fn into_config(self) -> Config {
        let mut fields = BTreeMap::from([("enable", self.enable.into())]);
        macro_rules! optional {
            ($name:literal, $value:expr) => {
                if let Some(value) = $value {
                    fields.insert($name, value.into());
                }
            };
        }
        optional!("package", self.package);
        optional!("enableJIT", self.enable_jit);
        optional!("enableTCPIP", self.enable_tcpip);
        optional!("checkConfig", self.check_config);
        optional!("dataDir", self.data_dir);
        optional!("authentication", self.authentication);
        optional!("identMap", self.ident_map);
        optional!("initialScript", self.initial_script);
        optional!("recoveryConfig", self.recovery_config);
        if !self.extensions.is_empty() {
            fields.insert(
                "extraPlugins",
                NixValue::function(move |packages| {
                    NixValue::list(
                        self.extensions
                            .into_iter()
                            .map(|name| packages.clone().select(&name)),
                    )
                }),
            );
        }
        if !self.settings.is_empty() {
            fields.insert("settings", NixValue::record(self.settings));
        }
        if !self.initdb_args.is_empty() {
            fields.insert(
                "initdbArgs",
                NixValue::list(self.initdb_args.into_iter().map(NixValue::from)),
            );
        }
        let mut databases = Vec::new();
        let mut roles = Vec::new();
        for database in self.databases {
            match database {
                Database::Unowned(name) => databases.push(name.into()),
                Database::Owned { name, clauses } => {
                    databases.push(name.clone().into());
                    roles.push(role_value(Role { name, clauses }, true));
                }
            }
        }
        roles.extend(self.roles.into_iter().map(|role| role_value(role, false)));
        if !databases.is_empty() {
            fields.insert("ensureDatabases", NixValue::list(databases));
        }
        if !roles.is_empty() {
            fields.insert("ensureUsers", NixValue::list(roles));
        }
        config::inputs(NixValue::record(fields)).into_config()
    }
}

fn role_value(role: Role, owns_database: bool) -> NixValue {
    NixValue::record([
        ("name", role.name.into()),
        ("ensureDBOwnership", owns_database.into()),
        (
            "ensureClauses",
            NixValue::record(
                role.clauses
                    .into_iter()
                    .map(|(name, clause)| (name, clause.value())),
            ),
        ),
    ])
}

// These helpers construct IR, not Nix source. Library functions own traversal.
#[track_caller]
fn option(path: &str) -> NixValue {
    OptionRef::<NixValue>::new(path).into_value()
}

#[track_caller]
fn pg(name: &str) -> NixValue {
    option(&format!("services.postgresql.{name}"))
}

#[track_caller]
fn lib(name: &str, arguments: impl IntoIterator<Item = NixValue>) -> NixValue {
    arguments
        .into_iter()
        .fold(Nixpkgs::new().function(name).as_value(), NixValue::call)
}

#[track_caller]
fn pkg(name: &str, arguments: impl IntoIterator<Item = NixValue>) -> NixValue {
    arguments.into_iter().fold(
        Nixpkgs::from_module().package_function(name).as_value(),
        NixValue::call,
    )
}

#[track_caller]
fn join(separator: &str, parts: impl IntoIterator<Item = NixValue>) -> NixValue {
    lib(
        "concatStringsSep",
        [separator.into(), NixValue::list(parts)],
    )
}

macro_rules! text {
    ($($part:expr),* $(,)?) => { join("", [$(NixValue::from($part)),*]) };
}

macro_rules! record {
    ($($name:literal: $value:expr),* $(,)?) => { NixValue::record([$(($name, NixValue::from($value))),*]) };
}

#[track_caller]
fn when(condition: NixValue, value: NixValue) -> NixValue {
    lib("mkIf", [condition, value])
}

#[track_caller]
fn default(value: NixValue) -> NixValue {
    lib("mkDefault", [value])
}

#[track_caller]
fn present(value: NixValue, text: NixValue) -> NixValue {
    NixValue::if_else(value.equals(NixValue::null()), "", text)
}

fn effective_package() -> NixValue {
    let package = pg("package");
    let base = NixValue::if_else(
        pg("enableJIT"),
        package.clone().select("withJIT"),
        package.select("withoutJIT"),
    );
    NixValue::if_else(
        pg("extraPlugins").equals(NixValue::list([])),
        base.clone(),
        base.select("withPackages").call(pg("extraPlugins")),
    )
}

fn default_package() -> NixValue {
    let removed = |version: &str| {
        lib(
            "throwIfNot",
            [
                false.into(),
                format!(
                    "postgresql_{version} was removed, please upgrade your postgresql version."
                )
                .into(),
                NixValue::null(),
            ],
        )
    };
    let mut package = removed("9_5");
    for (state_version, candidate) in [
        ("17.09", removed("9_6")),
        ("20.03", removed("11")),
        ("21.11", Nixpkgs::from_module().get("postgresql_13").into()),
        ("22.05", Nixpkgs::from_module().get("postgresql_14").into()),
        ("23.11", Nixpkgs::from_module().get("postgresql_15").into()),
        ("24.11", Nixpkgs::from_module().get("postgresql_16").into()),
    ] {
        package = NixValue::if_else(
            lib(
                "versionAtLeast",
                [option("system.stateVersion"), state_version.into()],
            ),
            candidate,
            package,
        );
    }
    NixValue::if_else(pg("enableJIT"), package.clone().select("withJIT"), package)
}

fn settings_text() -> NixValue {
    let printable = lib(
        "filterAttrs",
        [
            NixValue::function(|_| {
                NixValue::function(|value| {
                    NixValue::if_else(value.equals(NixValue::null()), false, true)
                })
            }),
            pg("settings"),
        ],
    );
    let lines = lib(
        "mapAttrsToList",
        [
            NixValue::function(|name| {
                NixValue::function(|value| {
                    let quoted = text!(
                        "'",
                        lib(
                            "replaceStrings",
                            [
                                NixValue::list(["'".into()]),
                                NixValue::list(["''".into()]),
                                value.clone()
                            ]
                        ),
                        "'"
                    );
                    let rendered = NixValue::if_else(
                        value.clone().equals(true),
                        "yes",
                        NixValue::if_else(
                            value.clone().equals(false),
                            "no",
                            NixValue::if_else(
                                lib("isString", [value.clone()]),
                                quoted,
                                value.to_text(),
                            ),
                        ),
                    );
                    text!(name, " = ", rendered)
                })
            }),
            printable,
        ],
    );
    lib("concatStringsSep", ["\n".into(), lines])
}

fn configuration_file() -> NixValue {
    pkg("writeTextDir", ["postgresql.conf".into(), settings_text()])
}

fn pre_start() -> NixValue {
    let data = pg("dataDir").to_text();
    let recovery = present(
        pg("recoveryConfig"),
        text!(
            "ln -sfn \"",
            pkg("writeText", ["recovery.conf".into(), pg("recoveryConfig")]).to_text(),
            "\" \\\n  \"",
            data.clone(),
            "/recovery.conf\"\n",
        ),
    );
    text!(
        "if ! test -e ",
        data.clone(),
        "/PG_VERSION; then\n",
        "  # Cleanup the data directory.\n  rm -f ",
        data.clone(),
        "/*.conf\n\n",
        "  # Initialise the database.\n  initdb -U ",
        pg("superUser"),
        " ",
        lib("escapeShellArgs", [pg("initdbArgs")]),
        "\n\n  # See postStart!\n  touch \"",
        data.clone(),
        "/.first_startup\"\nfi\n\n",
        "ln -sfn \"",
        configuration_file().to_text(),
        "/postgresql.conf\" \"",
        data,
        "/postgresql.conf\"\n",
        recovery,
        "\n",
    )
}

fn post_start() -> NixValue {
    let data = pg("dataDir").to_text();
    let initial = present(
        pg("initialScript"),
        text!(
            "$PSQL -f \"",
            pg("initialScript").to_text(),
            "\" -d postgres\n"
        ),
    );
    let databases = lib(
        "concatMapStrings",
        [
            NixValue::function(|database| {
                text!(
                    "$PSQL -tAc \"SELECT 1 FROM pg_database WHERE datname = '",
                    database.clone(),
                    "'\" | grep -q 1 || $PSQL -tAc 'CREATE DATABASE \"",
                    database,
                    "\"'\n",
                )
            }),
            pg("ensureDatabases"),
        ],
    );
    let users = lib(
        "concatMapStrings",
        [
            NixValue::function(|user| {
                let name = user.clone().select("name");
                let clauses = lib(
                    "filterAttrs",
                    [
                        NixValue::function(|_| {
                            NixValue::function(|value| {
                                NixValue::if_else(value.equals(NixValue::null()), false, true)
                            })
                        }),
                        user.clone().select("ensureClauses"),
                    ],
                );
                let clauses = lib(
                    "attrValues",
                    [lib(
                        "mapAttrs",
                        [
                            NixValue::function(|name| {
                                NixValue::function(|enabled| {
                                    NixValue::if_else(enabled, name.clone(), text!("no", name))
                                })
                            }),
                            clauses,
                        ],
                    )],
                );
                let ownership = NixValue::if_else(
                    user.select("ensureDBOwnership"),
                    text!(
                        "$PSQL -tAc 'ALTER DATABASE \"",
                        name.clone(),
                        "\" OWNER TO \"",
                        name.clone(),
                        "\";' "
                    ),
                    "",
                );
                text!(
                    "$PSQL -tAc \"SELECT 1 FROM pg_roles WHERE rolname='",
                    name.clone(),
                    "'\" | grep -q 1 || $PSQL -tAc 'CREATE USER \"",
                    name.clone(),
                    "\"'\n",
                    "$PSQL -tAc 'ALTER ROLE \"",
                    name,
                    "\" ",
                    lib("concatStringsSep", [" ".into(), clauses]),
                    "' \n\n",
                    ownership,
                    "\n"
                )
            }),
            pg("ensureUsers"),
        ],
    );
    text!(
        "PSQL=\"psql --port=",
        pg("settings.port").to_text(),
        "\"\n\n",
        "while ! $PSQL -d postgres -c \"\" 2> /dev/null; do\n    if ! kill -0 \"$MAINPID\"; then exit 1; fi\n    sleep 0.1\ndone\n\n",
        "if test -e \"",
        data.clone(),
        "/.first_startup\"; then\n  ",
        initial,
        "\n  rm -f \"",
        data,
        "/.first_startup\"\nfi\n",
        NixValue::if_else(
            pg("ensureDatabases").equals(NixValue::list([])),
            "",
            text!(databases, "\n")
        ),
        users,
        "\n",
    )
}

fn assertions() -> NixValue {
    lib(
        "map",
        [
            NixValue::function(|user| {
                let name = user.clone().select("name");
                record! {
                    "assertion": NixValue::if_else(user.select("ensureDBOwnership"), lib("elem", [name.clone(), pg("ensureDatabases")]), true),
                    "message": text!("For each database user defined with `services.postgresql.ensureUsers` and\n`ensureDBOwnership = true;`, a database with the same name must be defined\nin `services.postgresql.ensureDatabases`.\n\nOffender: ", name, " has not been found among databases.\n"),
                }
            }),
            pg("ensureUsers"),
        ],
    )
}

fn service_config() -> NixValue {
    let package = effective_package();
    let group_access = lib(
        "versionAtLeast",
        [package.clone().select("version"), "11.0".into()],
    );
    let data = pg("dataDir");
    let standard_data = text!("/var/lib/postgresql/", pg("package").select("psqlSchema"));
    let properties = record! {
        "ExecReload": text!(Nixpkgs::from_module().get("coreutils").as_value().to_text(), "/bin/kill -HUP $MAINPID"),
        "User": "postgres", "Group": "postgres", "RuntimeDirectory": "postgresql",
        "Type": NixValue::if_else(lib("versionAtLeast", [pg("package").select("version"), "9.6".into()]), "notify", "simple"),
        "KillSignal": "SIGINT", "KillMode": "mixed", "TimeoutSec": 120_i64,
        "ExecStart": text!(package.to_text(), "/bin/postgres"),
        "CapabilityBoundingSet": NixValue::list(["".into()]), "DevicePolicy": "closed",
        "PrivateTmp": true, "ProtectHome": true, "ProtectSystem": "strict",
        "MemoryDenyWriteExecute": default(pg("settings.jit").equals("off")),
        "NoNewPrivileges": true, "LockPersonality": true, "PrivateDevices": true, "PrivateMounts": true,
        "ProcSubset": "pid", "ProtectClock": true, "ProtectControlGroups": true, "ProtectHostname": true,
        "ProtectKernelLogs": true, "ProtectKernelModules": true, "ProtectKernelTunables": true,
        "ProtectProc": "invisible", "RemoveIPC": true,
        "RestrictAddressFamilies": NixValue::list(["AF_INET", "AF_INET6", "AF_NETLINK", "AF_UNIX"].map(NixValue::from)),
        "RestrictNamespaces": true, "RestrictRealtime": true, "RestrictSUIDSGID": true,
        "SystemCallArchitectures": "native",
        "SystemCallFilter": NixValue::list(["@system-service".into(), "~@privileged @resources".into()]),
        "UMask": NixValue::if_else(group_access.clone(), "0027", "0077"),
    };
    lib(
        "mkMerge",
        [NixValue::list([
            properties,
            when(
                NixValue::if_else(data.clone().equals("/var/lib/postgresql"), false, true),
                record! { "ReadWritePaths": NixValue::list([data.clone()]) },
            ),
            when(
                data.equals(standard_data),
                record! {
                    "StateDirectory": text!("postgresql postgresql/", pg("package").select("psqlSchema")),
                    "StateDirectoryMode": NixValue::if_else(group_access, "0750", "0700"),
                },
            ),
        ])],
    )
}

fn service() -> NixValue {
    record! {
        "description": "PostgreSQL Server",
        "wantedBy": NixValue::list(["multi-user.target".into()]),
        "after": NixValue::list(["network.target".into()]),
        "environment": record! { "PGDATA": pg("dataDir") },
        "path": NixValue::list([effective_package()]),
        "preStart": pre_start(), "postStart": post_start(),
        "serviceConfig": service_config(),
        "unitConfig": record! { "RequiresMountsFor": pg("dataDir").to_text() },
    }
}

fn settings() -> NixValue {
    record! {
        "hba_file": pkg("writeText", ["pg_hba.conf".into(), pg("authentication")]).to_text(),
        "ident_file": pkg("writeText", ["pg_ident.conf".into(), pg("identMap")]).to_text(),
        "log_destination": "stderr",
        "listen_addresses": NixValue::if_else(pg("enableTCPIP"), "*", "localhost"),
        "jit": default(NixValue::if_else(pg("enableJIT"), "on", "off")),
    }
}

fn checks() -> NixValue {
    let check = pkg(
        "runCommand",
        [
            "postgresql-configfile-check".into(),
            NixValue::record([] as [(&str, NixValue); 0]),
            text!(
                pg("package").to_text(),
                "/bin/postgres -D",
                configuration_file().to_text(),
                " -C config_file >/dev/null\ntouch $out\n"
            ),
        ],
    );
    let pkgs = Nixpkgs::from_module();
    let native = pkgs
        .value("stdenv.hostPlatform")
        .equals(pkgs.value("stdenv.buildPlatform"));
    let enabled = NixValue::if_else(pg("checkConfig"), native, false);
    lib("optional", [enabled, check])
}

#[rusnix::config]
mod config {
    use super::*;

    #[rusnix(root)]
    pub struct Inputs {
        services: Services,
    }

    struct Services {
        postgresql: NixValue,
    }

    #[rusnix(root)]
    pub struct Implementation {
        assertions: NixValue,
        services: Services,
        users: NixValue,
        environment: NixValue,
        system: NixValue,
        systemd: NixValue,
    }

    pub fn inputs(postgresql: NixValue) -> Inputs {
        Inputs {
            services: Services { postgresql },
        }
    }

    pub fn implementation() -> Implementation {
        let enabled = pg("enable");
        let guarded = |value| when(enabled.clone(), value);
        let authentication = lib("mkMerge", [NixValue::list([
            lib("mkBefore", ["# Generated file; do not edit!".into()]),
            lib("mkAfter", ["# default value of services.postgresql.authentication\nlocal all all              peer\nhost  all all 127.0.0.1/32 md5\nhost  all all ::1/128      md5\n".into()]),
        ])]);
        Implementation {
            assertions: guarded(assertions()),
            services: Services {
                postgresql: guarded(record! {
                    "settings": settings(),
                    "package": default(default_package()),
                    "dataDir": default(text!("/var/lib/postgresql/", pg("package").select("psqlSchema"))),
                    "authentication": authentication,
                }),
            },
            users: guarded(record! {
                "users": record! { "postgres": record! {
                    "name": "postgres", "uid": option("ids.uids.postgres"), "group": "postgres",
                    "description": "PostgreSQL server user", "home": pg("dataDir").to_text(), "useDefaultShell": true,
                } },
                "groups": record! { "postgres": record! { "gid": option("ids.gids.postgres") } },
            }),
            environment: guarded(record! {
                "systemPackages": NixValue::list([effective_package()]),
                "pathsToLink": NixValue::list(["/share/postgresql".into()]),
            }),
            system: guarded(record! { "checks": checks() }),
            systemd: guarded(record! { "services": record! { "postgresql": service() } }),
        }
    }
}

// Reuse the upstream public schema, but replace its implementation with this contribution.
pub fn implementation() -> NixosModule {
    NixosModule::empty().add(config::implementation())
}

pub fn model() -> NixosModule {
    let postgres = Postgresql {
        enable: true,
        settings: BTreeMap::from([("max_connections".into(), 100_i64.into())]),
        databases: vec![Database::Owned {
            name: "app".into(),
            clauses: BTreeMap::from([
                ("login".into(), Clause::Enable),
                ("superuser".into(), Clause::Disable),
            ]),
        }],
        ..Postgresql::default()
    };
    implementation().add(postgres)
}

fn main() {
    let artifact = rusnix_nix::nixos::compile_module(&model()).unwrap();
    println!("{}", artifact.module.source);
}
