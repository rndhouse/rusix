use rusnix_ir::IntoConfig;

#[derive(IntoConfig)]
struct Root {
    #[rusnix(omit_none)]
    value: i64,
}

fn main() {}
