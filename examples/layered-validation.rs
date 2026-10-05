//! Demonstrates validation layers: Rust checks field types, Rusnix checks bindings,
//! and NixOS checks the final configuration against its option declarations.
use rusnix_ir::{self as rusnix, IntoRusnixValue};

// A reusable value type; the config module supplies its placement.
#[derive(IntoRusnixValue)]
pub struct Port(pub u16);

#[rusnix::config]
mod config {
    use super::Port;
    use rusnix_ir::nixos::NixosModule;

    // These structs describe just the options this configuration uses.
    #[rusnix(root)]
    pub struct SshContribution<T> {
        pub services: Services<T>,
    }

    pub struct Services<T> {
        pub openssh: T,
    }

    struct SshOptions {
        enable: bool,
        ports: Vec<Port>,
    }

    pub fn module() -> NixosModule {
        NixosModule::empty()
            .add(SshContribution {
                services: Services {
                    openssh: SshOptions {
                        enable: false,
                        ports: vec![Port(22)],
                    },
                },
            })
            .import("nixos/modules/services/networking/ssh/sshd.nix")
    }
}

pub use config::{Services, SshContribution, module};

fn main() {
    let module = module();
    // Compiling validates the bindings; evaluating the output checks NixOS types.
    let artifact = rusnix_nix::nixos::compile_module(&module).unwrap();
    println!("{}", artifact.module.source);
}
