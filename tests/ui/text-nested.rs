fn main() {
    let _ = rusix_ir::nix_text!("{outer{inner}}", outer = "value");
}
