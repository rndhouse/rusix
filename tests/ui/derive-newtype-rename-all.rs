use rusnix_ir::IntoRusnixValue;

#[derive(IntoRusnixValue)]
#[rusnix(rename_all = "PascalCase")]
struct Port(u16);

fn main() {}
