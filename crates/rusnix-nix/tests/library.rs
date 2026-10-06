//! Generic library helpers preserve the caller's functions and deferred value semantics.
use rusnix_ir::{
    self as rusnix, Config, Expr,
    interop::{InputRef, NixLibrary, NixValue, Nixpkgs},
    nix_text,
};
use rusnix_nix::{NixSession, compile};
use std::path::Path;

#[rusnix::args]
mod args {
    use rusnix_ir::interop::NixValue;

    #[rusnix(root)]
    struct Inputs {
        lib: NixValue,
        enabled: bool,
        number: i64,
        text: String,
    }
}

fn evaluate(value: NixValue) -> serde_json::Value {
    NixSession::new()
        .unwrap()
        .evaluate_interop(&compile(&Config::new().set("result", value)).unwrap())
        .unwrap()
        .value["result"]
        .clone()
}

#[test]
fn standard_helpers_handle_concrete_values_empty_lists_and_false_conditions() {
    let lib = Nixpkgs::new().library();
    let values = NixValue::list([1_i64.into(), 2_i64.into()]);

    assert_eq!(
        evaluate(NixValue::record([
            ("optional", lib.optional(true, 42_i64)),
            ("absent", lib.optional(false, 42_i64)),
            ("optionals", lib.optionals(true, values.clone())),
            ("noOptionals", lib.optionals(false, values.clone())),
            ("text", lib.optional_text(true, "exact\n text")),
            ("noText", lib.optional_text(false, "unused")),
            ("all", lib.all([true, true])),
            ("notAll", lib.all([true, false])),
            ("emptyAll", lib.all([] as [bool; 0])),
            (
                "lists",
                lib.concat_lists([values, NixValue::list([3_i64.into()])])
            ),
            ("emptyLists", lib.concat_lists([])),
            ("not", !NixValue::from(true)),
            ("typedNot", (!Expr::boolean(false)).into()),
            (
                "curried",
                lib.as_value()
                    .clone()
                    .select("concatStringsSep")
                    .apply(["/".into(), NixValue::list(["a".into(), "b".into()])])
            ),
        ])),
        serde_json::json!({
            "optional": [42], "absent": [], "optionals": [1, 2], "noOptionals": [],
            "text": "exact\n text", "noText": "", "all": true, "notAll": false,
            "emptyAll": true, "lists": [1, 2, 3], "emptyLists": [],
            "not": false, "typedNot": true, "curried": "a/b",
        }),
    );
}

#[test]
fn library_helpers_accept_typed_symbolic_argument_values() {
    let factory = NixValue::function_attrs(["lib", "enabled", "number", "text"], |value| {
        let args = args::from_value(value);
        let lib = NixLibrary::from_value(args.lib());
        let _: Expr<bool> = !args.enabled();
        let conjunction = args.enabled().and(args.enabled());
        let items = NixValue::list([args.number().into()]);

        (
            Vec::<(&str, NixValue)>::new(),
            NixValue::record([
                ("optional", lib.optional(args.enabled(), args.number())),
                ("optionals", lib.optionals(args.enabled(), items.clone())),
                ("text", lib.optional_text(args.enabled(), args.text())),
                (
                    "validated",
                    lib.throw_if_not(args.enabled(), "disabled", args.number()),
                ),
                ("all", lib.all([args.enabled(), args.enabled()])),
                ("lists", lib.concat_lists([items.clone(), items])),
                ("not", (!args.enabled()).into()),
                ("and", conjunction.into()),
            ]),
        )
    });

    assert_eq!(
        evaluate(factory.call(NixValue::record([
            ("lib", Nixpkgs::new().value("lib")),
            ("enabled", true.into()),
            ("number", 42_i64.into()),
            ("text", "deferred".into()),
        ]))),
        serde_json::json!({
            "optional": [42], "optionals": [42], "text": "deferred", "validated": 42, "all": true,
            "lists": [42, 42], "not": false, "and": true,
        }),
    );
}

#[test]
fn standard_conditionals_and_all_leave_excluded_or_short_circuited_values_lazy() {
    let lib = Nixpkgs::new().library();
    let failure: NixValue = Expr::int(1).divide(Expr::int(0)).into();

    assert_eq!(
        evaluate(NixValue::record([
            ("optional", lib.optional(false, failure.clone())),
            ("optionals", lib.optionals(false, failure.clone())),
            ("text", lib.optional_text(false, failure.clone())),
            ("not", lib.optional(false, !failure.clone())),
            (
                "typedNot",
                lib.optional(false, !failure.clone().into_expr::<bool>())
            ),
            ("all", lib.all([false.into(), failure.clone()])),
            (
                "lists",
                lib.concat_lists([lib.optional(false, failure.clone())])
            ),
            (
                "headOnly",
                lib.as_value()
                    .clone()
                    .select("length")
                    .apply([lib.optional(true, failure.clone())])
            ),
        ])),
        serde_json::json!({
            "optional": [], "optionals": [], "text": "", "not": [], "typedNot": [], "all": false,
            "lists": [], "headOnly": 1,
        }),
    );

    // concatLists must not force individual elements merely to construct the list.
    assert_eq!(
        evaluate(
            lib.as_value()
                .clone()
                .select("length")
                .apply([lib.concat_lists([NixValue::list([failure])])])
        ),
        1,
    );
}

