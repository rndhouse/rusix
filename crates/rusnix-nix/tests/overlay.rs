//! A genuine overlay modifies upstream curl, using the ordinary nixpkgs fixed point.
#[path = "../../../examples/overlay/authoring.rs"]
mod authoring;

use rusnix_ir::{
    Config, Expr,
    interop::{
        InputRef, Nixpkgs,
        raw::{AsNixValue, NixFunctionExt, NixValue, NixpkgsExt},
    },
};
use rusnix_nix::{DiagnosticKind, Generated, NixSession, compile};
use std::{fs, path::Path};

fn reference() -> InputRef {
    InputRef::local(
        "overlay-reference",
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/overlay-reference.nix"),
    )
}

fn ordinary(overlay: Option<NixValue>) -> NixValue {
    reference().function("reference").call(NixValue::record([
        ("nixpkgs", Nixpkgs::new().value("path")),
        ("overlay", overlay.into()),
    ]))
}

fn inspect(pkgs: NixValue) -> NixValue {
    reference().function("inspect").call(pkgs)
}

fn save(name: &str, generated: &Generated, value: &serde_json::Value) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/overlay-equivalence")
        .join(name);
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("generated.nix"), &generated.source).unwrap();
    fs::write(
        root.join("source-map.json"),
        serde_json::to_vec_pretty(generated).unwrap(),
    )
    .unwrap();
    fs::write(
        root.join("comparison.json"),
        serde_json::to_vec_pretty(value).unwrap(),
    )
    .unwrap();
}

fn evaluate(name: &str, config: Config) -> serde_json::Value {
    let generated = compile(&config).unwrap();
    let value = NixSession::new()
        .unwrap()
        .evaluate_interop(&generated)
        .unwrap_or_else(|error| panic!("{name}: {error:?}"))
        .value;
    save(name, &generated, &value);
    value
}

#[test]
fn ordinary_baseline_and_handwritten_overlay_establish_the_reference() {
    let generated = compile(
        &Config::new()
            .set("baseline", inspect(Nixpkgs::new().as_value()))
            .set("ordinary", inspect(ordinary(None)))
            .set(
                "reference",
                inspect(ordinary(Some(reference().overlay("overlay").into()))),
            ),
    )
    .unwrap();
    let value = NixSession::new()
        .unwrap()
        .evaluate_interop(&generated)
        .unwrap()
        .value;
    save("reference", &generated, &value);
    assert_eq!(value["baseline"], value["ordinary"]);
    let baseline = &value["baseline"];
    let modified = &value["reference"];
    let mut expected_flags = baseline["configureFlags"].as_array().unwrap().clone();
    assert!(!expected_flags.contains(&"--disable-dict".into()));
    expected_flags.push("--disable-dict".into());
    assert_eq!(
        modified["configureFlags"],
        serde_json::json!(expected_flags)
    );
    assert_ne!(baseline["derivationPath"], modified["derivationPath"]);
    assert_ne!(baseline["recipe"], modified["recipe"]);
    assert_eq!(baseline["sourceRecipe"], modified["sourceRecipe"]);
    assert_eq!(baseline["hello"], modified["hello"]);
    assert_eq!(modified["downstream"]["curl"], modified["derivationPath"]);
    assert_ne!(
        baseline["downstream"]["derivationPath"],
        modified["downstream"]["derivationPath"]
    );
}

#[test]
fn rust_overlay_matches_the_handwritten_recipe_and_downstream_dependency_exactly() {
    let value = evaluate(
        "rust",
        Config::new()
            .set("baseline", inspect(ordinary(None)))
            .set(
                "reference",
                inspect(ordinary(Some(reference().overlay("overlay").into()))),
            )
            .set(
                "rustImported",
                inspect(ordinary(Some(authoring::overlay()))),
            )
            .set("rust", inspect(authoring::package_set())),
    );
    assert_eq!(value["rust"], value["reference"]);
    assert_eq!(value["rustImported"], value["reference"]);
    let baseline = &value["baseline"];
    let modified = &value["rust"];
    assert_eq!(modified["downstream"]["curl"], modified["derivationPath"]);
    assert_ne!(
        baseline["downstream"]["recipe"],
        modified["downstream"]["recipe"]
    );

    // The complete recipe changes only its flags and the resulting output identities.
    let mut expected_recipe = baseline["recipe"].as_str().unwrap().to_owned();
    let env_field = |value: &serde_json::Value| {
        format!(
            "(\"configureFlags\",{})",
            serde_json::to_string(&value["configureFlagsEnv"]).unwrap()
        )
    };
    assert!(expected_recipe.contains(&env_field(baseline)));
    expected_recipe = expected_recipe.replace(&env_field(baseline), &env_field(modified));
    for (output, path) in baseline["outputPaths"].as_object().unwrap() {
        expected_recipe = expected_recipe.replace(
            path.as_str().unwrap(),
            modified["outputPaths"][output].as_str().unwrap(),
        );
    }
    assert_eq!(expected_recipe, modified["recipe"].as_str().unwrap());
    assert_eq!(baseline["source"], modified["source"]);
    assert_eq!(baseline["hello"], modified["hello"]);
}

