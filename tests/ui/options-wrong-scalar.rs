use rusnix_ir::{self as rusnix, Expr};

#[rusnix::options]
mod options {
    #[rusnix(root)]
    struct Root {
        port: i64,
    }
}

fn needs_boolean(_: Expr<bool>) {}

fn main() {
    needs_boolean(options::root().port());
}
