use rusix_ir as rusix;

#[rusix::config]
mod config {
    #[rusix(root)]
    enum Machine {
        Enabled,
        Disabled,
    }
}

fn main() {}
