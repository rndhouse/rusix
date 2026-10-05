#[path = "../support/nixos.rs"]
mod support;

use support::OpenSsh;

fn main() {
    OpenSsh::new().ports(vec!["twenty-two"]);
}
