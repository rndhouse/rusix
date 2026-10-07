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
    let generated = compile(&Config::new().set("result", result)).unwrap();
    let value = NixSession::new()
        .unwrap()
        .evaluate_interop(&generated)
        .unwrap()
        .value;
    assert_eq!(value["result"], "value=shared");
}