fn replacement(name: &'static str) -> NixValue {
    NixValue::function(move |condition| {
        NixValue::function(move |value| {
            NixValue::record([
                ("function", name.into()),
                ("condition", condition),
                ("value", value),
            ])
        })
    })
}

#[test]
fn every_library_helper_uses_the_supplied_record_including_overridden_functions() {
    let library = NixValue::record([
        ("optional", replacement("optional")),
        ("optionals", replacement("optionals")),
        ("optionalString", replacement("optionalString")),
        (
            "all",
            NixValue::function(|predicate| {
                NixValue::function(move |conditions| {
                    NixValue::record([
                        ("function", "all".into()),
                        ("identityResult", predicate.call(true)),
                        ("conditions", conditions),
                    ])
                })
            }),
        ),
        (
            "concatLists",
            NixValue::function(|lists| {
                NixValue::record([("function", "concatLists".into()), ("lists", lists)])
            }),
        ),
        (
            "throwIfNot",
            NixValue::function(|condition| {
                NixValue::function(move |message| {
                    NixValue::function(move |value| {
                        NixValue::record([
                            ("condition", condition),
                            ("message", message),
                            ("value", value),
                        ])
                    })
                })
            }),
        ),
        ("custom", replacement("custom")),
    ]);
    let lib = NixLibrary::from_value(library);
    let values = NixValue::list([42_i64.into()]);

    assert_eq!(
        evaluate(NixValue::record([
            ("optional", lib.optional(false, 42_i64)),
            ("optionals", lib.optionals(false, values.clone())),
            ("text", lib.optional_text(false, "caller text")),
            ("all", lib.all([false])),
            ("lists", lib.concat_lists([values])),
            (
                "validated",
                lib.throw_if_not(false, "caller message", 7_i64)
            ),
            (
                "custom",
                lib.as_value()
                    .clone()
                    .select("custom")
                    .apply([true.into(), 7_i64.into()])
            ),
        ])),
        serde_json::json!({
            "optional": {"function":"optional", "condition":false, "value":42},
            "optionals": {"function":"optionals", "condition":false, "value":[42]},
            "text": {"function":"optionalString", "condition":false, "value":"caller text"},
            "all": {"function":"all", "identityResult":true, "conditions":[false]},
            "lists": {"function":"concatLists", "lists":[[42]]},
            "validated": {"condition":false, "message":"caller message", "value":7},
            "custom": {"function":"custom", "condition":true, "value":7},
        }),
    );
}

#[test]
fn helper_failures_capture_the_public_call_site_and_keep_the_nix_trace() {
    let lib = NixLibrary::from_value(NixValue::record([] as [(&str, NixValue); 0]));
    let lookup_line = line!() + 1;
    let missing = lib.as_value().clone().select("missing").call(true);
    let cases = [
        (missing, lookup_line),
        (lib.optional(true, "value"), line!()),
        (lib.optionals(true, NixValue::list([])), line!()),
        (lib.optional_text(true, "text"), line!()),
        (lib.all([true]), line!()),
        (lib.concat_lists([]), line!()),
        (lib.throw_if_not(true, "unused", 42_i64), line!()),
    ];

    for (value, line) in cases {
        let diagnostic = NixSession::new()
            .unwrap()
            .evaluate_interop(&compile(&Config::new().set("result", value)).unwrap())
            .unwrap_err();
        let origin = diagnostic.primary.as_ref().unwrap();
        assert_eq!(origin.line, line);
        assert_eq!(origin.file, file!());
        assert!(diagnostic.reason.contains("missing"));
        assert!(!diagnostic.raw_nix.is_empty());
    }
}

