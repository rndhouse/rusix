use rusnix_ir as rusnix;

type External = String;

#[rusnix::options]
mod options {
    use super::External;

    #[rusnix(root)]
    struct Root {
        external: External,
    }
}

fn main() {}
