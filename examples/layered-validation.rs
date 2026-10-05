//! Demonstrates validation layers: Rust checks field types, Rusnix checks bindings,
//! and NixOS checks the final configuration against its option declarations.
use rusnix_ir::{IntoConfig, IntoRusnixValue, nixos::NixosModule};

#[derive(IntoRusnixValue)]
pub struct Port(pub u16);

// These structs describe just the options this configuration uses.
#[derive(IntoConfig)]
pub struct SshContribution<T> {
    pub services: Services<T>,
}

#[derive(IntoRusnixValue)]
pub struct Services<T> {
    pub openssh: T,
}

#[derive(IntoRusnixValue)]
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

fn main() {
    let module = module();
    // Compiling validates the bindings; evaluating the output checks NixOS types.
    let artifact = rusnix_nix::nixos::compile_module(&module).unwrap();
    println!("{}", artifact.module.source);
}
