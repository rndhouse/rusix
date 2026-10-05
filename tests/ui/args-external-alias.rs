use rusnix_ir as rusnix;

type External = String;

#[rusnix::args]
mod args {
    use super::External;

    #[rusnix(root)]
    struct Root {
        external: External,
    }
}

fn main() {}
