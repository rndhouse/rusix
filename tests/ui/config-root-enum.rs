use rusnix_ir as rusnix;

#[rusnix::config]
mod config {
    #[rusnix(root)]
    enum Machine {
        Enabled,
        Disabled,
    }
}

fn main() {}
