use rusnix_ir::IntoRusnixValue;

#[derive(IntoRusnixValue)]
enum Mode {
    Server,
    #[rusnix(rename = "server")]
    Client,
}

fn main() {}
