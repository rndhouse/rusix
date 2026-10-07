//! Builds a service command that follows the final merged value of `services.example.port` in
//! NixOS.
//! The fictional service needs separate option declarations; Rust's OptionRef generates a
//! reference without reading the port.

use rusnix_ir::{
    self as rusnix,
    nixos::{NixosModule, OptionRef},
};

// One boundary supplies structural lowering for all the local configuration structs.
#[rusnix::config]
mod config {
    use rusnix_ir::Expr;

    /// A rooted contribution combining service inputs with a dependent systemd command.
    /// Nested structs determine the Nix attribute paths; this is an ordinary Rust model.
    #[rusnix(root)]
    pub struct ExampleService {
        // Defines the example service's input options.
        services: Services,
        // Defines a systemd command that can depend on a final merged input.
        systemd: Systemd,
    }

    // Places this configuration's service under the `services` namespace.
    struct Services {
        /// Input options for the fictional service, emitted under services.example.
        example: ExampleOptions,
    }

    // Concrete service inputs supplied before lowering.
    struct ExampleOptions {
        // Emits a concrete enable definition, unlike the deferred port dependency.
        enable: bool,
    }

    struct Systemd {
        /// Named service units contributed under the NixOS systemd.services namespace.
        services: Units,
    }

    struct Units {
        /// Service unit whose startup command follows the final services.example.port option.
        example: Unit,
    }

    // Separates a unit's systemd properties from its NixOS wrapper.
    struct Unit {
        // Automatically becomes `serviceConfig` under the default naming rule.
        service_config: ServiceConfig,
    }

    // Systemd properties use PascalCase instead of the default lowerCamelCase.
    #[rusnix(rename_all = "PascalCase")]
    struct ServiceConfig {
        // Expr<String> stays deferred and becomes `ExecStart`; it is not a Rust string.
        exec_start: Expr<String>,
    }

    // The command stays an Expr, so lowering preserves its symbolic dependency.
    pub fn service(command: Expr<String>) -> ExampleService {
        ExampleService {
            services: Services {
                example: ExampleOptions { enable: true },
            },
            systemd: Systemd {
                services: Units {
                    example: Unit {
                        service_config: ServiceConfig {
                            exec_start: command,
                        },
                    },
                },
            },
        }
    }
}

pub use config::service;

fn main() {
    // Concrete values are fixed before lowering.
    let concrete_port = 5432_i64;
    println!("Concrete Rust command: example --port={concrete_port}");

    // OptionRef does not read a value in Rust; NixOS resolves config.* after merging.
    // i64 is our expected type; NixOS still owns the actual option declaration.
    let port = OptionRef::<i64>::new("services.example.port");
    let command = port.into_expr().to_text().with_prefix("example --port=");
    let service = service(command);

    // Each add preserves an independent contribution for normal NixOS merging and priorities.
    let module = NixosModule::empty().add(service);

    // This Nix module follows later overrides of config.services.example.port.
    let artifact = rusnix_nix::nixos::compile_module(&module).unwrap();
    println!("{}", artifact.module.source);
}
