//! Demonstrates typed Rust models alongside existing Nix packages, modules and functions.
//! Opaque handles preserve Nix objects without package-specific Rust bindings.
use rusnix_ir::{
    self as rusnix, Config, IntoConfig, IntoRusnixValue, RusnixValue,
    interop::{InputRef, ModuleRef, NixFunction, NixValue, Nixpkgs, OverlayRef, PackageRef},
    nixos::NixosModule,
};

// Rust requires both credentials when this configuration chooses TLS.
pub enum Transport {
    Plain,
    Tls {
        certificate: String,
        private_key: String,
    },
}

impl IntoRusnixValue for Transport {
    fn into_value(self) -> RusnixValue {
        // Function-local records express the two shapes chosen by this mapping.
        #[derive(IntoRusnixValue)]
        struct Plain {
            tls: bool,
        }

        #[derive(IntoRusnixValue)]
        struct Tls {
            tls: bool,
            certificate: String,
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

#[rusnix::config]
mod config {
    use super::{NixValue, PackageRef, Transport};

    #[rusnix(root)]
    pub struct OwnedContribution {
        pub demo: Transport,
    }

    // Rust describes the package list; Nix resolves the actual packages.
    #[rusnix(root)]
    pub struct PackageContribution {
        environment: Environment,
    }

    struct Environment {
        system_packages: Vec<PackageRef>,
    }

    pub fn packages(packages: Vec<PackageRef>) -> PackageContribution {
        PackageContribution {
            environment: Environment {
                system_packages: packages,
            },
        }
    }

    #[rusnix(root)]
    pub struct FunctionResult {
        pub result: NixValue,
    }
}

use config::FunctionResult;
pub use config::{OwnedContribution, packages};

pub fn module(local: InputRef) -> NixosModule {
    let pkgs = Nixpkgs::new();
    let hello: PackageRef = pkgs.get("hello");
    let requests = pkgs.get("python312Packages.requests");
    let ssh: ModuleRef = pkgs.module("services/networking/ssh/sshd.nix");
    let overlay: OverlayRef = local.overlay("overlays.example");
    let overlaid = pkgs.with_overlay(overlay).get("rusnixOverlayHello");
    let external = local.package("packages.example");

    // Config::set is the escape hatch for an option path chosen at runtime.
    let arbitrary_path = ["services", "rusnixExternal", "enable"].join(".");
    let arbitrary = Config::new().set(arbitrary_path, true);
    NixosModule::empty()
        .add(packages(vec![hello, requests, overlaid, external]))
        .add(arbitrary)
        .import_ref(ssh)
        .import_ref(local.module("nixosModules.example"))
}

fn main() {
    // An ordinary Rust enum describes our own model.
    let owned = OwnedContribution {
        demo: Transport::Plain,
    };
    println!(
        "{}",
        rusnix_nix::compile(&owned.into_config()).unwrap().source
    );

    // The input file is needed when Nix evaluates the output, not when Rust lowers it.
    let local = InputRef::local("example", "input.nix");
    let module = module(local);
    let artifact = rusnix_nix::nixos::compile_module(&module).unwrap();
    println!("{}", artifact.module.source);

    // Nix checks this opaque function's arguments and result when it evaluates the call.
    let uppercase: NixFunction = Nixpkgs::new().function("toUpper");
    let value: NixValue = uppercase.call("rusnix");
    let generated = rusnix_nix::compile(&FunctionResult { result: value }.into_config()).unwrap();
    println!("{}", generated.source);

    // Mixed records cross the same opaque boundary; Nix owns the builder's schema.
    let pkgs = Nixpkgs::new();
    let args = NixValue::record([
        ("name", "example.conf".into()),
        ("text", "workers = 4\n".into()),
        ("executable", false.into()),
        (
            "passthru",
            NixValue::record([("package", pkgs.get("hello").into())]),
        ),
    ]);
    let file = pkgs.package_function("writeTextFile").call(args);
    let generated = rusnix_nix::compile(&FunctionResult { result: file }.into_config()).unwrap();
    println!("{}", generated.source);
}
