//! Shows how a Rust enum constrains configuration choices before emitting a fictional `demo` tree.
//! Rust computes the connection policy; Rusnix maps variant and field names to Nix strings and
//! attributes.
//!
//! ```nix
//! demo = { mode = "server"; acceptsConnections = true; };
//! ```

use rusnix_ir::{self as rusnix};

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
    let generated = rusnix_nix::compile(model()).unwrap();
    println!("{}", generated.source);
}
