//! Uses one Rust enum to compute firewall and service choices in a fictional `demo` tree.
//! Adding a variant requires updating both Rust matches before Nix source can be generated.
//!
//! ```nix
//! demo = { mode = "client"; firewall.allowedPorts = []; service.acceptsConnections = false; };
//! ```

use rusnix_ir::{self as rusnix};

// The enum, policy consumers and local configuration tree share one boundary.
#[rusnix::config]
mod config {
    /// One shared model choice; Rusnix automatically lowers unit variants to strings.
    pub enum Mode {
        /// A server needs both inbound firewall access and a listening service.
        Server,
        /// A client needs neither inbound access nor a listening service.
        Client,
    }

    /// An ordinary Rust port type; its inner number lowers to a Nix integer.
    pub struct Port(pub u16);

    /// The firewall decision computed from the model before any Nix evaluation.
    pub struct FirewallPolicy {
        /// Ports selected by Rust policy; order is preserved in the lowered list.
        pub allowed_ports: Vec<Port>,
    }

    /// A separate consumer of the same model choice.
    pub struct ServicePolicy {
        /// A concrete decision to allow incoming connections.
        pub accepts_connections: bool,
    }

    // No wildcard: a new mode must have an explicit policy in both functions.
    pub fn firewall_policy(mode: &Mode) -> FirewallPolicy {
        match mode {
            Mode::Server => FirewallPolicy {
                allowed_ports: vec![Port(443)],
            },
            Mode::Client => FirewallPolicy {
                allowed_ports: vec![],
            },
        }
    }

    pub fn service_policy(mode: &Mode) -> ServicePolicy {
        match mode {
            Mode::Server => ServicePolicy {
                accepts_connections: true,
            },
            Mode::Client => ServicePolicy {
                accepts_connections: false,
            },
        }
    }

    // Collects the model choice and both decisions under one fictional option tree.
    struct Policies {
        mode: Mode,
        firewall: FirewallPolicy,
        service: ServicePolicy,
    }

    /// A rooted contribution; the local structs supply automatic structural lowering.
    #[rusnix(root)]
    pub struct Root {
        // Keeps both Rust-computed policies under the fictional demo namespace.
        demo: Policies,
    }

    pub fn model() -> Root {
        let mode = Mode::Client;

        Root {
            demo: Policies {
                firewall: firewall_policy(&mode),
                service: service_policy(&mode),
                mode,
            },
        }
    }
}

pub use config::{Mode, firewall_policy, model, service_policy};

fn main() {
    // Rust checks that both matches are exhaustive; lowering emits their chosen values.
    let generated = rusnix_nix::compile(model()).unwrap();
    println!("{}", generated.source);
}
