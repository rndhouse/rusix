use rusix_ir as rusix;

#[rusix::config]
mod config {
    #[rusix(root)]
    struct Machine(bool);
}

fn main() {}
