//! Finite typed symbolic dependencies on the final merged NixOS configuration.
//! Accessors build config.* references; they do not declare the public schema or read values into Rust.
use rusnix_ir as rusnix;

#[rusnix::options]
mod references {
    use rusnix_ir::interop::raw::NixValue;

    #[rusnix(root)]
    struct Root {
        services: Services,
        system: System,
        ids: Ids,
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

    // These dependencies supply upstream defaults and the operating-system identities.
    struct System {
        state_version: String,
    }

    struct Ids {
        uids: PostgresIds,
        gids: PostgresIds,
    }

    struct PostgresIds {
        postgres: i64,
    }
}

pub(super) use references::root;
