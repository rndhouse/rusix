//! Demonstrates two configuration decisions consuming the same Rust enum.
//! Adding a variant makes the compiler identify both incomplete matches.
use rusnix_ir::{self as rusnix, IntoConfig};

// The enum, policy consumers and local configuration tree share one boundary.
#[rusnix::config]
mod config {
    pub enum Mode {
        Server,
        Client,
    }

    pub struct Port(pub u16);

    pub struct FirewallPolicy {
        pub allowed_ports: Vec<Port>,
    }

    pub struct ServicePolicy {
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

    struct Policies {
        mode: Mode,
        firewall: FirewallPolicy,
        service: ServicePolicy,
    }

    #[rusnix(root)]
    pub struct Root {
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
    let generated = rusnix_nix::compile(&model().into_config()).unwrap();
    println!("{}", generated.source);
}
