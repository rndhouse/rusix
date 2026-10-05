use rusnix_ir::IntoRusnixValue;

#[derive(IntoRusnixValue)]
#[rusnix(rename_all = "PascalCase", rename_all = "lowerCamelCase")]
struct Settings {
    exec_start: String,
}

fn main() {}
