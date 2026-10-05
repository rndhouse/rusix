use rusnix_ir as rusnix;

#[rusnix::args]
mod args {
    #[rusnix(root)]
    struct Root {
        port: u16,
    }
}

fn main() {}
