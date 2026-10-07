//! Reuses a typed Rust endpoint by placing it in a fictional `demo` configuration tree.
//! Nested structs become nested Nix attributes, while single-field wrappers become their
//! primitive values.
//!
//! ```nix
//! demo.endpoint = { host = "service.internal"; port = 443; };
//! ```

use rusnix_ir::{self as rusnix, IntoConfig, IntoRusnixValue};

/// An ordinary Rust domain type; its inner value lowers to a Nix string.
/// This separates names from other strings without adding hostname validation.
#[derive(IntoRusnixValue)]
pub struct Hostname(pub String);

/// A distinct Rust type for a listening port; its inner value lowers to an integer.
#[derive(IntoRusnixValue)]
pub struct Port(pub u16);

/// A reusable record that can appear wherever a parent places it.
#[derive(IntoRusnixValue)]
pub struct Endpoint {
    /// Requires a hostname-domain value rather than an arbitrary domain string.
    pub host: Hostname,
    /// Requires a Port; another integer-backed domain type will not compile here.
    pub port: Port,
}

// Reusable types above keep their derives; this local tree needs one boundary.
#[rusnix::config]
mod config {
    use super::{Endpoint, Hostname, Port};

    /// Places the reusable Endpoint in this configuration's attribute tree.
    #[rusnix(root)]
    pub struct Root {
        demo: Demo,
    }

    // Structural placement belongs to the parent, not to the reusable value.
    struct Demo {
        // Produces `demo.endpoint.host` and `demo.endpoint.port`.
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
    // Prints source only; evaluating it as NixOS needs declarations for the fictional demo options.
    let generated = rusnix_nix::compile(&model().into_config()).unwrap();
    println!("{}", generated.source);
}
