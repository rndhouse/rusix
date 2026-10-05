use rusnix_ir::IntoConfig;

#[derive(IntoConfig)]
#[rusnix(rename_all = "kebab-case")]
struct Config {
    listen_port: u16,
}

fn main() {}
