use rusnix_ir::IntoConfig;

#[derive(IntoConfig)]
struct Root {
    #[rusnix(omit_none, flatten)]
    value: Option<String>,
}

fn main() {}
