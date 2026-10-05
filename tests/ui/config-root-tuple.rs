use rusnix_ir as rusnix;

#[rusnix::config]
mod config {
    #[rusnix(root)]
    struct Machine(bool);
}

fn main() {}
