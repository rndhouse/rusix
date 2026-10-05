use rusnix_ir::IntoRusnixValue;

#[derive(IntoRusnixValue)]
#[rusnix(rename_all = "snake_case")]
enum Mode {
    ReadOnly,
}

fn main() {}
