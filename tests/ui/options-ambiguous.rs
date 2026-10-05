use rusnix_ir as rusnix;

#[rusnix::options]
mod options {
    #[rusnix(root)]
    struct Root {
        listen_port: i64,
        #[rusnix(rename = "listenPort")]
        duplicate: i64,
    }
}

fn main() {}
