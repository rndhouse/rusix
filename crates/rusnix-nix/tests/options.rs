//! Finite structural views share OptionRef IR, laziness and NixOS composition.
use rusnix_ir::{
    self as rusnix, Config, Expr, ValueKind,
    interop::{InputRef, NixValue},
    nixos::{DefinitionPriority, NixosModule, OptionRef},
};
use rusnix_nix::{DiagnosticKind, NixSession, Provenance, nixos::compile_module};
use std::{fs, path::Path};

#[rusnix::options]
mod options {
    use rusnix_ir::interop::NixValue;
    use std::collections::{BTreeMap, HashMap};

    #[rusnix(root)]
    struct Root {
        services: Services,
    }

    struct Services {
        example: Example,
        second: Example,
        #[rusnix(rename = "literal.node ${key}\"")]
        literal: Flags,
    }

    struct Example {
        enable: bool,
        port: i64,
        data_dir: String,
        payload: NixValue,
        names: Vec<String>,
        optional: Option<String>,
        values: BTreeMap<String, i64>,
        hash_values: HashMap<String, i64>,
        settings: Settings,
        missing: String,
    }

    #[rusnix(value)]
    struct Settings {
        port: i64,
        jit: String,
    }

    #[rusnix(rename_all = "PascalCase")]
    struct Flags {
        #[rusnix(rename = "with space")]
        special_flag: bool,
        other_flag: bool,
    }
}

fn input() -> InputRef {
    InputRef::local(
        "options",
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/options-view.nix"),
    )
}

fn module(value: impl rusnix_ir::ConfigValue) -> NixosModule {
    NixosModule::empty()
        .import_ref(input().module("schema"))
        .add(Config::new().set("environment.result", value))
}

#[test]
fn one_root_generates_expected_scalar_and_opaque_types() {
    let pg = options::root().services.example;
    let _: Expr<bool> = pg.enable();
    let _: Expr<String> = pg.data_dir();
    let _: Expr<i64> = pg.port();
    let _: NixValue = pg.payload();
    let _: NixValue = pg.names();
    let _: NixValue = pg.optional();
    let _: NixValue = pg.values();
    let _: NixValue = pg.hash_values();
    let _: Expr<i64> = pg.settings.port();
    let _: Expr<String> = pg.settings.jit();
    let call_line = line!() + 1;
    let all_settings = pg.settings.as_value();
    let config = Config::new().set("environment.result", all_settings);
    assert_eq!(config.assignments[0].value.origin.line, call_line);
    assert_eq!(config.assignments[0].value.origin.file, file!());
}

#[test]
fn nested_paths_are_bound_to_placement_without_capturing_navigation_origins() {
    let views = options::root();
    for (view, namespace) in [
        (views.services.example, "example"),
        (views.services.second, "second"),
    ] {
        let accessor_line = line!() + 1;
        let reference = view.data_dir();
        let config = Config::new().set("environment.result", reference);
        let node = &config.assignments[0].value;
        assert_eq!(node.origin.file, file!());
        assert_eq!(node.origin.line, accessor_line);
        let ValueKind::OptionReference(path) = &node.kind else {
            panic!("OptionRef IR")
        };
        assert_eq!(path.parts(), &["services", namespace, "dataDir"]);
    }
}

#[test]
fn scalar_collection_and_nullable_values_evaluate_in_nixos() {
    let pg = options::root().services.example;
    let value = rusnix_ir::nix_record! {
        "enable": pg.enable(), "directory": pg.data_dir(), "port": pg.port(),
        "payload": pg.payload(), "names": pg.names(), "optional": pg.optional(),
        "values": pg.values(), "hashValues": pg.hash_values(),
        "settings": pg.settings.as_value(), "jit": pg.settings.jit(),
        "nestedPort": pg.settings.port(),
    };
    let evaluated = NixSession::new()
        .unwrap()
        .evaluate_nixos(
            &compile_module(&module(value)).unwrap(),
            &["environment", "result"],
            false,
        )
        .unwrap()
        .value;
    assert_eq!(
        evaluated,
        serde_json::json!({
            "enable": false, "directory": "/base", "port": 5432,
            "payload": {"dynamic": 42}, "names": ["a", "b"], "optional": null,
            "values": {"x": 1}, "hashValues": {"y": 2},
            "settings": {"port": 1234, "jit": "off", "arbitrary": 0.5},
            "jit": "off", "nestedPort": 1234,
        })
    );
}

