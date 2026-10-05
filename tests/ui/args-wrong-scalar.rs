use rusnix_ir::{self as rusnix, Expr};

#[rusnix::args]
mod args {
    #[rusnix(root)]
    struct Root {
        port: i64,
    }
}

fn needs_boolean(_: Expr<bool>) {}

fn main() {
    needs_boolean(args::from_value(rusnix_ir::interop::NixValue::null()).port());
}
