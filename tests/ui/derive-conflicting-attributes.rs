use rusnix_ir::IntoConfig;

#[derive(IntoConfig)]
struct Example {
    #[rusnix(skip, rename = "enable")]
    enabled: bool,
}

fn main() {}
