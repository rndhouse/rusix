//! Typed authoring uses the same IR, lazy evaluator and diagnostic boundaries.
use rusnix_ir::{
    self as rusnix, Config, Expr, IntoRusnixValue,
    interop::{
        NixAttrs, NixCallable, NixExpression, NixList, NixValue, Nixpkgs, Package, PackageFunction,
    },
};
use rusnix_nix::{NixSession, compile};

#[rusnix::args]
mod views {
    use rusnix_ir::interop::{NixAttrs, NixCallable, NixLibrary, NixList, Package, Stdenv};

    #[rusnix(root)]
    struct Inputs {
        package: Package,
        lib: NixLibrary,
        builder: NixCallable<Package>,
        stdenv: Stdenv,
        dependencies: NixList<Package>,
        family: NixAttrs<Package>,
        recipe: Recipe,
    }

    #[rusnix(value)]
    struct Recipe {
        name: String,
        enabled: bool,
    }
}

fn evaluate(value: impl rusnix_ir::ConfigValue) -> serde_json::Value {
    NixSession::new()
        .unwrap()
        .evaluate_interop(&compile(&Config::new().set("result", value)).unwrap())
        .unwrap()
        .value["result"]
        .clone()
}

#[test]
fn typed_bindings_preserve_factory_interfaces_defaults_and_family_results() {
    let factory: PackageFunction<Expr<String>> =
        PackageFunction::from_function_attrs(["name", "unused"], |args| {
            (
                vec![("unused", Expr::int(1).divide(Expr::int(0)).into())],
                args.select("name").into_expr(),
            )
        });
    let family = factory.bind(|factory| {
        NixAttrs::new([(
            "member",
            factory.call(NixValue::record([("name", "bound".into())])),
        )])
    });
    assert_eq!(evaluate(family.get("member")), "bound");

    let factory: PackageFunction<NixAttrs<Expr<String>>> =
        PackageFunction::from_function_attrs(["name"], |args| {
            (
                Vec::<(&str, NixValue)>::new(),
                NixAttrs::new([("member", args.select("name").into_expr())]),
            )
        });
    let family =
        Nixpkgs::new().call_package(&factory, NixValue::record([("name", "family".into())]));
    assert_eq!(evaluate(family.get("member")), "family");
}

#[test]
fn bindings_and_callbacks_keep_custom_finite_views() {
    let recipe = views::Recipe::from_expression(NixValue::record([
        ("name", "kept".into()),
        ("enabled", true.into()),
    ]));
    let result: Expr<String> =
        recipe.bind(|recipe| Expr::choose(recipe.enabled(), recipe.name(), "excluded".into()));
    assert_eq!(evaluate(result), "kept");
    let callable = NixCallable::from_function(|recipe: views::Recipe| recipe.name());
    assert_eq!(
        evaluate(callable.call(NixValue::record([("name", "callback".into())]))),
        "callback"
    );
}

#[test]
fn typed_lists_and_text_leave_excluded_failures_lazy() {
    let lib = Nixpkgs::new().library();
    let bad: Expr<String> = NixValue::builtin("throw").call("excluded").into_expr();
    let list = NixList::concat([
        NixList::new(["first".into()]),
        NixList::optional(&lib, false, bad.clone()),
        NixList::new([bad.clone()]).when(&lib, false),
    ]);
    assert_eq!(evaluate(list), serde_json::json!(["first"]));
    assert_eq!(
        evaluate(Expr::concat(["text".into(), bad.when(&lib, false)])),
        "text"
    );
}

#[derive(IntoRusnixValue)]
struct Arguments {
    name: String,
}

#[derive(IntoRusnixValue)]
struct InvalidArguments {
    #[rusnix(flatten)]
    name: String,
}

#[test]
fn structured_arguments_lower_only_at_calls_and_return_conversion_errors() {
    let callable = NixCallable::from_function(|args: NixAttrs| args.field::<Expr<String>>("name"));
    assert_eq!(
        evaluate(
            callable
                .try_call(Arguments {
                    name: "record".into()
                })
                .unwrap()
        ),
        "record"
    );
    assert!(
        callable
            .try_call(InvalidArguments {
                name: "invalid".into()
            })
            .is_err()
    );
    let factory: PackageFunction<Expr<String>> =
        PackageFunction::from_function_attrs(["name"], |args| {
            (
                Vec::<(&str, NixValue)>::new(),
                args.select("name").into_expr(),
            )
        });
    assert_eq!(
        evaluate(
            Nixpkgs::new()
                .try_call_package(
                    &factory,
                    Arguments {
                        name: "factory".into()
                    }
                )
                .unwrap()
        ),
        "factory"
    );
    assert!(
        Nixpkgs::new()
            .try_call_package(
                &factory,
                InvalidArguments {
                    name: "invalid".into()
                }
            )
            .is_err()
    );
}

