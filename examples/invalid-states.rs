//! Demonstrates a Rust enum whose Plain case cannot carry TLS credentials.
//! The Tls case requires both credentials before Rusnix can lower it.
use rusnix_ir::{self as rusnix, IntoConfig, IntoRusnixValue, RusnixValue};

/// An ordinary Rust sum type: credentials exist only in the TLS alternative.
/// This shape cannot express disabled TLS with credentials or TLS missing a key.
pub enum Transport {
    /// Carries no credentials, so a plain connection cannot accidentally retain them.
    Plain,
    /// Requires both credential paths; file existence and contents are not checked in Rust.
    Tls {
        /// Identifies the public certificate file, distinct from the private key.
        certificate: Certificate,
        /// Identifies the matching private key file; cannot be replaced by a Certificate.
        private_key: PrivateKey,
    },
}

/// A reusable certificate-path type whose inner string is preserved during lowering.
#[derive(IntoRusnixValue)]
pub struct Certificate(pub String);

/// A separate key-path type; Rust prevents mixing it up with a Certificate.
#[derive(IntoRusnixValue)]
pub struct PrivateKey(pub String);

// Local structs lower automatically; the reusable types above supply their own conversions.
#[rusnix::config]
mod config {
    use super::{Certificate, PrivateKey, Transport};

    /// Places this service model under the fictional `demo` namespace.
    #[rusnix(root)]
    pub struct Root {
        /// The complete service contribution; nested fields determine its Nix shape.
        pub demo: ServiceConfig,
    }

    /// A component built from one valid transport alternative.
    pub struct ServiceConfig {
        /// Requires a complete Transport rather than independent TLS flags and credentials.
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
        // The enum's Nix representation is a domain choice, so it needs a custom conversion.
        // NixOS can still validate the resulting record against its own schema.
        // A plain connection emits only the TLS flag, with no credential fields.
        #[derive(IntoRusnixValue)]
        struct Plain {
            // The lowered flag for this alternative is always false.
            tls: bool,
        }

        // A TLS connection emits the flag together with both required paths.
        #[derive(IntoRusnixValue)]
        struct Tls {
            // The lowered flag for this alternative is always true.
            tls: bool,
            // Preserves the certificate selected by the Rust model.
            certificate: Certificate,
            // Becomes `privateKey` under Rusnix's default naming convention.
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
    // Lower the valid Rust model to Nix; backend schemas and credential files remain unchecked here.
    let generated = rusnix_nix::compile(&model().into_config()).unwrap();
    println!("{}", generated.source);
}
