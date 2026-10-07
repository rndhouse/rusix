use rusix_ir::IntoRusixValue;

#[derive(IntoRusixValue)]
enum Mode {
    Server,
    Client(String),
}

fn main() {}