#[test]
fn package_overrides_outputs_and_existing_references_preserve_package_type() {
    let package: Package = Nixpkgs::new().get("openssl").into();
    let package = package.override_arguments(NixValue::record([("withZlib", true.into())]));
    let package =
        package.override_attrs(|old| old.merge(NixAttrs::new([("version", "typed-probe".into())])));
    assert_eq!(
        evaluate(package.field::<Expr<String>>("version")),
        "typed-probe"
    );
    assert_eq!(
        evaluate(package.output("dev").field::<Expr<String>>("outputName")),
        "dev"
    );
    let output = Nixpkgs::new()
        .pkgs_function("lib.getDev")
        .returning::<Package>()
        .call(package);
    assert_eq!(evaluate(output.field::<Expr<String>>("outputName")), "dev");
}

#[test]
fn typed_binding_shares_source_and_preserves_child_failure_provenance() {
    let bad: Package =
        Package::from_expression(NixValue::builtin("throw").call("typed child failure"));
    let result = bad.bind(|package| package.field::<Expr<String>>("name"));
    let generated = compile(&Config::new().set("result", result)).unwrap();
    assert_eq!(generated.source.matches("typed child failure").count(), 1);
    let error = NixSession::new()
        .unwrap()
        .evaluate_interop(&generated)
        .unwrap_err();
    assert!(error.reason.contains("typed child failure"));
    assert!(format!("{error:?}").contains("typed_interop.rs"));
}

#[test]
fn typed_argument_accessors_keep_packages_callables_scopes_and_lists() {
    let pkgs = Nixpkgs::new();
    let package: Package = pkgs.get("openssl").into();
    let input = views::from_value(rusnix_ir::nix_record! {
        "package": package.clone(),
        "lib": pkgs.library(),
        "builder": pkgs.pkgs_function("stdenv.mkDerivation").returning::<Package>(),
        "stdenv": rusnix_ir::nix_record! {
            "buildPlatform": rusnix_ir::nix_record! { "system": "native" },
            "hostPlatform": rusnix_ir::nix_record! { "system": "native" },
        },
        "dependencies": NixList::new([package.clone()]),
        "family": NixAttrs::new([("openssl", package)]),
        "recipe": rusnix_ir::nix_record! { "name": "view", "enabled": true },
    });
    let built = input.builder().call(rusnix_ir::nix_record! {
        "name": "typed-view", "buildCommand": "", "version": "probe",
    });
    let result = rusnix_ir::nix_record! {
        "package": input.package().field::<Expr<String>>("version"),
        "family": input.family().get("openssl").field::<Expr<String>>("version"),
        "builder": built.field::<Expr<String>>("version"),
        "equal": input.stdenv().build_host_equal(),
        "length": NixValue::builtin("length").call(input.dependencies()),
        "recipe": input.recipe.name().require(&input.lib(), input.recipe.enabled(), "disabled"),
    };
    assert_eq!(
        evaluate(result),
        serde_json::json!({
            "package": "3.3.2", "family": "3.3.2", "builder": "probe", "equal": true,
            "length": 1, "recipe": "view",
        })
    );
}

#[test]
fn structured_callback_results_lower_inside_the_function_boundary() {
    let function: NixCallable<NixAttrs> = NixCallable::try_from_function(|name: Expr<String>| {
        #[derive(IntoRusnixValue)]
        struct ResultRecord {
            name: Expr<String>,
        }

        ResultRecord { name }
    })
    .unwrap();
    assert_eq!(
        evaluate(function.call("typed").field::<Expr<String>>("name")),
        "typed"
    );
    assert!(
        NixCallable::<NixAttrs>::try_from_function(|_: Expr<String>| {
            InvalidArguments {
                name: "invalid".into(),
            }
        })
        .is_err()
    );
}

#[test]
fn typed_text_keeps_package_output_string_context() {
    let source = Nixpkgs::new().get("openssl").as_value().to_text();
    let text = Expr::<String>::concat(["prefix:".into(), source.clone().into_expr()]);
    let result = NixValue::builtin("getContext")
        .call(source)
        .equals(NixValue::builtin("getContext").call(text));
    assert_eq!(evaluate(result), true);
}
