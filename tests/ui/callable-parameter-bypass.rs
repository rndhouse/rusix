use rusnix_ir::{Expr, interop::{NixCallable, NixValue}};

fn main() {
    let function = NixCallable::from_function(|text: Expr<String>| text);
    let _ = function.try_call(NixValue::from(3_i64));
}
