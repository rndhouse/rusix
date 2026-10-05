//! Demonstrates OptionRef: Rust declares a typed symbolic dependency.
//! NixOS resolves its final value after ordinary module merging.
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
        // Introduces `services.example`, not an entire generated NixOS schema.
        example: ExampleOptions,
    }

    // Concrete service inputs supplied before lowering.
    struct ExampleOptions {
        // Emits a concrete enable definition, unlike the deferred port dependency.
        enable: bool,
    }

    // Places dependent outputs under NixOS's systemd namespace.
    struct Systemd {
        // Contains the unit definitions contributed by this component.
        services: Units,
    }

    // Names the unit that consumes the example service's port.
    struct Units {
        // Produces `systemd.services.example` independently of `services.example`.
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
