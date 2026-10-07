//! Normal authoring needs no raw or backend imports.
use rusnix_ir::prelude::*;
use rusnix_nix::{NixSession, compile};

#[rusnix_ir::config]
mod model {
    use rusnix_ir::prelude::*;

    #[rusnix(root)]
    pub struct Arguments {
        pub text: Expr<String>,
    }
}

#[test]
fn typed_authoring_prelude_keeps_calls_templates_and_guards_lazy() {
    let callable =
        NixCallable::from_function(|text: Expr<String>| nix_text!("value={text}", text = text));
    let text = Expr::<String>::choose(
        true,
        callable.call("shared"),
        nix_text!("{bad}", bad = Expr::int(1).divide(Expr::int(0))),
    )
    .bind(|text| text.asserted(true));
    let identity = Nixpkgs::new().library();
    let text = identity.throw_if_not(true, "unrequested rejection", text);
    let call = NixCallable::from_function(|args: NixAttrs<Expr<String>>| args.get("text"));
    let text = call.bind(|call| call.call(NixAttrs::new([("text", text)])));
    let record = NixAttrs::try_from_record(model::Arguments { text }).unwrap();
    let result: Expr<String> = record.field("text");
    let generated = compile(&Config::new().set_dynamic("result", result)).unwrap();
    let value = NixSession::new()
        .unwrap()
        .evaluate_interop(&generated)
        .unwrap()
        .value;
    assert_eq!(value["result"], "value=shared");
}

#[test]
fn an_explicit_raw_adapter_preserves_a_custom_interface_through_normal_operations() {
    use rusnix_ir::interop::raw::{NixRepresentation, NixValue};

    #[derive(Clone, IntoRusnixValue)]
    struct Label(Expr<String>);

    impl NixRepresentation for Label {
        fn from_expression(value: NixValue) -> Self {
            Self(value.into_expr())
        }

        fn as_expression(&self) -> NixValue {
            self.0.clone().into()
        }
    }

    let label = Label("adapted".into()).bind(|label| label.asserted(true));
    let generated = compile(&Config::new().set_dynamic("result", label.0)).unwrap();
    let value = NixSession::new()
        .unwrap()
        .evaluate_interop(&generated)
        .unwrap()
        .value;
    assert_eq!(value["result"], "adapted");
}

#[rusnix_ir::args]
mod overlay_inputs {
    use rusnix_ir::prelude::*;

    #[rusnix(root)]
    struct Inputs {
        overlay: Overlay,
    }
}

#[test]
fn overlays_keep_their_type_through_views_and_bindings_without_raw_imports() {
    let overlay = Overlay::from_function(|_, prev| {
        let hello: Package = prev.field("hello");
        NixAttrs::new([("rusnixHello", hello.into())])
    });
    let inputs = NixCallable::from_function(|inputs: NixAttrs| {
        let inputs = overlay_inputs::from_value(inputs.into());
        let overlay = inputs.overlay().bind(|overlay| overlay.asserted(true));
        let overlay = Overlay::choose(true, overlay.clone(), overlay.asserted(false));
        let hello: Package = Nixpkgs::new()
            .with_overlay(overlay)
            .get("rusnixHello")
            .into();
        hello.field::<Expr<String>>("pname")
    });
    let generated = compile(&Config::new().set_dynamic(
        "name",
        inputs.call(NixAttrs::new([("overlay", overlay.into())])),
    ))
    .unwrap();
    let value = NixSession::new()
        .unwrap()
        .evaluate_interop(&generated)
        .unwrap()
        .value;
    assert_eq!(value["name"], "hello");
}
