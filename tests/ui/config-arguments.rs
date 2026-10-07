use rusix_ir as rusix;

#[rusix::config(prefix = "services")]
mod config {
    #[rusix(root)]
    struct Machine {
        enable: bool,
    }
}

fn main() {}
