//! Builds a fictional `demo` configuration through a function requiring typed endpoint and
//! transport inputs.
//! Rust checks the caller's types; conversion places the result in nested Nix attributes.
//!
//! ```nix
//! demo = { endpoint = { host = "service.internal"; port = 8080; }; transport.tls = false; };
//! ```

use rusnix_ir::{self as rusnix, IntoRusnixValue, RusnixValue};

/// A reusable hostname-domain string; no hostname syntax check is implied.
#[derive(IntoRusnixValue)]
pub struct Hostname(
    /// Hostname preserved as text in the generated Nix value.
    pub String,
);

/// A listening-port value, kept distinct from other integer domains in Rust.
#[derive(IntoRusnixValue)]
pub struct Port(
    /// Listening port emitted as an integer in Nix.
    pub u16,
);

/// A reusable input record whose fields keep their semantic types across function calls.
#[derive(IntoRusnixValue)]
pub struct Endpoint {
    /// Identifies the host using the caller's Hostname type.
    pub host: Hostname,
    /// Requires Port rather than accepting any integer-backed value.
    pub port: Port,
}

/// A complete transport choice supplied by the caller, not inferred from loose flags.
pub enum Transport {
    /// A connection that needs no TLS credentials.
    Plain,
    /// A TLS connection requires both paths; their actual files remain unchecked here.
    Tls {
        /// Path to the public certificate to emit into configuration.
        certificate: String,
        /// Path to the private key, required along with the certificate.
        private_key: String,
    },
}

/// Statically requires both domain inputs; callers cannot substitute unrelated values.
pub fn configure_service(endpoint: Endpoint, transport: Transport) -> ServiceConfig {
    ServiceConfig {
        endpoint,
        transport,
    }
}

// The module macro lowers local structs; Endpoint and Transport keep their own conversions.
#[rusnix::config]
mod config {
    use super::{Endpoint, Hostname, Port, Transport, configure_service};

    /// The function's typed result; its parent decides where it belongs in Nix.
    pub struct ServiceConfig {
        /// Carries the typed endpoint without erasing its host/port distinction.
        pub endpoint: Endpoint,
        /// Carries one complete transport alternative.
        pub transport: Transport,
    }

    /// A rooted contribution for the fictional service used in this example.
    #[rusnix(root)]
    pub struct Root {
        // Places the function's result at `demo`, with nested records beneath it.
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
        // This custom mapping chooses the Nix shape for each Rust alternative:
        // Plain emits { tls = false; }; Tls adds both credential paths with tls = true.
        #[derive(IntoRusnixValue)]
        struct Plain {
            /// Disabled TLS flag; this record shape omits both credential paths.
            tls: bool,
        }

        #[derive(IntoRusnixValue)]
        struct Tls {
            /// Enabled TLS flag emitted together with both credential paths.
            tls: bool,
            /// Certificate filename emitted as a Nix string without reading the file.
            certificate: String,
            // Private-key filename emitted as `privateKey` without reading the file.
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
    // Compilation emits the typed function's result; NixOS schema checks happen during evaluation.
    let generated = rusnix_nix::compile(model()).unwrap();
    println!("{}", generated.source);
}
