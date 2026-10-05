use rusnix_ir as rusnix;

#[rusnix::options]
mod options {
    #[rusnix(root, value)]
    struct Root {
        port: i64,
    }
}

fn main() {}
