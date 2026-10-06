//! Native Nix operations remain lazy and preserve source locations without library substitution.
use rusnix_ir::{Config, Expr, interop::NixValue};
use rusnix_nix::{NixSession, compile};

fn evaluate(value: NixValue) -> serde_json::Value {
    NixSession::new()
        .unwrap()
        .evaluate_interop(&compile(&Config::new().set("result", value)).unwrap())
        .unwrap()
        .value["result"]
        .clone()
}

#[test]
fn builtin_functions_accept_concrete_and_scoped_symbolic_values() {
    let replace = NixValue::builtin("replaceStrings");
    let factory = NixValue::function_attrs(["text"], |args| {
        (
            vec![("text", "8.11.0".into())],
            replace.apply([
                NixValue::list([".".into()]),
                NixValue::list(["_".into()]),
                args.select("text"),
            ]),
        )
    });

    assert_eq!(
        evaluate(
            factory
                .clone()
                .call(NixValue::record([] as [(&str, NixValue); 0]))
        ),
        "8_11_0"
    );
    assert_eq!(
        evaluate(factory.call(NixValue::record([("text", "overridden.release".into())]))),
        "overridden_release",
    );
    assert_eq!(
        evaluate(NixValue::builtin("lessThan").apply([1_i64.into(), 2_i64.into()])),
        true,
    );
}

#[test]
fn native_assertion_leaves_its_value_lazy_and_retains_its_call_site() {
    let unused: NixValue = Expr::int(1).divide(Expr::int(0)).into();
    let line = line!() + 1;
    let rejected = NixValue::assert(false, unused.clone());
    let generated = compile(&Config::new().set("result", rejected.clone())).unwrap();
    assert!(generated.source.contains("assert false;"));
    let error = NixSession::new()
        .unwrap()
        .evaluate_interop(&generated)
        .unwrap_err();
    assert!(error.reason.contains("assertion"), "{}", error.reason);
    assert_eq!(error.primary.as_ref().unwrap().file, file!());
    assert_eq!(error.primary.as_ref().unwrap().line, line);
    assert!(!error.raw_nix.is_empty());
    assert!(!error.reason.contains("division by zero"));

    assert_eq!(evaluate(NixValue::assert(true, 42_i64)), 42);
    assert_eq!(
        evaluate(NixValue::record([("good", 42_i64.into()), ("unused", rejected),]).select("good")),
        42
    );
    assert_eq!(
        evaluate(
            NixValue::assert(
                true,
                NixValue::record([("good", 42_i64.into()), ("unused", unused),])
            )
            .select("good")
        ),
        42
    );
}

#[test]
fn native_union_is_shallow_right_biased_and_keeps_displaced_values_lazy() {
    let bad: NixValue = Expr::int(1).divide(Expr::int(0)).into();
    let left = NixValue::record([
        ("unchanged", 7_i64.into()),
        ("replaced", bad.clone()),
        ("nested", NixValue::record([("left", true.into())])),
    ]);
    let right = NixValue::record([
        ("replaced", 42_i64.into()),
        ("nested", NixValue::record([("right", true.into())])),
        ("unused", bad),
    ]);
    let merged = left.merge_attrs(right);
    assert_eq!(evaluate(merged.clone().select("replaced")), 42);
    assert_eq!(evaluate(merged.clone().select("unchanged")), 7);
    assert_eq!(
        evaluate(merged.select("nested")),
        serde_json::json!({"right":true})
    );

    let factory =
        NixValue::function(|args| args.merge_attrs(NixValue::record([("version", 1_i64.into())])));
    assert_eq!(
        evaluate(factory.call(NixValue::record([("version", 2_i64.into())]))),
        serde_json::json!({"version":1})
    );
}

#[test]
fn native_operation_failures_preserve_child_origins_and_operation_boundaries() {
    let child_line = line!() + 1;
    let failure: NixValue = Expr::int(1).divide(Expr::int(0)).into();
    for value in [
        NixValue::assert(failure.clone(), 42_i64),
        NixValue::assert(true, failure.clone()),
        failure
            .clone()
            .merge_attrs(NixValue::record([] as [(&str, NixValue); 0])),
        NixValue::builtin("lessThan").apply([failure.clone(), 2_i64.into()]),
    ] {
        let error = NixSession::new()
            .unwrap()
            .evaluate_interop(&compile(&Config::new().set("result", value)).unwrap())
            .unwrap_err();
        assert_eq!(error.reason, "division by zero");
        assert_eq!(error.primary.as_ref().unwrap().line, child_line);
        assert!(!error.raw_nix.is_empty());
    }

    for wrong_left in [false, true] {
        let record = NixValue::record([] as [(&str, NixValue); 0]);
        let (left, right) = if wrong_left {
            (NixValue::from(1_i64), record)
        } else {
            (record, NixValue::from(1_i64))
        };
        let call_line = line!() + 1;
        let invalid = left.merge_attrs(right);
        let error = NixSession::new()
            .unwrap()
            .evaluate_interop(&compile(&Config::new().set("result", invalid)).unwrap())
            .unwrap_err();
        assert_eq!(error.primary.as_ref().unwrap().line, call_line);
        assert!(!error.raw_nix.is_empty());
    }

    let replace = NixValue::builtin("replaceStrings");
    let call_line = line!() + 1;
    let invalid = replace.apply([true.into(), false.into(), 1_i64.into()]);
    let error = NixSession::new()
        .unwrap()
        .evaluate_interop(&compile(&Config::new().set("result", invalid)).unwrap())
        .unwrap_err();
    assert_eq!(error.primary.as_ref().unwrap().line, call_line);
}

#[test]
fn builtin_names_are_literal_path_segments_and_invalid_paths_are_rejected() {
    let line = line!() + 1;
    let missing = NixValue::builtin("not.a.builtin");
    let error = NixSession::new()
        .unwrap()
        .evaluate_interop(&compile(&Config::new().set("result", missing)).unwrap())
        .unwrap_err();
    assert!(error.reason.contains("not.a.builtin"));
    assert_eq!(error.primary.as_ref().unwrap().line, line);
    assert!(!error.raw_nix.is_empty());

    assert!(compile(&Config::new().set("invalid", NixValue::builtin("bad\0name"))).is_err());
}
