//! Demonstrates OptionRef: Rust declares a typed symbolic dependency.
//! NixOS resolves its final value after ordinary module merging.
use rusnix_ir::{
    self as rusnix,
    nixos::{NixosModule, OptionRef},
};

#[rusnix::config]
mod config {
    use rusnix_ir::Expr;

    // Nested structs determine where the command appears in Nix.
    #[rusnix(root)]
    pub struct ExampleService {
        services: Services,
        systemd: Systemd,
    }

    struct Services {
        example: ExampleOptions,
    }

    struct ExampleOptions {
        enable: bool,
    }

    struct Systemd {
        services: Units,
    }

    struct Units {
        example: Unit,
    }

    struct Unit {
        service_config: ServiceConfig,
    }

    #[rusnix(rename_all = "PascalCase")]
    struct ServiceConfig {
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
    let port = OptionRef::<i64>::new("services.example.port");
    let command = port.into_expr().to_text().with_prefix("example --port=");
    let service = service(command);
    let module = NixosModule::empty().add(service);

    // This Nix module follows later overrides of config.services.example.port.
    let artifact = rusnix_nix::nixos::compile_module(&module).unwrap();
    println!("{}", artifact.module.source);
}
