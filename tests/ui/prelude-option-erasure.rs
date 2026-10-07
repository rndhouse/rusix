use rusix_ir::prelude::*;

fn main() {
    let _ = OptionRef::<String>::new("services.example.command").as_value();
}
