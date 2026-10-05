//! Inline module sugar uses the same traits, IR, and evaluator as explicit derives.
use rusnix_ir::{
    self as rusnix, Expr, IntoConfig, IntoRusnixValue, ValueKind,
    interop::{InputRef, Nixpkgs},
    nixos::{DefinitionPriority, NixosModule},
};
use rusnix_nix::{DiagnosticKind, NixSession, Provenance, compile, nixos::compile_module};

// A reusable type defined outside the authoring boundary.
#[derive(IntoRusnixValue)]
struct Endpoint<'a> {
    host_name: &'a str,
    listen_port: u16,
}

#[rusnix::config]
mod local {
    use super::Endpoint;
    use rusnix_ir::interop::InputRef;

    #[rusnix(root)]
    pub struct Machine<'a> {
        pub services: Services<'a>,
        #[rusnix(flatten)]
        pub metadata: Metadata,
        #[rusnix(skip)]
        pub input: InputRef,
    }

    pub struct Services<'a> {
        pub example_service: Endpoint<'a>,
    }

    #[rusnix(rename_all = "PascalCase")]
    pub struct Metadata {
        pub display_name: String,
        #[rusnix(rename = "literal.name")]
        pub unusual_name: bool,
    }
}

#[test]
fn local_nesting_reuses_external_types_and_existing_mapping_rules() {
    let model = local::Machine {
        services: local::Services {
            example_service: Endpoint {
                host_name: "local",
                listen_port: 8080,
            },
        },
        metadata: local::Metadata {
            display_name: "demo".into(),
            unusual_name: true,
        },
        input: InputRef::local("unused", "not-read.nix"),
    };
    let _ = &model.input;
    let config = model.into_config();
    let value = NixSession::new()
        .unwrap()
        .evaluate(&compile(&config).unwrap())
        .unwrap()
        .value;
    assert_eq!(
        value,
        serde_json::json!({
            "services": {"exampleService": {"hostName": "local", "listenPort": 8080}},
            "DisplayName": "demo", "literal.name": true,
        })
    );
    // The fine-grained API remains available for the same reusable value.
    #[derive(IntoConfig)]
    struct Explicit<'a> {
        services: ExplicitServices<'a>,
    }

    #[derive(IntoRusnixValue)]
    struct ExplicitServices<'a> {
        example_service: Endpoint<'a>,
    }
    let explicit = Explicit {
        services: ExplicitServices {
            example_service: Endpoint {
                host_name: "local",
                listen_port: 8080,
            },
        },
    }
    .into_config();
    assert_eq!(
        NixSession::new()
            .unwrap()
            .evaluate(&compile(&explicit).unwrap())
            .unwrap()
            .value["services"],
        value["services"]
    );
}

#[rusnix::config]
mod custom {
    use rusnix_ir::{IntoRusnixValue, RusnixValue};

    pub enum Mode {
        Server,
        Client,
    }

    impl IntoRusnixValue for Mode {
        #[track_caller]
        fn into_value(self) -> RusnixValue {
            match self {
                Self::Server => "server",
                Self::Client => "client",
            }
            .into_value()
        }
    }

    // Explicit control must not produce duplicate implementations.
    pub struct Count(pub i64);

    impl IntoRusnixValue for Count {
        #[track_caller]
        fn into_value(self) -> RusnixValue {
            (self.0 + 1).into_value()
        }
    }

    #[rusnix(root)]
    pub struct Machine {
        pub mode: Mode,
        pub workers: Count,
    }

    #[rusnix(root, rename_all = "PascalCase")]
    #[derive(IntoRusnixValue)]
    #[cfg_attr(all(), derive(Debug))]
    pub struct AlreadyValue {
        pub listen_port: u16,
    }

    #[rusnix(root)]
    #[derive(rusnix_ir::IntoConfig)]
    pub struct AlreadyConfig {
        pub enable: bool,
    }

    #[rusnix(root)]
    #[derive(IntoRusnixValue)]
    #[cfg_attr(all(), cfg(any()))]
    struct DisabledRoot {
        unused: bool,
    }

    #[rusnix(root)]
    pub struct ManualRoot {
        pub value: i64,
    }

