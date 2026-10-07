use rusix_ir as rusix;

#[rusix::options]
mod options {
    #[rusix(root)]
    struct Root {
        port: u16,
    }
}

fn main() {}
