//! Demonstrates a typed function contract for configuration.
//! A caller must supply an Endpoint and a Transport, rather than arbitrary strings or flags.
use rusnix_ir::{IntoConfig, IntoRusnixValue, RusnixValue};

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

#[derive(IntoRusnixValue)]
pub struct ServiceConfig {
    endpoint: Endpoint,
    transport: Transport,
}

pub fn configure_service(endpoint: Endpoint, transport: Transport) -> ServiceConfig {
    ServiceConfig {
        endpoint,
        transport,
    }
}

#[derive(IntoConfig)]
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

impl IntoRusnixValue for Transport {
    fn into_value(self) -> RusnixValue {
        // The caller uses an enum; Nix receives the selected record shape.
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
