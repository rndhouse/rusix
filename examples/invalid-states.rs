//! Demonstrates a Rust enum whose Plain case cannot carry TLS credentials.
//! The Tls case requires both credentials before Rusnix can lower it.
use rusnix_ir::{IntoConfig, IntoRusnixValue, RusnixValue};

pub enum Transport {
    Plain,
    Tls {
        certificate: Certificate,
        private_key: PrivateKey,
    },
}

#[derive(IntoRusnixValue)]
pub struct Certificate(pub String);

#[derive(IntoRusnixValue)]
pub struct PrivateKey(pub String);

#[derive(IntoConfig)]
pub struct Root {
    pub demo: ServiceConfig,
}

#[derive(IntoRusnixValue)]
pub struct ServiceConfig {
    pub transport: Transport,
}

pub fn model() -> Root {
    Root {
        demo: ServiceConfig {
            transport: Transport::Tls {
                certificate: Certificate("/run/keys/service.pem".into()),
                private_key: PrivateKey("/run/keys/service.key".into()),
            },
        },
    }
}

impl IntoRusnixValue for Transport {
    fn into_value(self) -> RusnixValue {
        // Each enum case maps to a different Nix record shape.
        #[derive(IntoRusnixValue)]
        struct Plain {
            tls: bool,
        }

        #[derive(IntoRusnixValue)]
        struct Tls {
            tls: bool,
            certificate: Certificate,
            private_key: PrivateKey,
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
