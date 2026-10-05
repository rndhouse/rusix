//! Demonstrates typed fields in a reusable Rust struct.
//! Nested structs become nested Nix attributes without handwritten paths.
use rusnix_ir::{self as rusnix, IntoConfig, IntoRusnixValue};

// These are normal Rust types chosen by this configuration.
#[derive(IntoRusnixValue)]
pub struct Hostname(pub String);

#[derive(IntoRusnixValue)]
pub struct Port(pub u16);

#[derive(IntoRusnixValue)]
pub struct Endpoint {
    pub host: Hostname,
    pub port: Port,
}

// Reusable types above keep their derives; this local tree needs one boundary.
#[rusnix::config]
mod config {
    use super::{Endpoint, Hostname, Port};

    #[rusnix(root)]
    pub struct Root {
        demo: Demo,
    }

    struct Demo {
        endpoint: Endpoint,
    }

    pub fn model() -> Root {
        Root {
            demo: Demo {
                endpoint: Endpoint {
                    host: Hostname("service.internal".into()),
                    port: Port(443),
                },
            },
        }
    }
}

pub use config::model;

fn main() {
    let generated = rusnix_nix::compile(&model().into_config()).unwrap();
    println!("{}", generated.source);
}
