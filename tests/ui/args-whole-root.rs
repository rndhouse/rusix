use rusnix_ir as rusnix;

#[rusnix::args]
mod args {
    #[rusnix(root)]
    struct Root {
        port: i64,
    }
}

fn main() {
    let _ = args::from_value(rusnix_ir::interop::NixValue::null()).as_value();
}
