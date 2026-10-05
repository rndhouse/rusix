use rusnix_ir as rusnix;

#[rusnix::options]
mod options {
    #[rusnix(root)]
    struct Root {
        port: u16,
    }
}

fn main() {}
