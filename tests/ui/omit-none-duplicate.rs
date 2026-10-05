use rusnix_ir as rusnix;

#[rusnix::config]
mod config {
    #[rusnix(root, omit_none, omit_none)]
    struct Root {
        value: Option<String>,
    }
}

fn main() {}
