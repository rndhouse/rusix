//! Public component conversion/composition exercised through real NixOS merges.
use rusnix_ir::{
    Config, Expr, IntoConfig,
    nixos::{DefinitionPriority, NixosModule},
};
use rusnix_nix::{DiagnosticKind, NixSession, OriginRole, Provenance, nixos::compile_module};

struct AuthorizedUser(String);

struct ListenPorts(Vec<u16>);

// The trait's track_caller applies to these implementations too. Placement is
// defined by the component; Rusnix does not supply a user or port domain model.
impl IntoConfig for AuthorizedUser {
    fn into_config(self) -> Config {
        Config::new().set("services.openssh.authorizedKeysCommandUser", self.0)
    }
}

impl IntoConfig for ListenPorts {
    fn into_config(self) -> Config {
        Config::new().set("services.openssh.ports", self.0)
    }
}

fn base() -> NixosModule {
    NixosModule::empty().import(ExistingModule::OpenSsh)
}

fn user(name: &str) -> AuthorizedUser {
    AuthorizedUser(name.into())
}

fn ports(port: u16) -> ListenPorts {
    ListenPorts(vec![port])
}

#[allow(dead_code)] // Shared fixture helpers include cases unused in this test target.
#[path = "../../../tests/support/nixos.rs"]
mod support;

use support::{ExistingModule, OpenSsh};

#[test]
fn different_typed_components_compose_with_independent_boundaries_and_caller_origins() {
    let user_line = line!() + 1;
    let module = base().add(user("root"));
    let ports_line = line!() + 1;
    let module = module.add(ports(2222));
    assert!(module.config.assignments.is_empty());
    assert_eq!(module.modules.len(), 2);
    for (child, line) in module.modules.iter().zip([user_line, ports_line]) {
        let origin = &child.config.assignments[0].origin;
        assert_eq!(origin.file, file!());
        assert_eq!(origin.line, line); // .add call, not the adapter's .set
        assert_eq!(child.config.origin.line, line);
    }
    let artifact = compile_module(&module).unwrap();
    assert_eq!(artifact.definitions.len(), 2);
    assert!(!artifact.module.source.contains("deepSeq"));
    let session = NixSession::new().unwrap();
    for (option, expected) in [
        ("authorizedKeysCommandUser", serde_json::json!("root")),
        ("ports", serde_json::json!([2222])),
    ] {
        assert_eq!(
            session
                .evaluate_nixos(&artifact, &["services", "openssh", option], false)
                .unwrap()
                .value,
            expected
        );
    }
}

#[test]
fn typed_conflict_reports_both_authoring_calls_via_definition_metadata() {
    let first_line = line!() + 1;
    let module = base().add(user("root"));
    let second_line = line!() + 1;
    let module = module.add(user("nobody"));
    let artifact = compile_module(&module).unwrap();
    let session = NixSession::new().unwrap();
    for child in &module.modules {
        session
            .evaluate_nixos(
                &compile_module(&base().module(child.clone())).unwrap(),
                &["services", "openssh", "authorizedKeysCommandUser"],
                false,
            )
            .unwrap();
    }
    let diagnostic = session
        .evaluate_nixos(
            &artifact,
            &["services", "openssh", "authorizedKeysCommandUser"],
            false,
        )
        .unwrap_err();
    assert_eq!(diagnostic.kind, DiagnosticKind::NixosMerge);
    assert_eq!(
        diagnostic.option_path.as_deref(),
        Some("services.openssh.authorizedKeysCommandUser")
    );
    assert_eq!(diagnostic.origins.len(), 2);
    for line in [first_line, second_line] {
        let origin = diagnostic
            .origins
            .iter()
            .find(|o| o.origin.as_ref().unwrap().line == line)
            .unwrap();
        assert_eq!(origin.origin.as_ref().unwrap().file, file!());
        assert_eq!(origin.role, OriginRole::ConflictingDefinition);
        assert_eq!(origin.provenance, Provenance::ModuleDefinition);
        assert!(diagnostic.raw_nix.contains(&format!(
            "rusnix-definition:{}",
            origin.origin.as_ref().unwrap().id
        )));
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let rendered = diagnostic.render(&root);
    assert_eq!(rendered.matches("  --> ").count(), 2);
    let json = serde_json::to_value(&diagnostic).unwrap();
    assert_eq!(json["origins"].as_array().unwrap().len(), 2);
    // Reviewable actual output, without a brittle whole-message snapshot.
    let out = root.join("target/authoring/conflict");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("module.nix"), &artifact.module.source).unwrap();
    std::fs::write(out.join("diagnostic.txt"), rendered).unwrap();
    std::fs::write(
        out.join("diagnostic.json"),
        serde_json::to_vec_pretty(&json).unwrap(),
    )
    .unwrap();
    std::fs::write(out.join("nix.stderr"), &diagnostic.raw_nix).unwrap();
}

