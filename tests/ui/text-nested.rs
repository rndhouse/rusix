fn main() {
    let _ = rusnix_ir::nix_text!("{outer{inner}}", outer = "value");
}
