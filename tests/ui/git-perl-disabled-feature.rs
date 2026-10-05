#[path = "../../examples/git/model.rs"]
mod model;

fn main() {
    let _ = model::Perl::Disabled { send_email: true };
}
