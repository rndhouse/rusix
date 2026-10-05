use rusnix_ir::IntoRusnixValue;

#[derive(IntoRusnixValue)]
#[rusnix(omit_none)]
enum Mode {
    Server,
}

fn main() {}