#[test]
fn two_typed_lists_merge_under_nixos_semantics() {
    let module = base().add(ports(22)).add(ports(2222));
    assert_eq!(module.modules.len(), 2);
    let artifact = compile_module(&module).unwrap();
    assert_eq!(artifact.definitions.len(), 2);
    assert_eq!(
        NixSession::new()
            .unwrap()
            .evaluate_nixos(&artifact, &["services", "openssh", "ports"], false)
            .unwrap()
            .value,
        serde_json::json!([2222, 22])
    );
}

#[test]
fn priorities_remain_local_to_contributions_and_discarded_values_stay_lazy() {
    let module = base()
        .module(
            NixosModule::new(user("nobody").into_config()).priority(DefinitionPriority::Default),
        )
        .add(user("root"));
    let session = NixSession::new().unwrap();
    let selection = &["services", "openssh", "authorizedKeysCommandUser"];
    assert_eq!(
        session
            .evaluate_nixos(&compile_module(&module).unwrap(), selection, false)
            .unwrap()
            .value,
        "root"
    );
    let module = module
        .module(NixosModule::new(user("sshd").into_config()).priority(DefinitionPriority::Force))
        .module(
            NixosModule::new(user("chosen").into_config())
                .priority(DefinitionPriority::Override(40)),
        )
        .module(
            NixosModule::new(Config::new().set(
                "services.openssh.authorizedKeysCommandUser",
                Expr::int(44).divide(Expr::int(0)),
            ))
            .priority(DefinitionPriority::Default),
        )
        // Parent priority applies only to its own bindings, never .add children.
        .priority(DefinitionPriority::Force);
    assert!(matches!(
        module.modules[1].priority,
        DefinitionPriority::Normal
    ));
    assert_eq!(
        session
            .evaluate_nixos(&compile_module(&module).unwrap(), selection, false)
            .unwrap()
            .value,
        "chosen"
    );
}

struct CalculatedPorts(Expr<i64>);

impl IntoConfig for CalculatedPorts {
    fn into_config(self) -> Config {
        Config::new().set("services.openssh.ports", vec![self.0])
    }
}

#[test]
fn previously_captured_expression_and_setter_origins_survive_conversion() {
    let expression_line = line!() + 1;
    let expression = Expr::int(44).divide(Expr::int(0));
    let add_line = line!() + 1;
    let module = base().add(CalculatedPorts(expression));
    let diagnostic = NixSession::new()
        .unwrap()
        .evaluate_nixos(
            &compile_module(&module).unwrap(),
            &["services", "openssh", "ports"],
            false,
        )
        .unwrap_err();
    assert_eq!(diagnostic.primary.as_ref().unwrap().line, expression_line);
    assert_eq!(diagnostic.provenance, Provenance::ErrorContext);
    assert_eq!(
        diagnostic.option_path.as_deref(),
        Some("services.openssh.ports")
    );
    assert!(diagnostic.related.iter().any(|o| o.line == add_line));
    let setter_line = line!() + 1;
    let ssh = OpenSsh::new().enable(false);
    let module = base().add(ssh);
    assert_eq!(
        module.modules[0].config.assignments[0].origin.line,
        setter_line
    );
}

