//! Keeps ports, user IDs and names distinct in Rust while emitting primitive values in a
//! fictional `demo` tree.
//! Renaming and flattening place the ownership fields beside the listener's fields in Nix.
//!
//! ```nix
//! demo = { port = 1000; host = "admin"; userId = 1000; owner = "admin"; };
//! ```

use rusnix_ir::{self as rusnix, IntoRusnixValue};

/// A listening-port domain value; the inner u16 lowers to a Nix integer.
/// The distinction from UserId is checked by Rust, not by a range validator.
#[derive(IntoRusnixValue)]
pub struct Port(
    /// Listening port emitted as an integer in Nix.
    pub u16,
);

/// A username-domain string, distinct from other strings in typed function calls.
#[derive(IntoRusnixValue)]
pub struct UserName(
    /// Account name emitted as a Nix string, distinct from a hostname in Rust.
    pub String,
);

/// A Unix-user identity with the same primitive representation as Port.
#[derive(IntoRusnixValue)]
pub struct UserId(
    /// User identifier emitted as a Nix integer, distinct from a port in Rust.
    pub u16,
);

/// Accepts only the port domain; `listen(UserId(1000))` cannot compile.
pub fn listen(port: Port) -> Port {
    port
}

// The local tree gets automatic conversions; reusable types above keep explicit derives.
#[rusnix::config]
mod config {
    use super::{Port, UserId, UserName, listen};

    // Local host-domain string; the module macro supplies its value conversion.
    struct Hostname(
        /// Hostname preserved as a Nix string; Rust keeps it separate from usernames.
        String,
    );

    // Keeps ownership information typed before placing it in the configuration tree.
    struct Identity {
        // Becomes `userId`, but cannot be passed to an API expecting Port in Rust.
        user_id: UserId,
        // This semantic rename calls the lowered username `owner`.
        #[rusnix(rename = "owner")]
        name: UserName,
    }

    // A listener uses a port and a host, with ownership fields flattened beside them.
    struct Listener {
        // Requires a port-domain value even though UserId also contains a u16.
        port: Port,
        /// Hostname emitted beside the port as a Nix string.
        host: Hostname,
        // Put Identity's fields beside port and host in the generated Nix record.
        #[rusnix(flatten)]
        owner: Identity,
    }

    /// Places the listener in the fictional `demo` option tree.
    #[rusnix(root)]
    pub struct Root {
        /// Listener fields emitted under demo, with user identity fields flattened beside them.
        demo: Listener,
    }

    pub fn model() -> Root {
        Root {
            demo: Listener {
                port: listen(Port(1000)),
                host: Hostname("admin".into()),
                owner: Identity {
                    user_id: UserId(1000),
                    name: UserName("admin".into()),
                },
            },
        }
    }
}

pub use config::model;

fn main() {
    // Nix receives primitive values; their semantic separation was enforced in Rust before lowering.
    let generated = rusnix_nix::compile(model()).unwrap();
    println!("{}", generated.source);
}
