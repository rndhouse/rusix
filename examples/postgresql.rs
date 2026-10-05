//! Configures PostgreSQL with ordinary Rust types, then lowers through Rusnix.
//! The private adapter below preserves the pinned NixOS implementation and final-option overrides.
use rusnix_ir::{
    interop::{NixValue, PackageRef},
    nixos::NixosModule,
};
use std::collections::BTreeMap;

// These types belong to this example; Rusnix core has no PostgreSQL model.
pub enum Clause {
    Preserve,
    Enable,
    Disable,
}

// None leaves a definition unset; Some(Preserve) explicitly preserves a privilege.
#[derive(Default)]
pub struct RoleClauses {
    pub superuser: Option<Clause>,
    pub createrole: Option<Clause>,
    pub createdb: Option<Clause>,
    pub inherit: Option<Clause>,
    pub login: Option<Clause>,
    pub replication: Option<Clause>,
    pub bypassrls: Option<Clause>,
}

pub struct Role {
    pub name: String,
    pub clauses: RoleClauses,
}

// An owned database creates the matching role; their names cannot disagree.
pub enum Database {
    Unowned(String),
    Owned { name: String, clauses: RoleClauses },
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

// Reuse upstream declarations, replacing only their configuration generation.
pub use lowering::implementation;

pub fn model() -> NixosModule {
    let postgres = Postgresql {
        enable: true,
        settings: BTreeMap::from([("max_connections".into(), 100_i64.into())]),
        databases: vec![Database::Owned {
            name: "app".into(),
            clauses: RoleClauses {
                login: Some(Clause::Enable),
                superuser: Some(Clause::Disable),
                ..RoleClauses::default()
            },
        }],
        ..Postgresql::default()
    };
    implementation().add(postgres)
}

// Compatibility details stay below the authoring surface, in the same file.
mod lowering {
    use super::{Clause, Database, Postgresql, Role, RoleClauses};
    use rusnix_ir::{
        self as rusnix, Config, Expr, IntoConfig,
        interop::{NixValue, Nixpkgs},
        nix_record as record, nix_text,
        nixos::{self, DefinitionPriority, NixosModule, OptionRef},
    };
    use std::collections::BTreeMap;

