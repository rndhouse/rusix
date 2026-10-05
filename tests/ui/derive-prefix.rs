use rusnix_ir::IntoConfig;

#[derive(IntoConfig)]
#[rusnix(prefix = "services.example")]
struct Example {
    enable: bool,
}

fn main() {}
