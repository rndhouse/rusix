//! A genuine overlay modifies upstream curl, using the ordinary nixpkgs fixed point.
use rusnix_ir::{
    Config,
    interop::{InputRef, NixValue, Nixpkgs},
};
use rusnix_nix::{Generated, NixSession, compile};
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
        root.join("comparison.json"),
        serde_json::to_vec_pretty(value).unwrap(),
    )
    .unwrap();
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
