use rusnix_ir as rusnix;

#[rusnix::args]
mod args {
    #[rusnix(root)]
    struct Root {
        subtree: Subtree,
    }

    struct Subtree {
        port: i64,
    }
}

fn main() {
    let _ = args::from_value(rusnix_ir::interop::raw::NixValue::null())
        .subtree
        .as_value();
}
