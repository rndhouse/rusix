use rusix_ir::IntoConfig;

#[derive(IntoConfig)]
struct Root {
    #[rusix(omit_none, flatten)]
    value: Option<String>,
}

fn main() {}
