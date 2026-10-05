use rusnix_ir as rusnix;

#[rusnix::config]
mod config {
    #[rusnix(root, root)]
    struct Machine {
        enable: bool,
    }
}

fn main() {}