#[test]
fn direct_conversion_tracks_its_caller_and_one_config_still_validates_duplicates() {
    let conversion_line = line!() + 1;
    let config = user("root").into_config();
    assert_eq!(config.assignments[0].origin.line, conversion_line);
    let config = config.set("services.openssh.authorizedKeysCommandUser", "nobody");
    let expected = config.assignments[1].origin.clone();
    let diagnostic = compile_module(&base().add(config)).unwrap_err();
    assert_eq!(diagnostic.kind, DiagnosticKind::Validation);
    assert_eq!(diagnostic.primary, Some(expected));
}

#[test]
fn per_value_priorities_leave_selection_to_nixos() {
    use rusnix_ir::interop::NixValue;

    let session = NixSession::new().unwrap();
    for (left, right, expected) in [
        (
            DefinitionPriority::Default,
            DefinitionPriority::Normal,
            "nobody",
        ),
        (
            DefinitionPriority::Normal,
            DefinitionPriority::Force,
            "nobody",
        ),
        (
            DefinitionPriority::Override(10),
            DefinitionPriority::Force,
            "root",
        ),
    ] {
        let module = base()
            .add(Config::new().set(
                "services.openssh.authorizedKeysCommandUser",
                NixValue::from("root").priority(left),
            ))
            .add(Config::new().set(
                "services.openssh.authorizedKeysCommandUser",
                NixValue::from("nobody").priority(right),
            ));
        assert_eq!(
            session
                .evaluate_nixos(
                    &compile_module(&module).unwrap(),
                    &["services", "openssh", "authorizedKeysCommandUser"],
                    false
                )
                .unwrap()
                .value,
            expected
        );
    }
}

#[test]
fn deferred_merge_orders_list_definitions_using_nixos_before_and_after() {
    use rusnix_ir::{interop::NixValue, nixos};

    let definitions = nixos::merge([
        NixValue::list([2222.into()]),
        NixValue::list([3333.into()]).after(),
        NixValue::list([1111.into()]).before(),
    ]);
    let module = base().add(Config::new().set("services.openssh.ports", definitions));
    assert_eq!(
        NixSession::new()
            .unwrap()
            .evaluate_nixos(
                &compile_module(&module).unwrap(),
                &["services", "openssh", "ports"],
                false
            )
            .unwrap()
            .value,
        serde_json::json!([1111, 2222, 3333])
    );
}

#[test]
fn deferred_when_discards_inactive_definitions_without_evaluating_their_values() {
    use rusnix_ir::{interop::NixValue, nixos::OptionRef};

    let discarded = NixValue::list([Expr::int(1).divide(Expr::int(0)).into()])
        .when(OptionRef::<bool>::new("services.openssh.enable").into_expr());
    let module = base().add(Config::new().set("services.openssh.ports", discarded));
    let artifact = compile_module(&module).unwrap();
    assert!(!artifact.module.source.contains("deepSeq"));
    assert_eq!(
        NixSession::new()
            .unwrap()
            .evaluate_nixos(&artifact, &["services", "openssh", "ports"], false)
            .unwrap()
            .value,
        serde_json::json!([22])
    );
}

#[test]
fn active_deferred_definition_keeps_the_failing_operation_origin() {
    use rusnix_ir::interop::NixValue;

    let divide_line = line!() + 1;
    let invalid = Expr::int(1).divide(Expr::int(0));
    let module = base().add(Config::new().set(
        "services.openssh.ports",
        NixValue::list([invalid.into()]).when(true),
    ));
    let diagnostic = NixSession::new()
        .unwrap()
        .evaluate_nixos(
            &compile_module(&module).unwrap(),
            &["services", "openssh", "ports"],
            false,
        )
        .unwrap_err();
    assert_eq!(diagnostic.primary.as_ref().unwrap().file, file!());
    assert_eq!(diagnostic.primary.as_ref().unwrap().line, divide_line);
    assert!(diagnostic.reason.contains("division by zero"));
    assert!(!diagnostic.raw_nix.is_empty());
}
