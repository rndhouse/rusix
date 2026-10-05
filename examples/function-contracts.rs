//! Demonstrates a typed function contract for configuration.
//! A caller must supply an Endpoint and a Transport, rather than arbitrary strings or flags.
use rusnix_ir::{self as rusnix, IntoConfig, IntoRusnixValue, RusnixValue};

// These reusable parameter types lower wherever a local tree places them.
#[derive(IntoRusnixValue)]
pub struct Hostname(pub String);

#[derive(IntoRusnixValue)]
pub struct Port(pub u16);

#[derive(IntoRusnixValue)]
pub struct Endpoint {
    pub host: Hostname,
    pub port: Port,
}

pub enum Transport {
    Plain,
    Tls {
        certificate: String,
        private_key: String,
    },
}

pub fn configure_service(endpoint: Endpoint, transport: Transport) -> ServiceConfig {
    ServiceConfig {
        endpoint,
        transport,
    }
}

#[rusnix::config]
mod config {
    use super::{Endpoint, Hostname, Port, Transport, configure_service};

    pub struct ServiceConfig {
        pub endpoint: Endpoint,
        pub transport: Transport,
    }

    #[rusnix(root)]
    pub struct Root {
        demo: ServiceConfig,
    }

    pub fn model() -> Root {
        Root {
            demo: configure_service(
                Endpoint {
                    host: Hostname("service.internal".into()),
                    port: Port(8080),
                },
                Transport::Plain,
            ),
        }
    }
}

pub use config::{ServiceConfig, model};

impl IntoRusnixValue for Transport {
    fn into_value(self) -> RusnixValue {
        // Function-local records express the Nix shape chosen by this explicit mapping.
        #[derive(IntoRusnixValue)]
        struct Plain {
            tls: bool,
        }

        #[derive(IntoRusnixValue)]
        struct Tls {
            tls: bool,
            certificate: String,
            private_key: String,
        }

        match self {
            Self::Plain => Plain { tls: false }.into_value(),
            Self::Tls {
                certificate,
                private_key,
            } => Tls {
                tls: true,
                certificate,
                private_key,
            }
            .into_value(),
        }
    }
}

fn main() {
    let generated = rusnix_nix::compile(&model().into_config()).unwrap();
    println!("{}", generated.source);
}