    impl Clause {
        fn value(self) -> NixValue {
            match self {
                Self::Preserve => NixValue::null(),
                Self::Enable => true.into(),
                Self::Disable => false.into(),
            }
        }
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
            ("ensureClauses", role_clauses(role.clauses)),
        ])
    }

    fn role_clauses(clauses: RoleClauses) -> NixValue {
        NixValue::record(
            [
                ("superuser", clauses.superuser),
                ("createrole", clauses.createrole),
                ("createdb", clauses.createdb),
                ("inherit", clauses.inherit),
                ("login", clauses.login),
                ("replication", clauses.replication),
                ("bypassrls", clauses.bypassrls),
            ]
            .into_iter()
            .filter_map(|(name, clause)| clause.map(|clause| (name, clause.value()))),
        )
    }

    // These declarations name final-option dependencies, not the NixOS schema.
    #[rusnix::options]
    mod options {
        use rusnix_ir::interop::NixValue;

        #[rusnix(root)]
        struct Root {
            services: Services,
        }

        struct Services {
            postgresql: Postgresql,
        }

        struct Postgresql {
            enable: bool,
            #[rusnix(rename = "enableJIT")]
            enable_jit: bool,
            #[rusnix(rename = "enableTCPIP")]
            enable_tcpip: bool,
            check_config: bool,
            data_dir: String,
            authentication: String,
            ident_map: String,
            super_user: String,
            package: NixValue,
            settings: Settings,
            extra_plugins: NixValue,
            initdb_args: Vec<String>,
            initial_script: Option<NixValue>,
            recovery_config: Option<String>,
            ensure_databases: Vec<String>,
            ensure_users: NixValue,
        }

        // Settings remain open-ended; also depend on the complete final attrset.
        #[rusnix(value)]
        struct Settings {
            port: i64,
            jit: String,
        }
    }

    // NixOS path options accept strings or paths; toString preserves their context.
    #[track_caller]
    fn path_text(value: Expr<String>) -> NixValue {
        NixValue::from(value).to_text()
    }

    #[track_caller]
    fn optional_file_script(value: NixValue, text: NixValue) -> NixValue {
        NixValue::if_else(value.equals(NixValue::null()), "", text)
    }

    fn effective_package() -> NixValue {
        let pg = options::root().services.postgresql;
        let package = pg.package();
        let base = NixValue::if_else(
            pg.enable_jit(),
            package.clone().select("withJIT"),
            package.select("withoutJIT"),
        );
        NixValue::if_else(
            pg.extra_plugins().equals(NixValue::list([])),
            base.clone(),
            base.select("withPackages").call(pg.extra_plugins()),
        )
    }

    fn default_package() -> NixValue {
        let pg = options::root().services.postgresql;
        let removed = |version: &str| {
            Nixpkgs::new().function("throwIfNot").apply([
                false.into(),
                format!(
                    "postgresql_{version} was removed, please upgrade your postgresql version."
                )
                .into(),
                NixValue::null(),
            ])
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
                Nixpkgs::new().function("versionAtLeast").apply([
                    OptionRef::<String>::new("system.stateVersion").into_value(),
                    state_version.into(),
                ]),
                candidate,
                package,
            );
        }
        NixValue::if_else(pg.enable_jit(), package.clone().select("withJIT"), package)
    }

    fn settings_text() -> NixValue {
        let pg = options::root().services.postgresql;
        let printable = Nixpkgs::new().function("filterAttrs").apply([
            NixValue::function(|_| {
                NixValue::function(|value| {
                    NixValue::if_else(value.equals(NixValue::null()), false, true)
                })
            }),
            pg.settings.as_value(),
        ]);
        let lines = Nixpkgs::new().function("mapAttrsToList").apply([
            NixValue::function(|name| {
                NixValue::function(|value| {
                    let quoted = nix_text!(
                        "'{value}'",
                        value = Nixpkgs::new().function("replaceStrings").apply([
                            NixValue::list(["'".into()]),
                            NixValue::list(["''".into()]),
                            value.clone()
                        ]),
                    );
                    let rendered = NixValue::if_else(
                        value.clone().equals(true),
                        "yes",
                        NixValue::if_else(
                            value.clone().equals(false),
                            "no",
                            NixValue::if_else(
                                Nixpkgs::new().function("isString").apply([value.clone()]),
                                quoted,
                                value.to_text(),
                            ),
                        ),
                    );
                    nix_text!("{name} = {rendered}", name = name, rendered = rendered)
                })
            }),
            printable,
        ]);
        NixValue::join_text("\n", lines)
    }

    fn configuration_file() -> NixValue {
        Nixpkgs::from_module()
            .package_function("writeTextDir")
            .apply(["postgresql.conf".into(), settings_text()])
    }

    fn pre_start() -> NixValue {
        let pg = options::root().services.postgresql;
        let data = path_text(pg.data_dir());
        let recovery = optional_file_script(
            pg.recovery_config(),
            nix_text!(
                r#"ln -sfn "{file}" \
  "{data}/recovery.conf"
"#,
                file = Nixpkgs::from_module()
                    .package_function("writeText")
                    .apply(["recovery.conf".into(), pg.recovery_config()]),
                data = data.clone(),
            ),
        );
        // Raw templates keep their whitespace; the holes remain deferred Nix values.
        nix_text!(
            r#"if ! test -e {data}/PG_VERSION; then
  # Cleanup the data directory.
  rm -f {data}/*.conf

  # Initialise the database.
  initdb -U {super_user} {initdb_args}

  # See postStart!
  touch "{data}/.first_startup"
fi

ln -sfn "{config_file}/postgresql.conf" "{data}/postgresql.conf"
{recovery}
"#,
            data = data,
            super_user = pg.super_user(),
            initdb_args = Nixpkgs::new()
                .function("escapeShellArgs")
                .apply([pg.initdb_args()]),
            config_file = configuration_file(),
            recovery = recovery,
        )
    }

    fn post_start() -> NixValue {
        let pg = options::root().services.postgresql;
        let data = path_text(pg.data_dir());
        let initial = optional_file_script(
            pg.initial_script(),
            nix_text!(
                r#"$PSQL -f "{script}" -d postgres
"#,
                script = pg.initial_script(),
            ),
        );
        let databases = Nixpkgs::new().function("concatMapStrings").apply([
            NixValue::function(|database| {
                nix_text!(
                    r#"$PSQL -tAc "SELECT 1 FROM pg_database WHERE datname = '{database}'" | grep -q 1 || $PSQL -tAc 'CREATE DATABASE "{database}"'
"#,
                    database = database,
                )
            }),
            pg.ensure_databases(),
        ]);
        let users = Nixpkgs::new().function("concatMapStrings").apply([
            NixValue::function(|user| {
                let name = user.clone().select("name");
                let clauses = Nixpkgs::new().function("filterAttrs").apply([
                    NixValue::function(|_| {
                        NixValue::function(|value| {
                            NixValue::if_else(value.equals(NixValue::null()), false, true)
                        })
                    }),
                    user.clone().select("ensureClauses"),
                ]);
                let clauses = Nixpkgs::new().function("attrValues").apply([Nixpkgs::new()
                    .function("mapAttrs")
                    .apply([
                        NixValue::function(|name| {
                            NixValue::function(|enabled| {
                                NixValue::if_else(enabled, name.clone(), nix_text!("no{name}", name = name))
                            })
                        }),
                        clauses,
                    ])]);
                let ownership = NixValue::if_else(
                    user.select("ensureDBOwnership"),
                    nix_text!(
                        r#"$PSQL -tAc 'ALTER DATABASE "{name}" OWNER TO "{name}";' "#,
                        name = name.clone(),
                    ),
                    "",
                );
                nix_text!(
                    r#"$PSQL -tAc "SELECT 1 FROM pg_roles WHERE rolname='{name}'" | grep -q 1 || $PSQL -tAc 'CREATE USER "{name}"'
$PSQL -tAc 'ALTER ROLE "{name}" {clauses}'{trailing_space}

{ownership}
"#,
                    name = name,
                    clauses = NixValue::join_text(" ", clauses),
                    // Preserve the upstream command's trailing space explicitly.
                    trailing_space = " ",
                    ownership = ownership,
                )
            }),
            pg.ensure_users(),
        ]);
        nix_text!(
            r#"PSQL="psql --port={port}"

while ! $PSQL -d postgres -c "" 2> /dev/null; do
    if ! kill -0 "$MAINPID"; then exit 1; fi
    sleep 0.1
done

if test -e "{data}/.first_startup"; then
  {initial}
  rm -f "{data}/.first_startup"
fi
{databases}{users}
"#,
            port = pg.settings.port(),
            data = data,
            initial = initial,
            databases = NixValue::if_else(
                pg.ensure_databases().equals(NixValue::list([])),
                "",
                nix_text!("{databases}\n", databases = databases),
            ),
            users = users,
        )
    }

    fn assertions() -> NixValue {
        let pg = options::root().services.postgresql;
        Nixpkgs::new().function("map").apply([
            NixValue::function(|user| {
                let name = user.clone().select("name");
                record! {
                    "assertion": NixValue::if_else(user.select("ensureDBOwnership"), Nixpkgs::new().function("elem").apply([name.clone(), pg.ensure_databases()]), true),
                    "message": nix_text!(
                        r#"For each database user defined with `services.postgresql.ensureUsers` and
`ensureDBOwnership = true;`, a database with the same name must be defined
in `services.postgresql.ensureDatabases`.

Offender: {name} has not been found among databases.
"#,
                        name = name,
                    ),
                }
            }),
            pg.ensure_users(),
        ])
    }

    fn service_config() -> NixValue {
        let pg = options::root().services.postgresql;
        let package = effective_package();
        let group_access = Nixpkgs::new()
            .function("versionAtLeast")
            .apply([package.clone().select("version"), "11.0".into()]);
        let data = NixValue::from(pg.data_dir());
        let standard_data = nix_text!(
            "/var/lib/postgresql/{schema}",
            schema = pg.package().select("psqlSchema")
        );
        let properties = record! {
            "ExecReload": nix_text!("{coreutils}/bin/kill -HUP $MAINPID", coreutils = Nixpkgs::from_module().get("coreutils")),
            "User": "postgres", "Group": "postgres", "RuntimeDirectory": "postgresql",
            "Type": NixValue::if_else(Nixpkgs::new().function("versionAtLeast").apply([pg.package().select("version"), "9.6".into()]), "notify", "simple"),
            "KillSignal": "SIGINT", "KillMode": "mixed", "TimeoutSec": 120_i64,
            "ExecStart": nix_text!("{package}/bin/postgres", package = package),
            "CapabilityBoundingSet": NixValue::list(["".into()]), "DevicePolicy": "closed",
            "PrivateTmp": true, "ProtectHome": true, "ProtectSystem": "strict",
            "MemoryDenyWriteExecute": NixValue::from(pg.settings.jit()).equals("off").priority(DefinitionPriority::Default),
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
        nixos::merge([
            properties,
            record! { "ReadWritePaths": NixValue::list([data.clone()]) }
                .when(NixValue::if_else(data.clone().equals("/var/lib/postgresql"), false, true)),
            record! {
                "StateDirectory": nix_text!("postgresql postgresql/{schema}", schema = pg.package().select("psqlSchema")),
                "StateDirectoryMode": NixValue::if_else(group_access, "0750", "0700"),
            }.when(data.equals(standard_data)),
        ])
    }

    fn service() -> NixValue {
        let pg = options::root().services.postgresql;
        record! {
            "description": "PostgreSQL Server",
            "wantedBy": NixValue::list(["multi-user.target".into()]),
            "after": NixValue::list(["network.target".into()]),
            "environment": record! { "PGDATA": pg.data_dir() },
            "path": NixValue::list([effective_package()]),
            "preStart": pre_start(), "postStart": post_start(),
            "serviceConfig": service_config(),
            "unitConfig": record! { "RequiresMountsFor": path_text(pg.data_dir()) },
        }
    }

    fn settings() -> NixValue {
        let pg = options::root().services.postgresql;
        record! {
            "hba_file": Nixpkgs::from_module().package_function("writeText").apply(["pg_hba.conf".into(), pg.authentication().into()]).to_text(),
            "ident_file": Nixpkgs::from_module().package_function("writeText").apply(["pg_ident.conf".into(), pg.ident_map().into()]).to_text(),
            "log_destination": "stderr",
            "listen_addresses": NixValue::if_else(pg.enable_tcpip(), "*", "localhost"),
            "jit": NixValue::if_else(pg.enable_jit(), "on", "off").priority(DefinitionPriority::Default),
        }
    }

    fn checks() -> NixValue {
        let pg = options::root().services.postgresql;
        let check = Nixpkgs::from_module()
            .package_function("runCommand")
            .apply([
                "postgresql-configfile-check".into(),
                record! {},
                nix_text!(
                    r#"{package}/bin/postgres -D{config_file} -C config_file >/dev/null
touch $out
"#,
                    package = pg.package(),
                    config_file = configuration_file(),
                ),
            ]);
        let pkgs = Nixpkgs::from_module();
        let native = pkgs
            .value("stdenv.hostPlatform")
            .equals(pkgs.value("stdenv.buildPlatform"));
        let enabled = NixValue::if_else(pg.check_config(), native, false);
        Nixpkgs::new().function("optional").apply([enabled, check])
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
            let pg = options::root().services.postgresql;
            let enabled = pg.enable();
            let guarded = |value: NixValue| value.when(enabled.clone());
            let authentication = nixos::merge([
                NixValue::from("# Generated file; do not edit!").before(),
                NixValue::from("# default value of services.postgresql.authentication\nlocal all all              peer\nhost  all all 127.0.0.1/32 md5\nhost  all all ::1/128      md5\n").after(),
            ]);
            Implementation {
                assertions: guarded(assertions()),
                services: Services {
                    postgresql: guarded(record! {
                        "settings": settings(),
                        "package": default_package().priority(DefinitionPriority::Default),
                        "dataDir": nix_text!("/var/lib/postgresql/{schema}", schema = pg.package().select("psqlSchema")).priority(DefinitionPriority::Default),
                        "authentication": authentication,
                    }),
                },
                users: guarded(record! {
                    "users": record! { "postgres": record! {
                        "name": "postgres", "uid": OptionRef::<i64>::new("ids.uids.postgres").into_expr(), "group": "postgres",
                        "description": "PostgreSQL server user", "home": path_text(pg.data_dir()), "useDefaultShell": true,
                    } },
                    "groups": record! { "postgres": record! { "gid": OptionRef::<i64>::new("ids.gids.postgres").into_expr() } },
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
}

fn main() {
    let artifact = rusnix_nix::nixos::compile_module(&model()).unwrap();
    println!("{}", artifact.module.source);
}
