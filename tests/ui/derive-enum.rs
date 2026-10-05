use rusnix_ir::IntoRusnixValue;

#[derive(IntoRusnixValue)]
enum Mode {
    Server,
    Client,
}

fn main() {}
