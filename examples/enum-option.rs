//! Demonstrates a Rust enum that constrains configuration functions and fields.
//! NixOS still validates the final value; Rust checks every typed caller first.
use rusnix_ir::{self as rusnix, IntoConfig};

// Local structs and unit enums lower automatically through one module boundary.
#[rusnix::config]
mod config {
    /// A choice defined by this configuration, not a built-in Rusnix type.
    /// Rusnix maps these unit variants to "server" and "client" automatically.
    pub enum Mode {
        /// Accept incoming connections.
        Server,
        /// Make outgoing connections without accepting incoming ones.
        Client,
    }

    // This is Rust policy, so adding a mode requires updating this match.
    pub fn accepts_connections(mode: &Mode) -> bool {
        match mode {
            Mode::Server => true,
            Mode::Client => false,
        }
    }

    // Groups a typed choice with a decision computed from it before lowering.
    struct ConnectionPolicy {
        // Remains a Mode throughout Rust code; becomes a string in Nix.
        mode: Mode,
        // A concrete Rust decision, not a read of final NixOS configuration.
        accepts_connections: bool,
    }

    /// A rooted contribution whose nested fields define the generated attribute paths.
    #[rusnix(root)]
    pub struct Root {
        // Places the policy under the fictional `demo` option tree.
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
    // IntoConfig lowers the tree; compilation emits Nix without evaluating it.
    let generated = rusnix_nix::compile(&model().into_config()).unwrap();
    println!("{}", generated.source);
}
