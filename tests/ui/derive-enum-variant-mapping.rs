use rusnix_ir::IntoRusnixValue;

#[derive(IntoRusnixValue)]
enum Mode {
    #[rusnix(flatten)]
    Server,
}

fn main() {}