#[test]
fn naming_and_literal_segments_are_applied_independently() {
    let flags = options::root().services.literal;
    let value =
        rusnix_ir::nix_record! { "exact": flags.special_flag(), "pascal": flags.other_flag() };
    let artifact = compile_module(&module(value)).unwrap();
    assert_eq!(
        NixSession::new()
            .unwrap()
            .evaluate_nixos(&artifact, &["environment", "result"], false)
            .unwrap()
            .value,
        serde_json::json!({"exact": true, "pascal": false})
    );
}

#[test]
fn direct_segment_constructor_preserves_the_existing_option_reference_ir() {
    let reference =
        OptionRef::<bool>::from_segments(["services", "literal.node ${key}\"", "with space"])
            .into_expr();
    let artifact = compile_module(&module(reference)).unwrap();
    assert_eq!(
        NixSession::new()
            .unwrap()
            .evaluate_nixos(&artifact, &["environment", "result"], false)
            .unwrap()
            .value,
        true
    );
}

#[test]
fn accessor_failure_maps_to_its_call_site_and_retains_the_raw_diagnostic() {
    let pg = options::root().services.example;
    let call_line = line!() + 1;
    let reference = pg.missing();
    let artifact = compile_module(&module(reference)).unwrap();
    let diagnostic = NixSession::new()
        .unwrap()
        .evaluate_nixos(&artifact, &["environment", "result"], false)
        .unwrap_err();
    assert_eq!(diagnostic.primary.as_ref().unwrap().line, call_line);
    assert_eq!(diagnostic.primary.as_ref().unwrap().file, file!());
    assert!(diagnostic.reason.contains("missing"));
    assert_eq!(diagnostic.provenance, Provenance::ErrorContext);
    assert!(!diagnostic.raw_nix.is_empty());
}

#[test]
fn consuming_operation_provenance_remains_more_precise_than_navigation() {
    let port = options::root().services.example.port();
    let division_line = line!() + 1;
    let quotient = Expr::int(44).divide(port);
    let artifact = compile_module(&module(quotient).import_ref(input().module("zero"))).unwrap();
    let diagnostic = NixSession::new()
        .unwrap()
        .evaluate_nixos(&artifact, &["environment", "result"], false)
        .unwrap_err();
    assert_eq!(diagnostic.kind, DiagnosticKind::NixEval);
    assert_eq!(diagnostic.primary.as_ref().unwrap().line, division_line);
    assert_eq!(diagnostic.reason, "division by zero");
}

#[test]
fn same_artifact_follows_ordinary_nix_overrides_and_priorities() {
    let scratch = tempfile::tempdir().unwrap();
    let downstream = scratch.path().join("downstream.nix");
    fs::write(&downstream, "{ module = {}; }").unwrap();
    let command = options::root()
        .services
        .example
        .port()
        .to_text()
        .with_prefix("example --port=");
    let artifact = compile_module(
        &module(command)
            .import_ref(InputRef::local("downstream", &downstream).module("module"))
            .module(
                NixosModule::new(Config::new().set("services.example.port", 5432))
                    .priority(DefinitionPriority::Default),
            ),
    )
    .unwrap();
    let original_source = artifact.module.source.clone();
    for (source, expected) in [
        ("{ module = {}; }", "example --port=5432"),
        (
            "{ module = { services.example.port = 6432; }; }",
            "example --port=6432",
        ),
        (
            "{ module = { lib, ... }: { services.example.port = lib.mkForce 7432; }; }",
            "example --port=7432",
        ),
    ] {
        fs::write(&downstream, source).unwrap();
        assert_eq!(
            NixSession::new()
                .unwrap()
                .evaluate_nixos(&artifact, &["environment", "result"], false)
                .unwrap()
                .value,
            expected
        );
        assert_eq!(artifact.module.source, original_source);
    }
}

#[test]
fn unused_accessors_and_deferred_values_do_not_force_referenced_options() {
    let pg = options::root().services.example;
    let artifact = compile_module(
        &module(rusnix_ir::nix_record! {
            "good": pg.data_dir(), "bad": pg.port(),
        })
        .import_ref(input().module("failing")),
    )
    .unwrap();
    assert!(!artifact.module.source.contains("deepSeq"));
    let session = NixSession::new().unwrap();
    assert_eq!(
        session
            .evaluate_nixos(&artifact, &["environment", "result", "good"], false)
            .unwrap()
            .value,
        "/base"
    );
    assert!(
        session
            .evaluate_nixos(&artifact, &["environment", "result", "bad"], false)
            .unwrap_err()
            .reason
            .contains("unused option view was evaluated")
    );
}
