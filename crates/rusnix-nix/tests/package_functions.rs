//! Native package function interfaces reuse scoped callbacks and the isolated evaluator.
use rusnix_ir::{
    Config, Expr,
    interop::{NixValue, Nixpkgs},
};
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
fn native_argument_defaults_remain_lazy_and_support_introspection() {
    let function = NixValue::function_attrs(["required", "derived", "unused"], |args| {
        (
            vec![
                ("derived", args.clone().select("required").to_text()),
                ("unused", Expr::int(1).divide(Expr::int(0)).into()),
            ],
            NixValue::record([
                ("required", args.clone().select("required")),
                ("derived", args.select("derived")),
            ]),
        )
    });
    assert_eq!(
        evaluate(
            function
                .clone()
                .call(NixValue::record([("required", 42_i64.into())]))
        ),
        serde_json::json!({"required":42,"derived":"42"})
    );
    assert_eq!(
        evaluate(
            Nixpkgs::new()
                .function("functionArgs")
                .call(function.clone())
        ),
        serde_json::json!({"required":false,"derived":true,"unused":true})
    );
    for fields in [
        vec![],
        vec![("required", 42_i64.into()), ("extra", true.into())],
    ] {
        let call_line = line!() + 1;
        let value = function.clone().call(NixValue::record(fields));
        let error = NixSession::new()
            .unwrap()
            .evaluate_interop(&compile(&Config::new().set("result", value)).unwrap())
            .unwrap_err();
        assert_eq!(error.primary.unwrap().line, call_line);
        assert!(!error.raw_nix.is_empty());
        assert!(error.reason.contains("argument"));
    }
}

#[test]
fn defaults_reference_other_defaults_and_nested_functions_keep_lexical_capture() {
    let function = NixValue::function_attrs(["same", "derived"], |outer| {
        let inner = NixValue::function_attrs(["same"], |inner| {
            (
                Vec::<(&str, NixValue)>::new(),
                NixValue::list([outer.clone().select("same"), inner.select("same")]),
            )
        });
        (
            vec![("same", 42_i64.into()), ("derived", outer.select("same"))],
            inner.call(NixValue::record([("same", 7_i64.into())])),
        )
    });
    assert_eq!(
        evaluate(function.call(NixValue::record([] as [(&str, NixValue); 0]))),
        serde_json::json!([42, 7])
    );
    let function = NixValue::function_attrs(["first", "second"], |args| {
        (
            vec![
                ("first", true.into()),
                ("second", args.clone().select("first")),
            ],
            args.select("second"),
        )
    });
    assert_eq!(
        evaluate(function.call(NixValue::record([] as [(&str, NixValue); 0]))),
        true
    );
}

#[test]
fn argument_interfaces_reject_invalid_bindings_defaults_and_escaped_parameters() {
    for names in [
        vec!["bad.name"],
        vec!["same", "same"],
        vec!["__rusnix_arg_0"],
        vec!["if"],
    ] {
        let value =
            NixValue::function_attrs(names, |_| (Vec::<(&str, NixValue)>::new(), true.into()));
        assert!(
            compile(&Config::new().set("result", value))
                .unwrap_err()
                .reason
                .contains("argument")
        );
    }
    for defaults in [
        vec![("unknown", true.into())],
        vec![("known", true.into()), ("known", false.into())],
    ] {
        let value = NixValue::function_attrs(["known"], |_| (defaults, true.into()));
        assert!(
            compile(&Config::new().set("result", value))
                .unwrap_err()
                .reason
                .contains("default")
        );
    }
    let mut escaped = None;
    let _ = NixValue::function_attrs(["known"], |args| {
        escaped = Some(args);
        (Vec::<(&str, NixValue)>::new(), true.into())
    });
    assert!(
        compile(&Config::new().set("result", escaped.unwrap()))
            .unwrap_err()
            .reason
            .contains("escaped")
    );
}

#[test]
fn pinned_paths_remain_paths_and_validate_traversal() {
    let path =
        Nixpkgs::new().source_path("pkgs/applications/version-management/git/ssh-path.patch");
    assert_eq!(
        evaluate(Nixpkgs::new().function("isPath").call(path.clone())),
        true
    );
    let text = Nixpkgs::new().function("fileContents").call(path);
    assert!(evaluate(text).as_str().unwrap().contains("ssh"));
    for path in ["../host", "", "/absolute", "a\0b"] {
        assert!(compile(&Config::new().set("result", Nixpkgs::new().source_path(path))).is_err());
    }
}