    impl IntoRusnixValue for ManualRoot {
        #[track_caller]
        fn into_value(self) -> RusnixValue {
            RusnixValue::record([("manualValue", self.value.into_value())])
        }
    }
}

#[test]
fn enums_and_custom_conversions_keep_their_explicit_semantics() {
    for (mode, expected) in [
        (custom::Mode::Server, "server"),
        (custom::Mode::Client, "client"),
    ] {
        let config = custom::Machine {
            mode,
            workers: custom::Count(2),
        }
        .into_config();
        let value = NixSession::new()
            .unwrap()
            .evaluate(&compile(&config).unwrap())
            .unwrap()
            .value;
        assert_eq!(value, serde_json::json!({"mode": expected, "workers": 3}));
    }
}

#[test]
fn explicit_value_config_and_manual_root_conversions_do_not_conflict() {
    for (config, expected) in [
        (
            custom::AlreadyValue { listen_port: 443 }.into_config(),
            serde_json::json!({"ListenPort":443}),
        ),
        (
            custom::AlreadyConfig { enable: true }.into_config(),
            serde_json::json!({"enable":true}),
        ),
        (
            custom::ManualRoot { value: 42 }.into_config(),
            serde_json::json!({"manualValue":42}),
        ),
    ] {
        assert_eq!(
            NixSession::new()
                .unwrap()
                .evaluate(&compile(&config).unwrap())
                .unwrap()
                .value,
            expected
        );
    }
}

#[rusnix::config]
mod ssh_models {
    #[rusnix(root)]
    pub struct Contribution<T> {
        pub services: Services<T>,
    }

    pub struct Services<T> {
        pub openssh: T,
    }

    pub struct Ports {
        pub ports: Vec<u16>,
    }

    pub struct User {
        pub authorized_keys_command_user: String,
    }
}

fn ports(values: Vec<u16>) -> ssh_models::Contribution<ssh_models::Ports> {
    ssh_models::Contribution {
        services: ssh_models::Services {
            openssh: ssh_models::Ports { ports: values },
        },
    }
}

fn user(value: &str) -> ssh_models::Contribution<ssh_models::User> {
    ssh_models::Contribution {
        services: ssh_models::Services {
            openssh: ssh_models::User {
                authorized_keys_command_user: value.into(),
            },
        },
    }
}

#[test]
fn module_components_keep_nixos_merge_and_priority_selection() {
    let module = NixosModule::empty()
        .import("nixos/modules/services/networking/ssh/sshd.nix")
        .add(ports(vec![22]))
        .add(ports(vec![2222]));
    assert_eq!(module.modules.len(), 2);
    let session = NixSession::new().unwrap();
    let selection = &["services", "openssh", "ports"];
    let value = session
        .evaluate_nixos(&compile_module(&module).unwrap(), selection, false)
        .unwrap()
        .value;
    let mut merged: Vec<_> = value
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p.as_i64().unwrap())
        .collect();
    merged.sort();
    assert_eq!(merged, vec![22, 2222]);
    let module = module.module(
        NixosModule::new(ports(vec![3333]).into_config()).priority(DefinitionPriority::Force),
    );
    assert_eq!(
        session
            .evaluate_nixos(&compile_module(&module).unwrap(), selection, false)
            .unwrap()
            .value,
        serde_json::json!([3333])
    );
}

#[test]
fn conflicting_module_components_report_both_authoring_calls() {
    let first = line!() + 1;
    let module = NixosModule::empty().add(user("root"));
    let second = line!() + 1;
    let module = module.add(user("nobody"));
    let module = module.import("nixos/modules/services/networking/ssh/sshd.nix");
    let diagnostic = NixSession::new()
        .unwrap()
        .evaluate_nixos(
            &compile_module(&module).unwrap(),
            &["services", "openssh", "authorizedKeysCommandUser"],
            false,
        )
        .unwrap_err();
    assert_eq!(diagnostic.kind, DiagnosticKind::NixosMerge);
    assert_eq!(
        diagnostic.option_path.as_deref(),
        Some("services.openssh.authorizedKeysCommandUser")
    );
    let lines: Vec<_> = diagnostic
        .origins
        .iter()
        .filter_map(|o| o.origin.as_ref())
        .map(|o| o.line)
        .collect();
    assert!(
        lines.contains(&first) && lines.contains(&second),
        "{diagnostic:?}"
    );
}

