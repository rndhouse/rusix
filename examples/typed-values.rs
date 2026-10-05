//! Demonstrates distinct Rust types with identical primitive representations.
//! A UserId cannot be passed to an API expecting Port, even when both contain 1000.
use rusnix_ir::{IntoConfig, IntoRusnixValue};

// These types are chosen by this configuration, not built into Rusnix.
#[derive(IntoRusnixValue)]
struct Hostname(String);

#[derive(IntoRusnixValue)]
pub struct Port(pub u16);

#[derive(IntoRusnixValue)]
pub struct UserName(pub String);

#[derive(IntoRusnixValue)]
pub struct UserId(pub u16);

pub fn listen(port: Port) -> Port {
    port
}

#[derive(IntoRusnixValue)]
struct Identity {
    user_id: UserId,
    #[rusnix(rename = "owner")]
    name: UserName,
}

#[derive(IntoRusnixValue)]
struct Listener {
    port: Port,
    host: Hostname,
    // Put Identity's fields beside port and host in the generated Nix record.
    #[rusnix(flatten)]
    owner: Identity,
}

#[derive(IntoConfig)]
pub struct Root {
    demo: Listener,
}

pub fn model() -> Root {
    Root {
        demo: Listener {
            port: listen(Port(1000)),
            host: Hostname("admin".into()),
            owner: Identity {
                user_id: UserId(1000),
                name: UserName("admin".into()),
            },
        },
    }
}

fn main() {
    let generated = rusnix_nix::compile(&model().into_config()).unwrap();
    println!("{}", generated.source);
}
