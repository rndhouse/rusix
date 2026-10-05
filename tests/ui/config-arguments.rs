use rusnix_ir as rusnix;

#[rusnix::config(prefix = "services")]
mod config {
    #[rusnix(root)]
    struct Machine {
        enable: bool,
    }
}

fn main() {}
