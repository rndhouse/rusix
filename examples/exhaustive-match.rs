//! Demonstrates two configuration decisions consuming the same Rust enum.
//! Adding a variant makes the compiler identify both incomplete matches.
use rusnix_ir::{IntoConfig, IntoRusnixValue, RusnixValue};

#[derive(Clone, Copy)]
pub enum Mode {
    Server,
    Client,
}

#[derive(IntoRusnixValue)]
pub struct Port(pub u16);

#[derive(IntoRusnixValue)]
pub struct FirewallPolicy {
    pub allowed_ports: Vec<Port>,
}

#[derive(IntoRusnixValue)]
pub struct ServicePolicy {
    pub accepts_connections: bool,
}

// No wildcard: a new mode must have an explicit policy in both functions.
pub fn firewall_policy(mode: Mode) -> FirewallPolicy {
    match mode {
        Mode::Server => FirewallPolicy {
            allowed_ports: vec![Port(443)],
        },
        Mode::Client => FirewallPolicy {
            allowed_ports: vec![],
        },
    }
}

pub fn service_policy(mode: Mode) -> ServicePolicy {
    match mode {
        Mode::Server => ServicePolicy {
            accepts_connections: true,
        },
        Mode::Client => ServicePolicy {
            accepts_connections: false,
        },
    }
}

#[derive(IntoRusnixValue)]
struct Policies {
    mode: Mode,
    firewall: FirewallPolicy,
    service: ServicePolicy,
}

#[derive(IntoConfig)]
pub struct Root {
    demo: Policies,
}

pub fn model() -> Root {
    let mode = Mode::Client;
    Root {
        demo: Policies {
            mode,
            firewall: firewall_policy(mode),
            service: service_policy(mode),
        },
    }
}

impl IntoRusnixValue for Mode {
    fn into_value(self) -> RusnixValue {
        match self {
            Self::Server => "server",
            Self::Client => "client",
        }
        .into_value()
    }
}

fn main() {
    let generated = rusnix_nix::compile(&model().into_config()).unwrap();
    println!("{}", generated.source);
}
