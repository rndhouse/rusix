use rusix_ir as rusix;

#[rusix::args]
mod args {
    #[rusix(root)]
    struct Root {
        port: u16,
    }
}

fn main() {}
