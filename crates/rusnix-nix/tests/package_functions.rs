//! Native package function interfaces reuse scoped callbacks and the isolated evaluator.
use rusnix_ir::{
    Config, Expr,
    interop::{NixValue, Nixpkgs, PackageFunction},
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

#[rusnix_ir::args]
mod arguments {
    use rusnix_ir::interop::NixValue;

    #[rusnix(root)]
    struct Inputs {
        curl: NixValue,
        stdenv: Stdenv,
        perl_packages: PerlPackages,
    }

    struct Stdenv {
        host_platform: Platform,
    }

    struct Platform {
        is_linux: bool,
    }

    struct PerlPackages {
        perl: Perl,
    }

    struct Perl {
        lib_prefix: String,
    }
}

fn generated(value: NixValue) -> rusnix_nix::Generated {
    compile(&Config::new().set("result", value)).unwrap()
}

// Ignore indentation and line wrapping when checking compiler-owned let bindings.
fn argument_record_count(source: &str) -> usize {
    source
        .split_whitespace()
        .collect::<String>()
        .matches("let__rusnix_arg_")
        .count()
}

#[test]
fn structural_argument_views_lower_to_direct_and_nested_lexical_access() {
    let function = NixValue::function_attrs(["curl", "stdenv", "perlPackages"], |value| {
        let args = arguments::from_value(value);
        (
            Vec::<(&str, NixValue)>::new(),
            NixValue::record([
                ("curl", args.curl()),
                ("linux", args.stdenv.host_platform.is_linux().into()),
                ("prefix", args.perl_packages.perl.lib_prefix().into()),
            ]),
        )
    });
    let source = generated(function.clone()).source;
    assert_eq!(argument_record_count(&source), 0);
    assert!(!source.contains("builtins.getAttr"));
    assert!(source.contains(".hostPlatform.isLinux"));
    assert!(source.contains(".perl.libPrefix"));

    assert_eq!(
        evaluate(function.call(NixValue::record([
            ("curl", "opaque curl input".into()),
            (
                "stdenv",
                NixValue::record([("hostPlatform", NixValue::record([("isLinux", true.into())]),)]),
            ),
            (
                "perlPackages",
                NixValue::record([("perl", NixValue::record([("libPrefix", "lib/perl".into())]),)]),
            ),
        ]))),
        serde_json::json!({"curl":"opaque curl input", "linux":true, "prefix":"lib/perl"}),
    );
}

#[test]
fn nested_select_operations_keep_each_origin_without_rebuilding_the_record() {
    let function = NixValue::function_attrs(["stdenv"], |args| {
        let root = args.select("stdenv");
        let host = root.select("hostPlatform");
        let linux = host.select("isLinux");
        (Vec::<(&str, NixValue)>::new(), linux)
    });
    let artifact = generated(function.clone());
    assert_eq!(argument_record_count(&artifact.source), 0);
    assert!(!artifact.source.contains("builtins.getAttr"));
    assert_eq!(
        artifact
            .spans
            .iter()
            .filter(|span| span.origin.purpose.starts_with("opaque Nix selection"))
            .count(),
        3,
    );

    assert_eq!(
        evaluate(function.call(NixValue::record([(
            "stdenv",
            NixValue::record([("hostPlatform", NixValue::record([("isLinux", true.into())]),)]),
        )]))),
        true,
    );
}

#[test]
fn dependent_defaults_use_lexical_bindings_and_observe_caller_overrides() {
    let function = NixValue::function_attrs(
        ["derived", "required", "nested", "platform", "unused"],
        |args| {
            (
                vec![
                    ("derived", args.clone().select("required")),
                    ("nested", args.clone().select("platform.flag")),
                    ("unused", Expr::int(1).divide(Expr::int(0)).into()),
                ],
                NixValue::record([
                    ("derived", args.clone().select("derived")),
                    ("nested", args.select("nested")),
                ]),
            )
        },
    );
    assert_eq!(
        argument_record_count(&generated(function.clone()).source),
        0
    );

    let call = |required, derived: Option<i64>| {
        let mut values = vec![
            ("required", NixValue::from(required)),
            ("platform", NixValue::record([("flag", true.into())])),
        ];
        if let Some(value) = derived {
            values.push(("derived", value.into()));
        }
        function.clone().call(NixValue::record(values))
    };
    assert_eq!(
        evaluate(NixValue::list([
            call(42_i64, None),
            call(7, None),
            call(7, Some(9))
        ])),
        serde_json::json!([
            {"derived":42,"nested":true}, {"derived":7,"nested":true},
            {"derived":9,"nested":true},
        ]),
    );
}

#[test]
fn callpackage_override_and_functionargs_preserve_the_native_interface() {
    let function = NixValue::function_attrs(["required", "derived"], |args| {
        (
            vec![("derived", args.clone().select("required"))],
            NixValue::record([
                ("required", args.clone().select("required")),
                ("derived", args.select("derived")),
            ]),
        )
    });
    let package = Nixpkgs::new().function("callPackageWith").apply([
        NixValue::record([("required", 42_i64.into())]),
        function.clone(),
        NixValue::record([] as [(&str, NixValue); 0]),
    ]);
    let override_required = package
        .clone()
        .select("override")
        .call(NixValue::record([("required", 7_i64.into())]));
    let override_derived = package
        .clone()
        .select("override")
        .call(NixValue::record([("derived", 9_i64.into())]));

    assert_eq!(
        evaluate(NixValue::record([
            ("base", package.select("derived")),
            ("overrideRequired", override_required.select("derived")),
            ("overrideDerived", override_derived.select("derived")),
            (
                "interface",
                Nixpkgs::new().function("functionArgs").call(function)
            ),
        ])),
        serde_json::json!({
            "base":42,"overrideRequired":7,"overrideDerived":9,
            "interface":{"required":false,"derived":true},
        }),
    );
}

#[test]
fn lexical_argument_names_and_literal_nested_segments_are_rendered_safely() {
    let keys = [
        "a.b",
        "some key",
        "2d",
        "if",
        "${trap}",
        "quote\"",
        "line\n",
        "back\\slash",
        "λ",
    ];
    let function =
        NixValue::function_attrs(["pkg-config", "deterministic-host-uname", "odd"], |args| {
            let mut fields = vec![
                ("pkg-config", args.clone().select("pkg-config")),
                (
                    "deterministic-host-uname",
                    args.clone().select("deterministic-host-uname"),
                ),
            ];
            fields.extend(keys.map(|key| (key, args.clone().select_segments(["odd", key]))));
            (Vec::<(&str, NixValue)>::new(), NixValue::record(fields))
        });
    let source = generated(function.clone()).source;
    assert_eq!(argument_record_count(&source), 0);
    assert!(!source.contains("builtins.getAttr"));
    assert!(source.contains(".\"a.b\""));
    assert!(source.contains(".\"if\""));
    assert!(source.contains(".\"\\${trap}\""));

    let expected = serde_json::Value::Object(
        keys.into_iter()
            .map(|key| (key.into(), 42.into()))
            .chain([
                ("pkg-config".into(), "pkg-config input".into()),
                ("deterministic-host-uname".into(), "uname input".into()),
            ])
            .collect(),
    );
    assert_eq!(
        evaluate(function.call(NixValue::record([
            ("pkg-config", "pkg-config input".into()),
            ("deterministic-host-uname", "uname input".into()),
            (
                "odd",
                NixValue::record(keys.map(|key| (key, 42_i64.into())))
            ),
        ]))),
        expected,
    );
}

#[test]
fn nested_callbacks_capture_outer_arguments_and_keep_their_own_parameters() {
    let function = NixValue::function_attrs(["value"], |outer| {
        let callback = NixValue::function(|first| {
            NixValue::function(|second| {
                NixValue::list([outer.select("value"), first.select("value"), second])
            })
        });
        (
            Vec::<(&str, NixValue)>::new(),
            callback
                .call(NixValue::record([("value", 7_i64.into())]))
                .call(9_i64),
        )
    });
    assert_eq!(
        argument_record_count(&generated(function.clone()).source),
        0
    );
    assert_eq!(
        evaluate(function.call(NixValue::record([("value", 42_i64.into())]))),
        serde_json::json!([42, 7, 9]),
    );
}

#[test]
fn shadowed_outer_arguments_use_one_lazy_capture_for_defaults_and_callbacks() {
    let function = NixValue::function_attrs(["same", "unused"], |outer| {
        let inner = NixValue::function_attrs(["same", "derived"], |inner| {
            (
                vec![
                    ("same", outer.clone().select("same")),
                    ("derived", inner.clone().select("same")),
                ],
                NixValue::function(|callback| {
                    NixValue::list([
                        outer.clone().select("same"),
                        inner.clone().select("same"),
                        inner.select("derived"),
                        callback,
                    ])
                }),
            )
        });
        (
            vec![
                ("same", 42_i64.into()),
                ("unused", Expr::int(1).divide(Expr::int(0)).into()),
            ],
            NixValue::list([
                inner
                    .clone()
                    .call(NixValue::record([("same", 7_i64.into())]))
                    .call("explicit"),
                inner
                    .call(NixValue::record([] as [(&str, NixValue); 0]))
                    .call("default"),
            ]),
        )
    });
    assert_eq!(
        argument_record_count(&generated(function.clone()).source),
        1
    );
    assert_eq!(
        evaluate(function.call(NixValue::record([] as [(&str, NixValue); 0]))),
        serde_json::json!([[42, 7, 7, "explicit"], [42, 42, 42, "default"]]),
    );
}

#[test]
fn whole_argument_record_access_retains_resolved_defaults() {
    let function = NixValue::function_attrs(["required", "derived"], |args| {
        (vec![("derived", args.clone().select("required"))], args)
    });
    assert_eq!(
        argument_record_count(&generated(function.clone()).source),
        1
    );
    assert_eq!(
        evaluate(function.call(NixValue::record([("required", 42_i64.into())]))),
        serde_json::json!({"required":42,"derived":42}),
    );
}

#[test]
fn missing_argument_fields_remain_lazy_runtime_errors_with_accessor_provenance() {
    let mut accessor_line = 0;
    let function = NixValue::function_attrs(["curl", "stdenv", "perlPackages"], |value| {
        let args = arguments::from_value(value);
        accessor_line = line!() + 1;
        let linux = args.stdenv.host_platform.is_linux();
        (Vec::<(&str, NixValue)>::new(), linux.into())
    });
    let artifact = generated(function.call(NixValue::record([
        ("curl", "unused".into()),
        ("stdenv", NixValue::record([] as [(&str, NixValue); 0])),
        ("perlPackages", "unused".into()),
    ])));
    let diagnostic = NixSession::new()
        .unwrap()
        .evaluate_interop(&artifact)
        .unwrap_err();
    assert_eq!(diagnostic.primary.as_ref().unwrap().line, accessor_line);
    assert_eq!(diagnostic.primary.as_ref().unwrap().file, file!());
    assert!(diagnostic.reason.contains("hostPlatform"));
    assert!(!diagnostic.raw_nix.is_empty());

    let mut missing_line = 0;
    let missing = NixValue::function_attrs(["known"], |args| {
        missing_line = line!() + 1;
        let value = args.select("missing");
        (Vec::<(&str, NixValue)>::new(), value)
    });
    assert_eq!(argument_record_count(&generated(missing.clone()).source), 1);
    let failure = generated(missing.call(NixValue::record([("known", true.into())])));
    let diagnostic = NixSession::new()
        .unwrap()
        .evaluate_interop(&failure)
        .unwrap_err();
    assert_eq!(diagnostic.primary.as_ref().unwrap().line, missing_line);
    assert!(diagnostic.reason.contains("missing"));

    // No undefined bare identifier may appear in an unused default or branch.
    let lazy = NixValue::function_attrs(["known", "unused"], |args| {
        (
            vec![("unused", args.clone().select("missing"))],
            NixValue::if_else(false, args.clone().select("missing"), args.select("known")),
        )
    });
    assert_eq!(
        evaluate(lazy.call(NixValue::record([("known", true.into())]))),
        true
    );
}

#[test]
fn selected_argument_references_cannot_escape_their_function_scope() {
    let mut escaped = None;
    let _ = NixValue::function_attrs(["known"], |args| {
        escaped = Some(args.select("known.nested"));
        (Vec::<(&str, NixValue)>::new(), true.into())
    });
    let diagnostic = compile(&Config::new().set("result", escaped.unwrap())).unwrap_err();
    assert!(diagnostic.reason.contains("escaped"));
    assert!(
        diagnostic
            .primary
            .as_ref()
            .unwrap()
            .file
            .ends_with("tests/package_functions.rs")
    );
}

#[test]
fn ordinary_deferred_records_still_use_opaque_attribute_selection() {
    let value = NixValue::record([("known", true.into())]).select("known");
    assert!(generated(value.clone()).source.contains("builtins.getAttr"));
    assert_eq!(evaluate(value), true);
}

#[test]
fn typed_package_function_emits_and_composes_as_an_ordinary_value() {
    let definition_line = line!() + 1;
    let factory = PackageFunction::from_function_attrs(["name", "label"], |args| {
        (
            vec![("label", args.clone().select("name"))],
            args.select("label"),
        )
    });
    let config = Config::new().set("factory", factory.clone());
    let artifact = compile(&config).unwrap();
    assert!(artifact.spans.iter().any(|span| {
        span.origin.line == definition_line && span.origin.purpose == "Nix argument-set function"
    }));
    assert_eq!(
        artifact.source,
        compile(&Config::new().set("factory", factory.as_value()))
            .unwrap()
            .source
    );

    // Emission never evaluates the function; explicit value conversion permits ordinary calls.
    let value: NixValue = factory.clone().into();
    assert_eq!(
        evaluate(value.call(NixValue::record([("name", "ordinary caller".into())]))),
        "ordinary caller"
    );
    assert_eq!(
        evaluate(Nixpkgs::new().function("functionArgs").call(factory)),
        serde_json::json!({"name":false,"label":true})
    );
}

#[test]
fn real_call_package_supplies_dependencies_defaults_and_authoritative_overrides() {
    let factory = PackageFunction::from_function_attrs(["curl", "derived", "unused"], |args| {
        (
            vec![
                ("derived", args.clone().select("curl.pname")),
                ("unused", Expr::int(1).divide(Expr::int(0)).into()),
            ],
            NixValue::record([
                ("name", args.clone().select("curl.pname")),
                ("derived", args.select("derived")),
            ]),
        )
    });
    let pkgs = Nixpkgs::new();
    let base = pkgs.call_package(&factory, NixValue::record([] as [(&str, NixValue); 0]));
    let supplied = pkgs.call_package(
        &factory,
        NixValue::record([("curl", NixValue::record([("pname", "caller curl".into())]))]),
    );
    let explicit_default = pkgs.call_package(
        &factory,
        NixValue::record([("derived", "explicit default".into())]),
    );
    let overridden = base.clone().select("override").call(NixValue::record([(
        "curl",
        NixValue::record([("pname", "overridden curl".into())]),
    )]));
    assert_eq!(
        evaluate(NixValue::record([
            ("base", base.select("derived")),
            ("caller", supplied.select("derived")),
            ("explicit", explicit_default.select("derived")),
            ("override", overridden.select("derived")),
        ])),
        serde_json::json!({
            "base":"curl", "caller":"caller curl", "explicit":"explicit default",
            "override":"overridden curl",
        })
    );
}

#[test]
fn call_package_wires_deferred_package_results_between_factories() {
    let dependency = PackageFunction::from_function_attrs(["lib"], |args| {
        (
            Vec::<(&str, NixValue)>::new(),
            NixValue::record([("version", args.select("lib.version"))]),
        )
    });
    let consumer = PackageFunction::from_function_attrs(["rusnixDependency"], |args| {
        (
            Vec::<(&str, NixValue)>::new(),
            args.select("rusnixDependency.version"),
        )
    });
    let pkgs = Nixpkgs::new();
    let result = pkgs.call_package(
        &consumer,
        NixValue::record([(
            "rusnixDependency",
            pkgs.call_package(&dependency, NixValue::record([] as [(&str, NixValue); 0])),
        )]),
    );
    assert_eq!(evaluate(result), evaluate(pkgs.lib_value("version")));
}

#[test]
fn missing_call_package_dependency_recovers_the_rust_call_boundary() {
    let factory = PackageFunction::from_function_attrs(["rusnixMissingDependency"], |args| {
        (
            Vec::<(&str, NixValue)>::new(),
            args.select("rusnixMissingDependency"),
        )
    });
    let call_line = line!() + 2;
    let result =
        Nixpkgs::new().call_package(&factory, NixValue::record([] as [(&str, NixValue); 0]));
    let error = NixSession::new()
        .unwrap()
        .evaluate_interop(&generated(result))
        .unwrap_err();
    assert_eq!(error.primary.as_ref().unwrap().line, call_line);
    assert_eq!(error.primary.as_ref().unwrap().file, file!());
    assert!(error.reason.contains("rusnixMissingDependency"));
    assert!(!error.raw_nix.is_empty());
}

#[test]
fn typed_package_body_and_caller_override_failures_keep_child_provenance() {
    let mut body_line = 0;
    let factory = PackageFunction::from_function_attrs(["curl"], |args| {
        body_line = line!() + 1;
        let body = args.select("curl.rusnixMissingField");
        (Vec::<(&str, NixValue)>::new(), body)
    });
    let result =
        Nixpkgs::new().call_package(&factory, NixValue::record([] as [(&str, NixValue); 0]));
    let error = NixSession::new()
        .unwrap()
        .evaluate_interop(&generated(result))
        .unwrap_err();
    assert_eq!(error.primary.as_ref().unwrap().line, body_line);
    assert!(error.reason.contains("rusnixMissingField"));
    assert!(!error.raw_nix.is_empty());

    let factory = PackageFunction::from_function_attrs(["curl"], |args| {
        (Vec::<(&str, NixValue)>::new(), args.select("curl"))
    });
    let override_line = line!() + 1;
    let failing = Expr::int(1).divide(Expr::int(0));
    let result =
        Nixpkgs::new().call_package(&factory, NixValue::record([("curl", failing.into())]));
    let error = NixSession::new()
        .unwrap()
        .evaluate_interop(&generated(result))
        .unwrap_err();
    assert_eq!(error.primary.as_ref().unwrap().line, override_line);
    assert!(error.reason.contains("division by zero"));
    assert!(!error.raw_nix.is_empty());
}
