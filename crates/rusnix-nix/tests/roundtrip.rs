use rusnix_ir::{Config, Origin, ValueKind};
use rusnix_nix::{
    Diagnostic, DiagnosticKind, Generated, NixSession, Provenance, SourceSpan, compile,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[path = "../../rusnix-cli/src/fixtures.rs"]
mod fixtures;

fn session() -> NixSession {
    NixSession::new().expect("create disposable Nix store")
}

fn failure(name: &str) -> (Generated, Diagnostic, Origin) {
    let config = fixtures::config(name).unwrap();
    let ValueKind::List(items) = &config.assignments[0].value.kind else {
        panic!("list fixture")
    };
    let expected = items[if name == "nested" { 1 } else { 0 }].origin.clone();

    let generated = compile(&config).unwrap();

    let diagnostic = session().evaluate(&generated).unwrap_err();
    assert_eq!(
        diagnostic.kind,
        DiagnosticKind::NixEval,
        "{}",
        diagnostic.raw_nix
    );
    assert_eq!(
        diagnostic.primary,
        Some(expected.clone()),
        "{}",
        diagnostic.raw_nix
    );
    assert_eq!(
        diagnostic.provenance,
        if name == "bad-port" {
            Provenance::ErrorContext
        } else {
            Provenance::SourceMap
        }
    );
    (generated, *diagnostic, expected)
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn snapshot(name: &str, diagnostic: &Diagnostic) {
    let mut diagnostic = diagnostic.clone();
    // Rust preserves the include path spelling. Normalize only repository-relative
    // path spelling for snapshots; keep the actual line/column and Nix reason.
    if let Some(origin) = &mut diagnostic.primary {
        origin.file = workspace()
            .join(&origin.file)
            .canonicalize()
            .unwrap()
            .strip_prefix(workspace())
            .unwrap()
            .to_string_lossy()
            .into();
    }
    let expected = workspace().join(format!("tests/snapshots/{name}.txt"));
    assert_eq!(
        diagnostic.summary(),
        fs::read_to_string(expected).unwrap(),
        "snapshot {name}"
    );
}

#[test]
fn valid_rust_ir_ast_nix_json_roundtrip() {
    let generated = compile(&fixtures::config("good").unwrap()).unwrap();
    assert!(generated.source.contains("# rn-"));
    let evaluated = session().evaluate(&generated).unwrap();
    assert_eq!(
        evaluated.value,
        serde_json::json!({
            "services": { "openssh": { "enable": true, "ports": [22] } },
            "logging": { "level": "verbose" }
        })
    );
    assert_eq!(
        compile(&fixtures::config("good").unwrap()).unwrap().source,
        generated.source
    );
}

#[test]
fn compact_comments_and_persisted_source_spans_use_the_same_origin_ids() {
    let generated = compile(&fixtures::config("good").unwrap()).unwrap();
    let json = serde_json::to_string(&generated).unwrap();
    let restored: Generated = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.source, generated.source);
    assert_eq!(restored.spans.len(), generated.spans.len());

    for (span, original) in restored.spans.iter().zip(&generated.spans) {
        assert_eq!(span.origin, original.origin);
        assert_eq!(span.enclosing, original.enclosing);
        assert_eq!(span.origin.id.len(), 19);
        let comment = format!("# {}\n", span.origin.id);
        assert!(
            restored.source[..span.start]
                .trim_end()
                .ends_with(comment.trim_end())
        );
        assert_eq!(restored.origin(&span.origin.id), Some(&span.origin));

        // Byte positions change when comments shrink; persisted spans must still
        // select the same Rust expression at its generated start position.
        let before = &restored.source[..span.start];
        let line = before.bytes().filter(|byte| *byte == b'\n').count() + 1;
        let column = before.rsplit('\n').next().unwrap().len() + 1;
        assert_eq!(restored.at_position(line, column), Some(&span.origin));
    }
}

#[test]
fn invalid_domain_value_maps_to_rust_constraint() {
    let (_, diagnostic, origin) = failure("bad-port");
    let event: serde_json::Value = serde_json::from_str(
        diagnostic
            .raw_nix
            .lines()
            .find_map(|line| line.strip_prefix("@nix "))
            .unwrap(),
    )
    .unwrap();
    assert!(
        event["trace"]
            .as_array()
            .unwrap()
            .iter()
            .any(|frame| { frame["raw_msg"].as_str() == Some(origin.id.as_str()) })
    );
    snapshot("bad-port", &diagnostic);
    let rendered = diagnostic.render(&workspace());
    assert!(rendered.contains("vec![Expr::int(70000).in_range"));
    assert!(rendered.contains("= option: services.openssh.ports"));
}

#[test]
fn nested_list_failure_maps_to_divide_not_container() {
    let (_, diagnostic, _) = failure("nested");
    assert!(
        diagnostic
            .related
            .iter()
            .any(|o| o.purpose == "set services.openssh.ports")
    );
    snapshot("nested", &diagnostic);
}

#[test]
fn generated_syntax_failure_is_a_compiler_bug_even_with_a_rust_span() {
    let source = "{ broken = ; }\n".to_string();
    let origin = Origin::new("innocent.rs", 10, 3, "set innocent");

    let generated = Generated {
        spans: vec![SourceSpan {
            start: 0,
            end: source.len(),
            origin,
            enclosing: vec![],
            diagnostic_site: false,
        }],
        source,
    };

    let diagnostic = session().evaluate(&generated).unwrap_err();
    assert_eq!(diagnostic.kind, DiagnosticKind::Compiler);
    assert!(diagnostic.primary.is_none());
    assert!(!diagnostic.raw_nix.is_empty());
    snapshot("codegen-bug", &diagnostic);
    let selected = session()
        .evaluate_attribute(&generated, "good")
        .unwrap_err();
    assert_eq!(selected.kind, DiagnosticKind::Compiler);
    assert!(selected.primary.is_none());
}

#[test]
fn backend_static_binding_failure_is_a_compiler_bug() {
    let generated = Generated {
        source: "__rusnix_undefined_variable\n".into(),
        spans: vec![],
    };

    let diagnostic = session().evaluate(&generated).unwrap_err();
    assert_eq!(
        diagnostic.kind,
        DiagnosticKind::Compiler,
        "{}",
        diagnostic.raw_nix
    );
    assert!(diagnostic.primary.is_none());
}

#[test]
fn missing_provenance_is_explicit_and_original_is_retained() {
    let generated = Generated {
        source: "builtins.throw \"backend failure without metadata\"\n".into(),
        spans: vec![],
    };

    let diagnostic = session().evaluate(&generated).unwrap_err();
    assert_eq!(diagnostic.kind, DiagnosticKind::NixEval);
    assert!(diagnostic.primary.is_none());
    snapshot("unmapped", &diagnostic);
}

#[test]
fn ir_validation_precedes_codegen() {
    let diagnostic = compile(&fixtures::config("conflict").unwrap()).unwrap_err();
    assert_eq!(diagnostic.kind, DiagnosticKind::Validation);
    snapshot("conflict", &diagnostic);
}

#[test]
fn structured_and_text_diagnostics_map_the_same_operation() {
    let (generated, diagnostic, origin) = failure("nested");
    let event: serde_json::Value = serde_json::from_str(
        diagnostic
            .raw_nix
            .lines()
            .find_map(|l| l.strip_prefix("@nix "))
            .unwrap(),
    )
    .unwrap();
    let frame = event["trace"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["file"].as_str().is_some())
        .unwrap();
    let reported = frame["file"].as_str().unwrap();
    let file = PathBuf::from(reported.rsplitn(3, ':').last().unwrap());

    // Structured provenance works even if the human-readable format changes.
    let mut structured = event.clone();
    structured["msg"] = "a completely different rendering".into();
    let raw = format!("@nix {structured}");
    let mapped = Diagnostic::from_nix(DiagnosticKind::NixEval, &raw, &generated, &file);
    assert_eq!(mapped.primary, Some(origin.clone()));
    assert_eq!(mapped.reason, "division by zero");

    // Older internal-json emits only `msg`; plain text uses this path as well.
    let text = event["msg"].as_str().unwrap();
    for raw in [
        text.to_owned(),
        format!("@nix {}", serde_json::json!({"action":"msg", "msg":text})),
    ] {
        let mapped = Diagnostic::from_nix(DiagnosticKind::NixEval, &raw, &generated, &file);
        assert_eq!(mapped.primary, Some(origin.clone()));
        assert_eq!(mapped.provenance, Provenance::SourceMap);
    }

    // Remove context trace entries: generated positions remain a useful fallback.
    structured["trace"]
        .as_array_mut()
        .unwrap()
        .retain(|f| !f["raw_msg"].as_str().unwrap_or("").starts_with("rn-"));
    let raw = format!("@nix {structured}");
    let mapped = Diagnostic::from_nix(DiagnosticKind::NixEval, &raw, &generated, &file);
    assert_eq!(mapped.primary, Some(origin));
    assert_eq!(mapped.provenance, Provenance::SourceMap);
}

#[test]
fn shallow_error_context_loses_nested_failure_context() {
    let origin = Origin::new("lazy.rs", 1, 1, "outer attrset");
    let source = format!(
        "builtins.addErrorContext \"{}\" {{ nested = builtins.throw \"lazy child failed\"; }}",
        origin.id
    );

    let generated = Generated {
        source,
        spans: vec![],
    };

    let diagnostic = session().evaluate(&generated).unwrap_err();
    assert_eq!(diagnostic.kind, DiagnosticKind::NixEval);
    assert_eq!(diagnostic.reason, "lazy child failed");
    // Even with a lookup entry for the marker, the outer context is absent.
    let map = Generated {
        source: generated.source.clone(),
        spans: vec![SourceSpan {
            start: 0,
            end: generated.source.len(),
            origin,
            enclosing: vec![],
            diagnostic_site: false,
        }],
    };
    let mapped = Diagnostic::from_nix(
        DiagnosticKind::NixEval,
        &diagnostic.raw_nix,
        &map,
        Path::new("deliberately-no-source-map-fallback"),
    );
    assert_eq!(mapped.provenance, Provenance::Unavailable);
}

#[test]
fn string_and_attribute_escaping_and_integer_edges_survive_nix() {
    let text = "Unicode λ, \"quote\", \\ slash, ${builtins.throw \"injection\"}\n\t\r";
    let config = Config::new()
        .set("a\"${injection}.λ", text)
        .set("numbers", vec![i64::MIN, -22, 0, i64::MAX]);

    let generated = compile(&config).unwrap();
    let value = session().evaluate(&generated).unwrap().value;
    assert_eq!(value["a\"${injection}"]["λ"], text);
    assert_eq!(
        value["numbers"],
        serde_json::json!([i64::MIN, -22, 0, i64::MAX])
    );
}

#[test]
fn real_store_write_is_inside_disposable_root_and_is_cleaned_up() {
    let session = session();
    let root = session.root().to_owned();

    let generated = Generated {
        source: "builtins.toFile \"rusnix-isolation-check\" \"isolated content\"".into(),
        spans: vec![],
    };
    let value = session.evaluate(&generated).unwrap().value;
    let logical = value.as_str().unwrap();
    let relative = logical.strip_prefix('/').unwrap();
    let physical = root.join("store").join(relative);
    assert!(physical.starts_with(&root));
    assert_eq!(fs::read_to_string(&physical).unwrap(), "isolated content");
    assert!(root.join("store/nix/var/nix/db/db.sqlite").exists());
    drop(session);
    assert!(!root.exists());
}

#[test]
fn selecting_good_does_not_demand_bad() {
    let config = fixtures::config("selective").unwrap();

    let generated = compile(&config).unwrap();
    assert!(!generated.source.contains("builtins.deepSeq"));
    assert!(!generated.source.contains("builtins.seq"));
    assert_eq!(
        generated.source.matches("builtins.addErrorContext").count(),
        0
    );
    assert_eq!(
        session()
            .evaluate_attribute(&generated, "good")
            .unwrap()
            .value,
        42
    );
    // The same artifact really contains a failure, not a discarded bad branch.
    assert_eq!(
        session()
            .evaluate_attribute(&generated, "bad")
            .unwrap_err()
            .reason,
        "division by zero"
    );
}

#[test]
fn selecting_bad_maps_to_divide_and_retains_static_path() {
    let config = fixtures::config("selective").unwrap();

    let generated = compile(&config).unwrap();

    let diagnostic = session().evaluate_attribute(&generated, "bad").unwrap_err();
    assert_eq!(diagnostic.kind, DiagnosticKind::NixEval);
    assert_eq!(
        diagnostic.primary,
        Some(config.assignments[1].value.origin.clone())
    );
    assert_eq!(diagnostic.provenance, Provenance::SourceMap);
    assert_eq!(diagnostic.reason, "division by zero");
    assert!(
        diagnostic
            .related
            .iter()
            .any(|origin| origin.purpose == "set bad")
    );
    assert!(diagnostic.render(&workspace()).contains("= option: bad"));
    assert!(!diagnostic.raw_nix.is_empty());
    snapshot("selective-bad", &diagnostic);

    let event: serde_json::Value = serde_json::from_str(
        diagnostic
            .raw_nix
            .lines()
            .find_map(|line| line.strip_prefix("@nix "))
            .unwrap(),
    )
    .unwrap();
    let runtime_ids: Vec<_> = event["trace"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|frame| frame["raw_msg"].as_str().filter(|id| id.starts_with("rn-")))
        .collect();
    // Both the operation and its enclosing assignment come from source spans.
    assert!(runtime_ids.is_empty());
}

#[test]
fn selected_list_child_keeps_operation_origin_without_container_forcing() {
    use rusnix_ir::Expr;

    let config = Config::new().set("good", 42).set(
        "bad",
        vec![Expr::int(22), Expr::int(44).divide(Expr::int(0))],
    );
    let ValueKind::List(items) = &config.assignments[1].value.kind else {
        panic!()
    };

    let generated = compile(&config).unwrap();
    assert!(!generated.source.contains("deepSeq"));
    assert_eq!(
        session()
            .evaluate_attribute(&generated, "good")
            .unwrap()
            .value,
        42
    );

    let diagnostic = session().evaluate_attribute(&generated, "bad").unwrap_err();
    assert_eq!(diagnostic.primary, Some(items[1].origin.clone()));
    assert_eq!(diagnostic.provenance, Provenance::SourceMap);
    assert!(
        diagnostic
            .related
            .iter()
            .any(|origin| origin.purpose == "set bad")
    );
}

#[test]
fn live_source_map_fallback_works_without_any_runtime_context() {
    use rusnix_nix::{
        ast::{Builtin, NixExpr, NixKind},
        render,
    };

    let config = fixtures::config("selective").unwrap();
    let assignment = &config.assignments[1];
    // Fault injection at the AST boundary: deliberately omit the runtime context
    // while keeping operation spans and semantic ancestry. No textual span edits.
    let operation = NixExpr::attributed(
        NixKind::Call(
            Builtin::Div,
            vec![
                NixExpr::plain(NixKind::Int(44)),
                NixExpr::plain(NixKind::Int(0)),
            ],
        ),
        assignment.value.origin.clone(),
    );
    let ast = NixExpr::attributed(
        NixKind::AttrSet(vec![
            (vec!["good".into()], NixExpr::plain(NixKind::Int(42))),
            (
                vec!["bad".into()],
                NixExpr::attributed(
                    NixKind::Group(Box::new(operation)),
                    assignment.origin.clone(),
                ),
            ),
        ]),
        config.origin.clone(),
    );

    let generated = render(&ast);
    assert!(!generated.source.contains("addErrorContext"));
    assert_eq!(
        session()
            .evaluate_attribute(&generated, "good")
            .unwrap()
            .value,
        42
    );

    let diagnostic = session().evaluate_attribute(&generated, "bad").unwrap_err();
    assert_eq!(diagnostic.kind, DiagnosticKind::NixEval);
    assert_eq!(diagnostic.reason, "division by zero");
    assert_eq!(diagnostic.primary, Some(assignment.value.origin.clone()));
    assert_eq!(diagnostic.provenance, Provenance::SourceMap);
    assert!(
        diagnostic
            .related
            .iter()
            .any(|origin| origin.purpose == "set bad")
    );
}

#[test]
fn reused_operation_origin_recovers_the_selected_occurrence_path() {
    use rusnix_ir::Expr;

    let operation = Expr::int(44).divide(Expr::int(0));
    let config = Config::new()
        .set("first", operation.clone())
        .set("second", operation);

    let generated = compile(&config).unwrap();
    assert_eq!(
        config.assignments[0].value.origin.id,
        config.assignments[1].value.origin.id
    );

    let diagnostic = session()
        .evaluate_attribute(&generated, "second")
        .unwrap_err();
    assert_eq!(
        diagnostic.primary,
        Some(config.assignments[1].value.origin.clone())
    );
    assert!(
        diagnostic
            .related
            .iter()
            .any(|origin| origin.purpose == "set second")
    );
    assert!(
        !diagnostic
            .related
            .iter()
            .any(|origin| origin.purpose == "set first")
    );
}

#[test]
fn attribute_selection_is_literal_data_not_source_or_cli_flags() {
    let attribute = "--store /nix/store ${throw \"injection\"}";
    let config = Config::new().set(attribute, 42);

    let generated = compile(&config).unwrap();
    assert_eq!(
        session()
            .evaluate_attribute(&generated, attribute)
            .unwrap()
            .value,
        42
    );
}

#[allow(dead_code)]
#[path = "../examples/ssh.rs"]
mod legacy_ssh;

#[test]
fn legacy_escape_hatch_example_evaluates_with_its_range_constraint() {
    let generated = compile(&legacy_ssh::config()).unwrap();
    assert_eq!(
        session().evaluate(&generated).unwrap().value,
        serde_json::json!({
            "services": { "openssh": { "enable": true, "ports": [22] } },
            "logging": { "level": "normal" },
        })
    );
}
