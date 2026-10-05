use rusnix_ir::IntoConfig;

#[derive(IntoConfig)]
struct Config {
    #[rusnix(rename_all = "PascalCase")]
    listen_port: u16,
}

fn main() {}
