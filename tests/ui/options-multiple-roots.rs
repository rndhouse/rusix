use rusnix_ir as rusnix;

#[rusnix::options]
mod options {
    #[rusnix(root)]
    struct First {
        port: i64,
    }

    #[rusnix(root)]
    struct Second {
        enable: bool,
    }
}

fn main() {}
