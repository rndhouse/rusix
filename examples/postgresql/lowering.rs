//! Configuration implementation / lowering compatible with the pinned NixOS PostgreSQL module.
//! Final-option dependencies stay symbolic; public option declarations are still supplied upstream.
use super::{
    model::{Clause, Database, Postgresql, Role, RoleClauses},
    options,
};
use rusnix_ir::{
    self as rusnix, Config, Expr, IntoConfig, IntoRusnixValue,
    interop::{NixValue, Nixpkgs},
    nix_record as record, nix_text,
    nixos::{self, DefinitionPriority, NixosModule},
};
use std::collections::BTreeMap;

impl Clause {
    fn value(self) -> NixValue {
        // null means preserve; false must instead generate an explicit NO... clause.
        match self {
            Self::Preserve => NixValue::null(),
            Self::Enable => true.into(),
            Self::Disable => false.into(),
        }
    }
}

// This adapter has domain semantics: omit unspecified inputs and derive matching owner roles.
// The structural roots below determine placement under services.postgresql.
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

    // The upstream empty-list special case avoids calling withPackages at all.
    // Otherwise nixpkgs resolves extensions for the selected package variant.
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
            format!("postgresql_{version} was removed, please upgrade your postgresql version.")
                .into(),
            NixValue::null(),
        ])
    };

    // Preserve state-version defaults and lazy errors for removed versions.
    // A later package definition can override this default without forcing those errors.
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
                options::root().system.state_version().into(),
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

    // null omits a setting; the callbacks run in Nix over the final merged settings.
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

                // Match upstream boolean spelling and PostgreSQL single-quote escaping.
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
    // This opaque helper call produces a derivation, retaining store-path string context.
    // Rust describes its inputs; nixpkgs owns file generation and nothing is built here.
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
            r#"
                ln -sfn "{file}" "{data}/recovery.conf"
            "#,
            file = Nixpkgs::from_module()
                .package_function("writeText")
                .apply(["recovery.conf".into(), pg.recovery_config()]),
            data = data.clone(),
        ),
    );

    // Leading-newline blocks dedent the shell text; holes remain deferred Nix values.
    nix_text!(
        r#"
            if ! test -e {data}/PG_VERSION; then
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
            r#"
                $PSQL -f "{script}" -d postgres
            "#,
            script = pg.initial_script(),
        ),
    );

    let databases = Nixpkgs::new().function("concatMapStrings").apply([
        NixValue::function(|database| {
            nix_text!(
                r#"
                    $PSQL -tAc "SELECT 1 FROM pg_database WHERE datname = '{database}'" | grep -q 1 || $PSQL -tAc 'CREATE DATABASE "{database}"'
                "#,
                database = database,
            )
        }),
        pg.ensure_databases(),
    ]);

    let users = Nixpkgs::new().function("concatMapStrings").apply([
        NixValue::function(|user| {
            let name = user.clone().select("name");

            // Preserve leaves a clause out of ALTER ROLE; Disable emits NO plus its name.
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
                r#"
                    $PSQL -tAc "SELECT 1 FROM pg_roles WHERE rolname='{name}'" | grep -q 1 || $PSQL -tAc 'CREATE USER "{name}"'
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
        r#"
            PSQL="psql --port={port}"

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

// Maps the service's lifecycle and hardening policy to systemd property names.
#[derive(IntoRusnixValue)]
#[rusnix(rename_all = "PascalCase")]
struct ServiceConfig {
    // Runs the database process as the dedicated Unix account.
    user: &'static str,
    // Shares database files through the dedicated Unix group.
    group: &'static str,

    // Selects notification support from the final PostgreSQL package version.
    r#type: NixValue,
    // Starts the opaque package selected after NixOS merging.
    exec_start: NixValue,
    // Signals the running server to reread its configuration.
    exec_reload: NixValue,
    // Requests PostgreSQL fast shutdown rather than terminating transactions abruptly.
    kill_signal: &'static str,
    // Lets PostgreSQL shut down its children before systemd intervenes.
    kill_mode: &'static str,
    // Bounds the time systemd allows for startup and shutdown.
    timeout_sec: i64,

    // Asks systemd to manage the PostgreSQL directory under /run.
    runtime_directory: &'static str,
    // Gives the service its own temporary directories.
    private_tmp: bool,
    // Hides users’ home directories from the service.
    protect_home: bool,
    // Makes system paths read-only except for explicitly permitted locations.
    protect_system: &'static str,
    // Chooses file permissions according to package support for group access.
    #[rusnix(rename = "UMask")]
    umask: NixValue,

    // Drops all Linux capabilities; systemd spells the empty set as an empty string.
    capability_bounding_set: Vec<&'static str>,
    // Denies access to devices except those allowed by systemd.
    device_policy: &'static str,
    // Defaults to rejecting writable executable memory when final settings disable JIT.
    memory_deny_write_execute: NixValue,
    // Prevents the service from gaining privileges through exec.
    no_new_privileges: bool,
    // Prevents changes to the process execution personality.
    lock_personality: bool,
    // Gives the service a restricted private device namespace.
    private_devices: bool,
    // Isolates mount changes from the rest of the system.
    private_mounts: bool,
    // Limits /proc to process information.
    proc_subset: &'static str,
    // Prevents changes to the system clock.
    protect_clock: bool,
    // Makes control-group interfaces read-only.
    protect_control_groups: bool,
    // Prevents changes to the system hostname.
    protect_hostname: bool,
    // Blocks access to kernel logs.
    protect_kernel_logs: bool,
    // Prevents loading or unloading kernel modules.
    protect_kernel_modules: bool,
    // Makes kernel tuning interfaces read-only.
    protect_kernel_tunables: bool,
    // Hides other users’ processes from the service.
    protect_proc: &'static str,
    // Removes IPC objects owned by the service account when it stops.
    #[rusnix(rename = "RemoveIPC")]
    remove_ipc: bool,

    // Allows only the socket families used by PostgreSQL and service management.
    restrict_address_families: Vec<&'static str>,
    // Prevents creating additional namespaces.
    restrict_namespaces: bool,
    // Prevents requesting real-time scheduling.
    restrict_realtime: bool,
    // Prevents creating files with set-user-ID or set-group-ID bits.
    #[rusnix(rename = "RestrictSUIDSGID")]
    restrict_suid_sgid: bool,
    // Accepts only the host’s native syscall ABI.
    system_call_architectures: &'static str,
    // Retains upstream syscall allow/deny groups and their ordering.
    system_call_filter: Vec<&'static str>,
}

// The NixOS service record contains deferred scripts and merged systemd properties.
#[derive(IntoRusnixValue)]
struct ServiceDefinition {
    // Names the service in systemd status output.
    description: &'static str,
    // Starts the service with the normal multi-user target.
    wanted_by: Vec<&'static str>,
    // Preserves upstream ordering after network setup.
    after: Vec<&'static str>,
    // Passes the final data directory as PGDATA without reading it in Rust.
    environment: NixValue,
    // Makes the final PostgreSQL package and extensions available to generated scripts.
    path: Vec<NixValue>,
    // Initializes the data directory and installs generated configuration links.
    pre_start: NixValue,
    // Waits for readiness, then runs initialization and provisioning SQL.
    post_start: NixValue,
    // Carries the merged systemd properties, including conditional directory management.
    service_config: NixValue,
    // Keeps mount dependencies tied to the final data directory.
    unit_config: NixValue,
}

// Supplies upstream defaults, while settings themselves remain an open record.
#[derive(IntoRusnixValue)]
struct PostgresqlDefaults {
    // Adds generated file locations and defaults to the open PostgreSQL settings map.
    settings: NixValue,
    // Supplies the state-version-dependent package at NixOS default priority.
    package: NixValue,
    // Defaults the data directory from the final package’s database schema.
    data_dir: NixValue,
    // Combines the upstream header and fallback rules using normal NixOS ordering.
    authentication: NixValue,
}

// The operating-system account is separate from PostgreSQL's database roles.
#[derive(IntoRusnixValue)]
struct UnixUser {
    // Declares the operating-system account independently of database roles.
    name: &'static str,
    // Refers to the final NixOS allocation for the PostgreSQL Unix user.
    uid: Expr<i64>,
    // Places the account in the dedicated PostgreSQL Unix group.
    group: &'static str,
    // Labels the account in the generated user database.
    description: &'static str,
    // Uses the final data directory, retaining its Nix string dependency context.
    home: NixValue,
    // Retains the upstream account’s access to the default login shell.
    use_default_shell: bool,
}

// Account names remain keys in open maps; the containing NixOS namespaces are fixed.
#[derive(IntoRusnixValue)]
struct UserDefinitions {
    // Maps account names to Unix user definitions without closing the NixOS namespace.
    users: NixValue,
    // Maps group names to definitions whose IDs are resolved by NixOS.
    groups: NixValue,
}

// Exposes PostgreSQL's package and shared files through the normal NixOS environment.
#[derive(IntoRusnixValue)]
struct Environment {
    // Adds the effective PostgreSQL package, including selected extensions.
    system_packages: Vec<NixValue>,
    // Exposes PostgreSQL’s shared files in the system environment.
    paths_to_link: Vec<&'static str>,
}

// These fixed records have no fallible flattening. Keep them atomic when applying
// NixOS wrappers instead of turning their fields into separate option bindings.
#[track_caller]
fn opaque(value: impl IntoRusnixValue) -> NixValue {
    value
        .into_value()
        .into_nix_value()
        .expect("fixed compatibility record has no structural flattening errors")
}

fn assertions() -> NixValue {
    let pg = options::root().services.postgresql;

    // Rust's owned form guarantees matching names, but ordinary Nix contributors
    // can still violate the invariant; retain the upstream assertion for those inputs.
    Nixpkgs::new().function("map").apply([
        NixValue::function(|user| {
            let name = user.clone().select("name");

            nixos::assertion(
                NixValue::if_else(
                    user.select("ensureDBOwnership"),
                    Nixpkgs::new()
                        .function("elem")
                        .apply([name.clone(), pg.ensure_databases()]),
                    true,
                ),
                nix_text!(
                    r#"
                        For each database user defined with `services.postgresql.ensureUsers` and
                        `ensureDBOwnership = true;`, a database with the same name must be defined
                        in `services.postgresql.ensureDatabases`.

                        Offender: {name} has not been found among databases.
                    "#,
                    name = name,
                ),
            )
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

    let properties = opaque(ServiceConfig {
        user: "postgres",
        group: "postgres",

        r#type: NixValue::if_else(
            Nixpkgs::new()
                .function("versionAtLeast")
                .apply([pg.package().select("version"), "9.6".into()]),
            "notify",
            "simple",
        ),
        exec_start: nix_text!("{package}/bin/postgres", package = package),
        exec_reload: nix_text!(
            "{coreutils}/bin/kill -HUP $MAINPID",
            coreutils = Nixpkgs::from_module().get("coreutils"),
        ),
        kill_signal: "SIGINT",
        kill_mode: "mixed",
        timeout_sec: 120,

        runtime_directory: "postgresql",
        private_tmp: true,
        protect_home: true,
        protect_system: "strict",
        umask: NixValue::if_else(group_access.clone(), "0027", "0077"),

        capability_bounding_set: vec![""],
        device_policy: "closed",
        memory_deny_write_execute: NixValue::from(pg.settings.jit())
            .equals("off")
            .priority(DefinitionPriority::Default),
        no_new_privileges: true,
        lock_personality: true,
        private_devices: true,
        private_mounts: true,
        proc_subset: "pid",
        protect_clock: true,
        protect_control_groups: true,
        protect_hostname: true,
        protect_kernel_logs: true,
        protect_kernel_modules: true,
        protect_kernel_tunables: true,
        protect_proc: "invisible",
        remove_ipc: true,

        restrict_address_families: vec!["AF_INET", "AF_INET6", "AF_NETLINK", "AF_UNIX"],
        restrict_namespaces: true,
        restrict_realtime: true,
        restrict_suid_sgid: true,
        system_call_architectures: "native",
        system_call_filter: vec!["@system-service", "~@privileged @resources"],
    });

    // Data paths matching the package-schema default get StateDirectory management. Other
    // directories retain upstream ReadWritePaths behavior and ownership responsibility.
    nixos::merge([
        properties,
        record! { "ReadWritePaths": NixValue::list([data.clone()]) }.when(NixValue::if_else(
            data.clone().equals("/var/lib/postgresql"),
            false,
            true,
        )),
        NixValue::record([
            (
                "StateDirectory",
                nix_text!(
                    "postgresql postgresql/{schema}",
                    schema = pg.package().select("psqlSchema"),
                ),
            ),
            (
                "StateDirectoryMode",
                NixValue::if_else(group_access, "0750", "0700"),
            ),
        ])
        .when(data.equals(standard_data)),
    ])
}

fn service() -> NixValue {
    let pg = options::root().services.postgresql;

    opaque(ServiceDefinition {
        description: "PostgreSQL Server",
        wanted_by: vec!["multi-user.target"],
        after: vec!["network.target"],

        environment: record! { "PGDATA": pg.data_dir() },
        path: vec![effective_package()],

        pre_start: pre_start(),
        post_start: post_start(),
        service_config: service_config(),
        unit_config: record! { "RequiresMountsFor": path_text(pg.data_dir()) },
    })
}

fn settings() -> NixValue {
    let pg = options::root().services.postgresql;

    // PostgreSQL owns this open namespace, including its literal snake_case keys.
    NixValue::record([
        (
            "hba_file",
            Nixpkgs::from_module()
                .package_function("writeText")
                .apply(["pg_hba.conf".into(), pg.authentication().into()])
                .to_text(),
        ),
        (
            "ident_file",
            Nixpkgs::from_module()
                .package_function("writeText")
                .apply(["pg_ident.conf".into(), pg.ident_map().into()])
                .to_text(),
        ),
        ("log_destination", "stderr".into()),
        (
            "listen_addresses",
            NixValue::if_else(pg.enable_tcpip(), "*", "localhost"),
        ),
        (
            "jit",
            NixValue::if_else(pg.enable_jit(), "on", "off").priority(DefinitionPriority::Default),
        ),
    ])
}

fn checks() -> NixValue {
    let pg = options::root().services.postgresql;

    let check = Nixpkgs::from_module()
        .package_function("runCommand")
        .apply([
            "postgresql-configfile-check".into(),
            record! {},
            nix_text!(
                r#"
                    {package}/bin/postgres -D{config_file} -C config_file >/dev/null
                    touch $out
                "#,
                package = pg.package(),
                config_file = configuration_file(),
            ),
        ]);

    let pkgs = Nixpkgs::from_module();
    // Upstream skips executing the configuration-check derivation for cross builds.
    let native = pkgs
        .value("stdenv.hostPlatform")
        .equals(pkgs.value("stdenv.buildPlatform"));
    let enabled = NixValue::if_else(pg.check_config(), native, false);

    Nixpkgs::new().function("optional").apply([enabled, check])
}

#[rusnix::config]
mod config {
    use super::*;

    // Places the author's optional input record at the existing PostgreSQL option path.
    #[rusnix(root)]
    pub(super) struct Inputs {
        services: Services,
    }

    struct Services {
        postgresql: NixValue,
    }

    // A separate contribution deriving service behavior from final merged options.
    #[rusnix(root)]
    pub(super) struct Implementation {
        assertions: NixValue,
        services: Services,
        users: NixValue,
        environment: NixValue,
        system: NixValue,
        systemd: NixValue,
    }

    pub(super) fn inputs(postgresql: NixValue) -> Inputs {
        Inputs {
            services: Services { postgresql },
        }
    }

    pub(super) fn implementation() -> Implementation {
        let pg = options::root().services.postgresql;
        let enabled = pg.enable();
        let guarded = |value: NixValue| value.when(enabled.clone());

        // Normal rules go between the header and fallback rules; mkForce can replace all.
        let authentication = nixos::merge([
            NixValue::from("# Generated file; do not edit!").before(),
            NixValue::from("# default value of services.postgresql.authentication\nlocal all all              peer\nhost  all all 127.0.0.1/32 md5\nhost  all all ::1/128      md5\n").after(),
        ]);

        let postgres_user = opaque(UnixUser {
            name: "postgres",
            uid: options::root().ids.uids.postgres(),
            group: "postgres",
            description: "PostgreSQL server user",
            home: path_text(pg.data_dir()),
            use_default_shell: true,
        });

        Implementation {
            assertions: guarded(assertions()),
            services: Services {
                postgresql: guarded(opaque(PostgresqlDefaults {
                    settings: settings(),
                    package: default_package().priority(DefinitionPriority::Default),
                    data_dir: nix_text!(
                        "/var/lib/postgresql/{schema}",
                        schema = pg.package().select("psqlSchema"),
                    )
                    .priority(DefinitionPriority::Default),
                    authentication,
                })),
            },
            users: guarded(opaque(UserDefinitions {
                users: record! { "postgres": postgres_user },
                groups: record! {
                    "postgres": record! {
                        "gid": options::root().ids.gids.postgres(),
                    },
                },
            })),
            environment: guarded(opaque(Environment {
                system_packages: vec![effective_package()],
                paths_to_link: vec!["/share/postgresql"],
            })),
            system: guarded(record! { "checks": checks() }),
            systemd: guarded(record! { "services": record! { "postgresql": service() } }),
        }
    }
}

/// Supplies configuration-generation definitions without importing the upstream implementation.
/// The equivalence harness separately retains its public option declarations and migration imports.
pub(crate) fn implementation() -> NixosModule {
    NixosModule::empty().add(config::implementation())
}
