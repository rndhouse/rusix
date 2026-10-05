//! Demonstrates a Rust enum that constrains configuration functions and fields.
//! NixOS still validates the final value; Rust checks every typed caller first.
use rusnix_ir::{self as rusnix, IntoConfig};

// Local structs and unit enums lower automatically through one module boundary.
#[rusnix::config]
mod config {
    pub enum Mode {
        Server,
        Client,
    }

    pub fn accepts_connections(mode: &Mode) -> bool {
        match mode {
            Mode::Server => true,
            Mode::Client => false,
        }
    }

    struct ConnectionPolicy {
        mode: Mode,
        accepts_connections: bool,
    }

    #[rusnix(root)]
    pub struct Root {
        demo: ConnectionPolicy,
    }

    pub fn model() -> Root {
        let mode = Mode::Server;
        Root {
            demo: ConnectionPolicy {
                accepts_connections: accepts_connections(&mode),
                mode,
            },
        }
    }
}

pub use config::{Mode, accepts_connections, model};

fn main() {
    let generated = rusnix_nix::compile(&model().into_config()).unwrap();
    println!("{}", generated.source);
}