#[test]
fn prev_modifies_the_previous_package_once_per_overlay_without_self_recursion() {
    let twice = authoring::package_set()
        .select("extend")
        .call(authoring::overlay());
    let reference_twice = ordinary(Some(reference().overlay("overlay").into()))
        .select("extend")
        .call(reference().overlay("overlay").as_value());
    let value = evaluate(
        "prev",
        Config::new()
            .set("baseline", inspect(ordinary(None)))
            .set("once", inspect(authoring::package_set()))
            .set("twice", inspect(twice))
            .set("referenceTwice", inspect(reference_twice)),
    );
    assert_eq!(value["twice"], value["referenceTwice"]);
    let mut flags = value["baseline"]["configureFlags"]
        .as_array()
        .unwrap()
        .clone();
    flags.push("--disable-dict".into());
    assert_eq!(value["once"]["configureFlags"], serde_json::json!(flags));
    flags.push("--disable-dict".into());
    assert_eq!(value["twice"]["configureFlags"], serde_json::json!(flags));
    assert_ne!(
        value["once"]["derivationPath"],
        value["twice"]["derivationPath"]
    );
}

#[test]
fn final_sees_a_later_overlay_while_prev_keeps_the_preceding_package() {
    let probe = NixValue::function(|final_pkgs| {
        NixValue::function(|prev_pkgs| {
            NixValue::record([
                ("rusnixFinalCurl", final_pkgs.select("curl.drvPath")),
                ("rusnixPreviousCurl", prev_pkgs.select("curl.drvPath")),
            ])
        })
    });
    let pkgs = Nixpkgs::new()
        .pkgs_function("extend")
        .call(probe)
        .select("extend")
        .call(authoring::overlay());
    let value = evaluate(
        "final",
        Config::new()
            .set("baseline", Nixpkgs::new().value("curl.drvPath"))
            .set("modified", pkgs.clone().select("curl.drvPath"))
            .set("final", pkgs.clone().select("rusnixFinalCurl"))
            .set("prev", pkgs.select("rusnixPreviousCurl")),
    );
    assert_eq!(value["final"], value["modified"]);
    assert_eq!(value["prev"], value["baseline"]);
    assert_ne!(value["final"], value["prev"]);
}

#[test]
fn unrelated_failing_package_set_attributes_remain_lazy() {
    let poison = NixValue::function(|_| {
        NixValue::function(|_| {
            NixValue::record([(
                "rusnixUnusedPackage",
                NixValue::builtin("throw").call("unused overlay attribute evaluated"),
            )])
        })
    });
    let pkgs = Nixpkgs::new()
        .pkgs_function("extend")
        .call(poison)
        .select("extend")
        .call(authoring::overlay());
    let generated = compile(
        &Config::new()
            .set("curl", pkgs.clone().select("curl.drvPath"))
            .set("hello", pkgs.clone().select("hello.drvPath"))
            .set("ordinaryHello", Nixpkgs::new().value("hello.drvPath")),
    )
    .unwrap();
    assert!(!generated.source.contains("deepSeq"));
    assert!(!generated.source.contains("builtins.seq"));
    let session = NixSession::new().unwrap();
    let value = session.evaluate_interop(&generated).unwrap().value;
    save("lazy", &generated, &value);
    assert_eq!(value["hello"], value["ordinaryHello"]);
    let demanded = compile(&Config::new().set("bad", pkgs.select("rusnixUnusedPackage"))).unwrap();
    let error = session.evaluate_interop(&demanded).unwrap_err();
    assert!(error.reason.contains("unused overlay attribute evaluated"));
}

#[test]
fn failure_inside_override_attrs_maps_to_the_rust_operation() {
    let operation_line = std::cell::Cell::new(0);
    let overlay = NixValue::function(|_| {
        NixValue::function(|prev| {
            let curl = prev
                .select("curl")
                .override_attrs(NixValue::function(|old| {
                    operation_line.set(line!() + 1);
                    let bad: NixValue = Expr::int(1).divide(Expr::int(0)).into();
                    NixValue::record([(
                        "configureFlags",
                        NixValue::concat_lists([
                            old.select("configureFlags"),
                            NixValue::list([bad.to_text()]),
                        ]),
                    )])
                }));
            NixValue::record([("curl", curl)])
        })
    });
    let pkgs = Nixpkgs::new().pkgs_function("extend").call(overlay);
    let generated =
        compile(&Config::new().set("flags", pkgs.select("curl.configureFlags"))).unwrap();
    let error = NixSession::new()
        .unwrap()
        .evaluate_interop(&generated)
        .unwrap_err();
    assert_eq!(error.kind, DiagnosticKind::NixEval);
    assert_eq!(error.reason, "division by zero");
    let origin = error.primary.as_ref().unwrap();
    assert_eq!(origin.file, file!());
    assert_eq!(origin.line, operation_line.get());
    assert_eq!(origin.purpose, "integer division");
    assert!(!error.raw_nix.is_empty());
    save(
        "provenance",
        &generated,
        &serde_json::to_value(&error).unwrap(),
    );
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/overlay-equivalence/provenance");
    fs::write(root.join("nix.stderr"), &error.raw_nix).unwrap();
}
