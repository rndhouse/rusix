#[path = "../../../examples/composed-packages/graph.rs"]
pub mod graph;

#[path = "support/packages.rs"]
mod support;

use rusnix_ir::{
    Config,
    interop::{NixValue, Nixpkgs},
};

fn compare(
    name: &str,
    graph: NixValue,
    fields: impl IntoIterator<Item = (&'static str, NixValue)>,
) -> serde_json::Value {
    let mut fields: Vec<_> = fields.into_iter().collect();
    fields.push(("graph", graph));
    let artifact = support::artifact("composed", fields);
    support::compare("composed", name, artifact)
}

#[test]
fn connected_rust_graph_matches_all_default_exact_recipes() {
    let value = compare("openssl-curl-git", graph::graph(), []);
    assert_eq!(
        value["openssl"]["derivationPath"],
        "/nix/store/mqf1h79k4p5y723yvn0869ni24ifpnpn-openssl-3.3.2.drv"
    );
    assert_eq!(
        value["curl"]["derivationPath"],
        "/nix/store/m9lkswh8vqpcpvj43mlp243jqgd3ka7q-curl-8.11.0.drv"
    );
    assert_eq!(
        value["git"]["derivationPath"],
        "/nix/store/qk30rjnnsz9lxdp9b6k5hbjj7p8w17pg-git-2.47.0.drv"
    );
    assert_eq!(
        value["mariadb"]["derivationPath"],
        "/nix/store/ph4f1jlld7627mn1xqb05d357gw0l663-mariadb-server-10.11.10.drv"
    );
    assert_eq!(
        value["mariadbClient"]["derivationPath"],
        "/nix/store/gd3f8yw68jnsh5m09rkylxa093pqa7s2-mariadb-client-10.11.10.drv"
    );
}

#[test]
fn tagged_factory_value_is_observed_by_curl_and_git() {
    let openssl = Nixpkgs::new().call_package(
        &graph::openssl::factory(graph::openssl::model::Release::Preview),
        graph::arguments(),
    );
    let tagged = openssl
        .select("overrideAttrs")
        .call(NixValue::function(|_| {
            NixValue::record([(
                "passthru",
                NixValue::record([("rusnixAuthor", "Rust OpenSSL".into())]),
            )])
        }));
    let generated = support::artifact(
        "composed",
        [
            (
                "graph",
                graph::compose(
                    tagged,
                    |pkgs, openssl| {
                        pkgs.call_package(
                            &graph::curl::factory(),
                            graph::curl_arguments(pkgs, openssl.clone()),
                        )
                        .select("overrideAttrs")
                        .call(NixValue::function(|old| {
                            NixValue::record([(
                                "passthru",
                                old.select("passthru").merge_attrs(NixValue::record([(
                                    "rusnixAuthor",
                                    "Rust curl".into(),
                                )])),
                            )])
                        }))
                    },
                    graph::mariadb::model::Release::V1011.arguments(),
                ),
            ),
            ("probe", true.into()),
        ],
    );
    let value = support::session()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .evaluate_interop(&generated)
        .unwrap()
        .value;
    assert_eq!(value["result"]["edges"]["curlOpenSSL"], "Rust OpenSSL");
    assert_eq!(value["result"]["edges"]["gitOpenSSL"], "Rust OpenSSL");
    assert_eq!(value["result"]["edges"]["sameOpenSSL"], true);
    assert_eq!(value["result"]["edges"]["mariadbCurl"], "Rust curl");
    assert_eq!(value["result"]["upstream"], value["result"]["candidate"]);
}

#[test]
fn live_openssl_argument_override_propagates_to_both_consumers() {
    let pkgs = Nixpkgs::new();
    let base = compare("base", graph::graph(), []);
    let openssl = pkgs
        .call_package(
            &graph::openssl::factory(graph::openssl::model::Release::Preview),
            graph::arguments(),
        )
        .select("override")
        .call(NixValue::record([("withZlib", true.into())]));
    let changed = compare(
        "openssl-zlib",
        graph::with_openssl(openssl),
        [("opensslArgs", NixValue::record([("withZlib", true.into())]))],
    );
    for name in ["openssl", "curl", "git", "mariadb", "mariadbClient"] {
        assert_ne!(
            base[name]["derivationPath"],
            changed[name]["derivationPath"]
        );
    }
}

#[test]
fn ordinary_openssl_also_composes_and_unused_graph_stays_lazy() {
    compare(
        "ordinary-openssl",
        graph::with_openssl(Nixpkgs::new().value("openssl")),
        [],
    );
    let bad = NixValue::builtin("throw").call("unused OpenSSL");
    let generated = rusnix_nix::compile(
        &Config::new()
            .set("good", true)
            .set("graph", graph::with_openssl(bad)),
    )
    .unwrap();
    let session = support::session().lock().unwrap_or_else(|p| p.into_inner());
    assert_eq!(
        session
            .evaluate_attribute(&generated, "good")
            .unwrap()
            .value,
        true
    );
}

#[test]
fn generated_graph_shares_factories_and_dependencies_lexically() {
    let generated = rusnix_nix::compile(&Config::new().set("graph", graph::graph())).unwrap();
    assert_eq!(generated.source.matches("openssl-3.3.2.tar.gz").count(), 0); // version is lexical interpolation
    assert_eq!(
        generated
            .source
            .matches("sha256-LopAsBl5r+i+C7+z3l3BxnCf7bRtbInBDaEUq1/D0oE=")
            .count(),
        1
    );
    assert_eq!(
        generated
            .source
            .matches("sha256-21nPDWccpuf1wsXsF3CEozp54EyX5xzxg6XN6iNQVOs=")
            .count(),
        1
    );
    assert_eq!(
        generated
            .source
            .matches("sha256-sGp0ZQuDoWqpqwmJhEgrAo51sABnSxH/KIdyxhmm8CI=")
            .count(),
        1
    );
    assert!(!generated.source.contains("deepSeq"));
    assert!(!generated.source.contains("let __rusnix_arg_"));
}

#[test]
fn curl_consuming_invalid_supplied_openssl_keeps_child_boundary() {
    let generated = rusnix_nix::compile(&Config::new().set(
        "result",
        graph::with_openssl(1_i64.into()).select("curl.drvPath"),
    ))
    .unwrap();
    let error = support::session()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .evaluate_interop(&generated)
        .unwrap_err();
    assert!(!error.raw_nix.is_empty());
    assert!(
        error.origins.iter().any(|o| o
            .origin
            .as_ref()
            .is_some_and(|o| o.file.ends_with("curl-nixpkg/lowering.rs"))),
        "{error:?}"
    );
}

#[test]
fn disabled_openssl_is_not_forced_by_composition_or_overrides() {
    let bad = NixValue::builtin("throw").call("excluded supplied OpenSSL");
    let curl = graph::with_openssl(bad)
        .select("curl.override")
        .call(NixValue::record([("opensslSupport", false.into())]));
    let generated =
        rusnix_nix::compile(&Config::new().set("result", curl.select("drvPath"))).unwrap();
    assert!(
        support::session()
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .evaluate_interop(&generated)
            .unwrap()
            .value["result"]
            .as_str()
            .unwrap()
            .ends_with("-curl-8.11.0.drv")
    );
}

#[test]
fn child_openssl_definition_failure_survives_the_full_graph() {
    let openssl = Nixpkgs::new().call_package(
        &graph::openssl::factory(graph::openssl::model::Release::Preview),
        NixValue::record([("fetchurl", 1_i64.into())]),
    );
    let generated = rusnix_nix::compile(&Config::new().set(
        "result",
        graph::with_openssl(openssl).select("mariadb.drvPath"),
    ))
    .unwrap();
    let error = support::session()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .evaluate_interop(&generated)
        .unwrap_err();
    assert!(!error.raw_nix.is_empty());
    assert!(
        error.origins.iter().any(|o| o
            .origin
            .as_ref()
            .is_some_and(|o| o.file.ends_with("openssl-nixpkg/lowering.rs"))),
        "{error:?}"
    );
}

#[test]
fn mariadb_consuming_failing_rust_curl_keeps_child_definition() {
    let graph = graph::compose(
        Nixpkgs::new().value("openssl"),
        |pkgs, openssl| {
            pkgs.call_package(
                &graph::curl::factory(),
                graph::curl_arguments(pkgs, openssl.clone())
                    .merge_attrs(NixValue::record([("fetchurl", 1_i64.into())])),
            )
        },
        graph::mariadb::model::Release::V1011.arguments(),
    );
    let generated =
        rusnix_nix::compile(&Config::new().set("result", graph.select("mariadb.drvPath"))).unwrap();
    let error = failure("mariadb-consuming-curl", &generated);
    assert!(error.reason.contains("call"));
    assert!(
        error.origins.iter().any(|o| o
            .origin
            .as_ref()
            .is_some_and(|o| o.file.ends_with("curl-nixpkg/lowering.rs"))),
        "{:?}",
        error.origins
    );
}

/// Backend-only delayed type validation has no generated child frame to map.
#[test]
fn delayed_stdenv_dependency_validation_retains_reason_and_honest_mapping_limit() {
    let graph = graph::compose(
        Nixpkgs::new().value("openssl"),
        |_, _| 1_i64.into(),
        graph::mariadb::model::Release::V1011.arguments(),
    );
    let generated =
        rusnix_nix::compile(&Config::new().set("result", graph.select("mariadb.drvPath"))).unwrap();
    let error = failure("backend-invalid-dependency", &generated);
    assert!(
        error
            .reason
            .contains("element 6 of buildInputs for mariadb-server")
    );
    assert!(
        error
            .primary
            .as_ref()
            .unwrap()
            .purpose
            .contains("mariadb.drvPath")
    );
}

fn failure(name: &str, generated: &rusnix_nix::Generated) -> rusnix_nix::Diagnostic {
    let error = support::session()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .evaluate_interop(generated)
        .unwrap_err();
    assert!(!error.raw_nix.is_empty());
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../target/composed-equivalence/{name}"));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("generated.nix"), &generated.source).unwrap();
    std::fs::write(
        root.join("error.json"),
        serde_json::to_vec_pretty(&error).unwrap(),
    )
    .unwrap();
    *error
}