#[rusnix::config]
mod expressions {
    use rusnix_ir::Expr;

    #[rusnix(root)]
    pub struct Root {
        pub good: i64,
        pub bad: Vec<Expr<i64>>,
    }
}

#[test]
fn unused_values_stay_lazy_and_operation_origins_survive_the_module() {
    let operation_line = line!() + 1;
    let bad = Expr::int(44).divide(Expr::int(0));
    let generated = compile(
        &expressions::Root {
            good: 42,
            bad: vec![bad],
        }
        .into_config(),
    )
    .unwrap();
    let session = NixSession::new().unwrap();
    assert!(!generated.source.contains("deepSeq"));
    assert_eq!(
        session
            .evaluate_attribute(&generated, "good")
            .unwrap()
            .value,
        42
    );
    let diagnostic = session.evaluate_attribute(&generated, "bad").unwrap_err();
    assert_eq!(diagnostic.primary.as_ref().unwrap().line, operation_line);
    assert_eq!(diagnostic.primary.as_ref().unwrap().file, file!());
    assert!(diagnostic.related.iter().any(|o| o.purpose == "set bad"));
    assert_eq!(diagnostic.provenance, Provenance::ErrorContext);
}

#[rusnix::config]
mod handles {
    use rusnix_ir::{
        Expr,
        interop::{InputRef, ModuleRef, NixFunction, NixValue, OverlayRef, PackageRef},
        nixos::OptionRef,
    };

    #[rusnix(root)]
    pub struct Root {
        pub package: PackageRef,
        pub module: ModuleRef,
        pub overlay: OverlayRef,
        pub function: NixFunction,
        pub value: NixValue,
        pub port: OptionRef<i64>,
        pub command: Expr<String>,
        #[rusnix(skip)]
        pub input: InputRef,
    }
}

#[test]
fn symbolic_and_opaque_values_remain_native_ir_objects() {
    let pkgs = Nixpkgs::new();
    let input = InputRef::local("example", "not-read.nix");
    let package = pkgs.get("hello");
    let origin = package.reference().origin.clone();
    let model = handles::Root {
        package,
        module: pkgs.module("misc/label.nix"),
        overlay: input.overlay("overlays.example"),
        function: pkgs.function("toUpper"),
        value: pkgs.function("toUpper").call("rusnix"),
        port: rusnix::nixos::OptionRef::<i64>::new("services.example.port"),
        command: rusnix::nixos::OptionRef::<i64>::new("services.example.port")
            .into_expr()
            .to_text(),
        input,
    };
    let _ = &model.input;
    let config = model.into_config();
    assert_eq!(config.assignments.len(), 7);
    assert_eq!(config.assignments[0].value.origin, origin);
    assert!(
        config.assignments[..4]
            .iter()
            .all(|a| matches!(a.value.kind, ValueKind::Reference(_)))
    );
    assert!(matches!(
        config.assignments[4].value.kind,
        ValueKind::Apply(..)
    ));
    assert!(matches!(
        config.assignments[5].value.kind,
        ValueKind::OptionReference(_)
    ));
    assert!(matches!(
        config.assignments[6].value.kind,
        ValueKind::ToText(_)
    ));
    compile_module(&NixosModule::empty().add(config)).unwrap();
}

#[rusnix::config]
mod packages {
    use rusnix_ir::interop::PackageRef;

    #[rusnix(root)]
    pub struct Root {
        pub environment: Environment,
    }

    pub struct Environment {
        pub system_packages: Vec<PackageRef>,
    }
}

#[test]
fn opaque_packages_in_automatic_structs_resolve_through_real_nixpkgs() {
    let module = NixosModule::empty()
        .import_ref(Nixpkgs::new().module("config/system-path.nix"))
        .module(
            NixosModule::new(
                packages::Root {
                    environment: packages::Environment {
                        system_packages: vec![Nixpkgs::new().get("hello")],
                    },
                }
                .into_config(),
            )
            .priority(DefinitionPriority::Force),
        );
    let value = NixSession::new()
        .unwrap()
        .evaluate_system_packages(&compile_module(&module).unwrap())
        .unwrap()
        .value;
    assert_eq!(value[0]["pname"], "hello");
    assert_eq!(value[0]["isDerivation"], true);
}
