//! Demonstrates a Rust enum that constrains configuration functions and fields.
//! NixOS still validates the final value; Rust checks every typed caller first.
use rusnix_ir::{self as rusnix, IntoConfig, IntoRusnixValue, RusnixValue};

// Copy lets the same mode feed a decision and remain in the configuration.
#[derive(Clone, Copy)]
pub enum Mode {
    Server,
    Client,
}

pub fn accepts_connections(mode: Mode) -> bool {
    match mode {
        Mode::Server => true,
        Mode::Client => false,
    }
}

impl IntoRusnixValue for Mode {
    fn into_value(self) -> RusnixValue {
        // Leaf enums choose their Nix values with an exhaustive match.
        match self {
            Self::Server => "server",
            Self::Client => "client",
        }
        .into_value()
    }
}

// The local tree lowers automatically; Mode keeps its explicit mapping above.
#[rusnix::config]
mod config {
    use super::{Mode, accepts_connections};

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
                mode,
                accepts_connections: accepts_connections(mode),
            },
        }
    }
}

pub use config::model;

fn main() {
    let generated = rusnix_nix::compile(&model().into_config()).unwrap();
    println!("{}", generated.source);
}
