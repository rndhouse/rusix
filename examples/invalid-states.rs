//! Demonstrates a Rust enum whose Plain case cannot carry TLS credentials.
//! The Tls case requires both credentials before Rusnix can lower it.
use rusnix_ir::{self as rusnix, IntoConfig, IntoRusnixValue, RusnixValue};

pub enum Transport {
    Plain,
    Tls {
        certificate: Certificate,
        private_key: PrivateKey,
    },
}

// Reusable credential types can also be lowered outside this local tree.
#[derive(IntoRusnixValue)]
pub struct Certificate(pub String);

#[derive(IntoRusnixValue)]
pub struct PrivateKey(pub String);

#[rusnix::config]
mod config {
    use super::{Certificate, PrivateKey, Transport};

    #[rusnix(root)]
    pub struct Root {
        pub demo: ServiceConfig,
    }

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
}

pub use config::{Root, ServiceConfig, model};

impl IntoRusnixValue for Transport {
    fn into_value(self) -> RusnixValue {
        // This explicit mapping uses function-local records for the two Nix shapes.
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