#[test]
fn child_expression_failures_remain_more_precise_than_helper_boundaries() {
    let lib = Nixpkgs::new().library();
    let line = line!() + 1;
    let failure: NixValue = Expr::int(1).divide(Expr::int(0)).into();
    let values = [
        lib.optional(true, failure.clone()),
        lib.optionals(true, failure.clone()),
        lib.optional_text(true, failure.clone()),
        lib.all([failure.clone()]),
        lib.concat_lists([NixValue::list([failure.clone()])]),
        lib.throw_if_not(true, "unused", failure.clone()),
        lib.throw_if_not(failure.clone(), "unused", 42_i64),
        !failure.clone(),
        (!failure.into_expr::<bool>()).into(),
    ];

    for value in values {
        let diagnostic = NixSession::new()
            .unwrap()
            .evaluate_interop(&compile(&Config::new().set("result", value)).unwrap())
            .unwrap_err();
        assert_eq!(diagnostic.primary.as_ref().unwrap().line, line);
        assert_eq!(diagnostic.primary.as_ref().unwrap().file, file!());
        assert_eq!(diagnostic.reason, "division by zero");
        assert!(!diagnostic.raw_nix.is_empty());
    }
}

#[test]
fn throw_if_not_preserves_lazy_branches_and_reports_the_validation_call() {
    let lib = Nixpkgs::new().library();
    let failure: NixValue = Expr::int(1).divide(Expr::int(0)).into();

    // A successful check does not evaluate the failure message.
    assert_eq!(
        evaluate(lib.throw_if_not(true, failure.clone(), 42_i64)),
        42
    );
    let line = line!() + 1;
    let rejected = lib.throw_if_not(false, "feature combination rejected", failure);

    // Constructing a rejected expression does not force it if Nix never uses it.
    assert_eq!(
        evaluate(
            NixValue::record([("good", 42_i64.into()), ("bad", rejected.clone())]).select("good")
        ),
        42,
    );

    let diagnostic = NixSession::new()
        .unwrap()
        .evaluate_interop(&compile(&Config::new().set("result", rejected)).unwrap())
        .unwrap_err();
    assert!(diagnostic.reason.contains("feature combination rejected"));
    assert!(!diagnostic.reason.contains("division by zero"));
    assert_eq!(diagnostic.primary.as_ref().unwrap().file, file!());
    assert_eq!(diagnostic.primary.as_ref().unwrap().line, line);
    assert!(!diagnostic.raw_nix.is_empty());
}

// A test-only reproduction of Git's three-rule composition, not a public helper.
fn ordered_guards(
    lib: &NixLibrary,
    checks: [(NixValue, NixValue); 3],
    value: NixValue,
) -> NixValue {
    checks
        .into_iter()
        .rev()
        .fold(value, |body, (condition, message)| {
            lib.throw_if_not(condition, message, body)
        })
}

#[test]
fn ordered_guards_stop_at_the_first_failure_without_forcing_later_values() {
    let lib = Nixpkgs::new().library();
    let session = NixSession::new().unwrap();
    let unused: NixValue = Expr::int(1).divide(Expr::int(0)).into();

    for rejected in 0..3 {
        let checks = std::array::from_fn(|index| {
            if index < rejected {
                (true.into(), unused.clone())
            } else if index == rejected {
                (false.into(), format!("rule {index} rejected").into())
            } else {
                (unused.clone(), unused.clone())
            }
        });
        let guarded = ordered_guards(&lib, checks, unused.clone());
        let diagnostic = session
            .evaluate_interop(&compile(&Config::new().set("result", guarded.clone())).unwrap())
            .unwrap_err();
        assert!(
            diagnostic
                .reason
                .contains(&format!("rule {rejected} rejected"))
        );
        assert!(!diagnostic.reason.contains("division by zero"));
        assert_eq!(diagnostic.primary.as_ref().unwrap().file, file!());
        assert!(!diagnostic.raw_nix.is_empty());

        assert_eq!(
            evaluate(
                NixValue::record([("good", 42_i64.into()), ("unused", guarded)]).select("good")
            ),
            42,
        );
    }

    // Passing guards return the result, leaving unused messages and fields lazy.
    let result = NixValue::record([("good", 42_i64.into()), ("unused", unused.clone())]);
    let guarded = ordered_guards(
        &lib,
        std::array::from_fn(|_| (true.into(), unused.clone())),
        result,
    );
    assert_eq!(evaluate(guarded.select("good")), 42);
}

#[test]
fn ordered_guards_retain_failing_condition_and_message_origins() {
    let lib = Nixpkgs::new().library();
    let session = NixSession::new().unwrap();

    for failing_message in [false, true] {
        let line = line!() + 1;
        let failure: NixValue = Expr::int(1).divide(Expr::int(0)).into();
        let rule = if failing_message {
            (false.into(), failure.clone())
        } else {
            (failure.clone(), "unused message".into())
        };
        let guarded = ordered_guards(
            &lib,
            [
                (true.into(), failure.clone()),
                rule,
                (false.into(), failure.clone()),
            ],
            failure,
        );
        let diagnostic = session
            .evaluate_interop(&compile(&Config::new().set("result", guarded)).unwrap())
            .unwrap_err();
        assert_eq!(diagnostic.reason, "division by zero");
        assert_eq!(diagnostic.primary.as_ref().unwrap().file, file!());
        assert_eq!(diagnostic.primary.as_ref().unwrap().line, line);
        assert!(!diagnostic.raw_nix.is_empty());
    }
}

