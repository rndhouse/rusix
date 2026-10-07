use rusix_ir as rusix;

#[rusix::args]
mod args {
    #[rusix(root)]
    struct Root {
        subtree: Subtree,
    }

    struct Subtree {
        port: i64,
    }
}

fn main() {
    let _ = args::from_value(rusix_ir::interop::raw::NixValue::null())
        .subtree
        .as_value();
}
