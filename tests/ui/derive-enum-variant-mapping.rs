use rusix_ir::IntoRusixValue;

#[derive(IntoRusixValue)]
enum Mode {
    #[rusix(flatten)]
    Server,
}

fn main() {}