#[test]
fn ordered_guards_use_the_supplied_throw_function_at_every_level() {
    let lib = NixLibrary::from_value(NixValue::record([(
        "throwIfNot",
        NixValue::function(|_| {
            NixValue::function(|message| {
                NixValue::function(move |value| NixValue::list([message, value]))
            })
        }),
    )]));
    let guarded = ordered_guards(
        &lib,
        ["first", "second", "third"].map(|message| (false.into(), message.into())),
        42_i64.into(),
    );

    // This supplied function deliberately ignores conditions instead of throwing.
    assert_eq!(
        evaluate(guarded),
        serde_json::json!(["first", ["second", ["third", 42]]]),
    );
}

#[test]
fn symbolic_negation_reports_its_caller_for_invalid_backend_boolean_types() {
    let line = line!() + 1;
    let value = !NixValue::from("not a boolean");
    let diagnostic = NixSession::new()
        .unwrap()
        .evaluate_interop(&compile(&Config::new().set("result", value)).unwrap())
        .unwrap_err();
    assert_eq!(diagnostic.primary.as_ref().unwrap().line, line);
    assert_eq!(diagnostic.primary.as_ref().unwrap().file, file!());
    assert!(diagnostic.reason.to_lowercase().contains("boolean"));
}

#[test]
fn boolean_conjunction_short_circuits_and_reports_operand_and_call_origins() {
    let failure_line = line!() + 1;
    let failure: NixValue = Expr::int(1).divide(Expr::int(0)).into();
    let unused = Expr::boolean(false).and(failure.clone().into_expr());

    assert_eq!(evaluate(unused.into()), false);
    for left in [false, true] {
        for right in [false, true] {
            assert_eq!(
                evaluate(Expr::boolean(left).and(Expr::boolean(right)).into()),
                left && right,
            );
        }
    }

    let failure = Expr::boolean(true).and(failure.into_expr());
    let diagnostic = NixSession::new()
        .unwrap()
        .evaluate_interop(&compile(&Config::new().set("result", failure)).unwrap())
        .unwrap_err();
    assert_eq!(diagnostic.primary.as_ref().unwrap().line, failure_line);
    assert_eq!(diagnostic.reason, "division by zero");

    // A declared bool expectation is not proof of the backend value's type.
    // Even the right operand is checked as a boolean, matching Nix &&.
    for left in [false, true] {
        let right = NixValue::from("not a boolean").into_expr();
        let call_line = line!() + 1;
        let invalid = Expr::boolean(left).and(right);
        let artifact = compile(&Config::new().set("result", invalid)).unwrap();
        let result = NixSession::new().unwrap().evaluate_interop(&artifact);

        if left {
            let diagnostic = result.unwrap_err();
            assert_eq!(diagnostic.primary.as_ref().unwrap().line, call_line);
            assert!(diagnostic.reason.to_lowercase().contains("boolean"));
            assert!(!diagnostic.raw_nix.is_empty());
        } else {
            assert_eq!(result.unwrap().value["result"], false);
        }
    }
}

#[test]
fn optional_text_keeps_exact_bytes_and_store_dependency_context() {
    let lib = Nixpkgs::new().library();
    let dependency = Nixpkgs::new().get("hello");
    let text = nix_text!("prefix {dependency}\n suffix", dependency = dependency);
    let context = InputRef::local(
        "context",
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/structured-interop.nix"),
    )
    .function("getContext");
    let selected = lib.optional_text(Expr::boolean(true), text.clone());
    let excluded = lib.optional_text(Expr::boolean(false), text.clone());

    assert_eq!(
        evaluate(NixValue::record([
            ("bytesEqual", selected.clone().equals(text.clone())),
            (
                "validatedContextEqual",
                context
                    .call(lib.throw_if_not(true, "unused", selected.clone()))
                    .equals(context.call(selected.clone()))
            ),
            (
                "contextsEqual",
                context.call(selected.clone()).equals(context.call(text))
            ),
            (
                "contextNotEmpty",
                !context
                    .call(selected)
                    .equals(NixValue::record([] as [(&str, NixValue); 0]))
            ),
            ("excluded", excluded.clone()),
            ("excludedContext", context.call(excluded)),
        ])),
        serde_json::json!({
            "bytesEqual":true,"validatedContextEqual":true,"contextsEqual":true,"contextNotEmpty":true,
            "excluded":"","excludedContext":{},
        }),
    );
}
