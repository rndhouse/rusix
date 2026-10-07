use rusix_ir::IntoRusixValue;

#[derive(IntoRusixValue)]
#[rusix(omit_none)]
enum Mode {
    Server,
}

fn main() {}
