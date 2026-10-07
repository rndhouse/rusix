use rusix_ir::{self as rusix, Expr};

#[rusix::args]
mod args {
    #[rusix(root)]
    struct Root {
        port: i64,
    }
}

fn needs_boolean(_: Expr<bool>) {}

fn main() {
    needs_boolean(args::from_value(rusix_ir::interop::raw::NixValue::null()).port());
}
