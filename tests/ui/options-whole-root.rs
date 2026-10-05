use rusnix_ir as rusnix;

#[rusnix::options]
mod options {
    #[rusnix(root)]
    struct Root {
        port: i64,
    }
}

fn main() {
    let _ = options::root().as_value();
}
