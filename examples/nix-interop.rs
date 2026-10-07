//! Demonstrates typed Rust models alongside existing Nix packages, modules and functions.
//! Opaque handles preserve Nix objects without package-specific Rust bindings.
use rusnix_ir::interop::raw::NixFunctionExt;
use rusnix_ir::{
    self as rusnix, Config, IntoConfig, IntoRusnixValue, RusnixValue,
    interop::{InputRef, ModuleRef, NixFunction, Nixpkgs, OverlayRef, PackageRef, raw::NixValue},
    nix_record,
    nixos::NixosModule,
};

/// A model owned by this configuration; Rust requires both credentials for TLS.
pub enum Transport {
    /// A plain connection carries no TLS paths.
    Plain,
    /// A TLS connection must supply both paths; this example does not inspect their files.
    Tls {
        /// Certificate path emitted as a string, not an opaque Nix ecosystem object.
        certificate: String,
        /// Private-key path required by the Rust alternative.
        private_key: String,
    },
}

impl IntoRusnixValue for Transport {
    fn into_value(self) -> RusnixValue {
        // Our domain chooses these two record shapes; opaque Nix objects need no such model.
        // The plain shape omits credential fields entirely.
        #[derive(IntoRusnixValue)]
        struct Plain {
            // Emits the disabled-TLS flag for Transport::Plain.
            tls: bool,
        }

        // The TLS shape carries the two paths required by Transport::Tls.
        #[derive(IntoRusnixValue)]
        struct Tls {
            // Emits the enabled-TLS flag for this alternative.
            tls: bool,
            // Preserves the concrete certificate-path string.
            certificate: String,
            // Becomes `privateKey` under the default naming rule.
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

// One module boundary lowers these local trees; each explicit root is a separate contribution type.
#[rusnix::config]
mod config {
    use super::{NixValue, PackageRef, Transport};

    /// A rooted contribution for the domain model this Rust program owns.
    #[rusnix(root)]
    pub struct OwnedContribution {
        /// Places our typed Transport at `demo`; NixOS can validate the resulting shape later.
        pub demo: Transport,
    }

    /// A separate rooted contribution for packages from the existing Nix ecosystem.
    #[rusnix(root)]
    pub struct PackageContribution {
        // Introduces the existing NixOS environment namespace.
        environment: Environment,
    }

    // Rust describes the list's category, not the packages' internal schemas.
    struct Environment {
        // PackageRef keeps actual Nix package expressions; NixOS checks the resolved objects.
        system_packages: Vec<PackageRef>,
    }

    pub fn packages(packages: Vec<PackageRef>) -> PackageContribution {
        PackageContribution {
            environment: Environment {
                system_packages: packages,
            },
        }
    }

    /// A generic output contribution used to display an opaque function result.
    #[rusnix(root)]
    pub struct FunctionResult {
        /// Keeps the Nix-side result deferred; Rust does not infer its type or stringify it.
        pub result: NixValue,
    }
}

use config::FunctionResult;
pub use config::{OwnedContribution, packages};

pub fn module(local: InputRef) -> NixosModule {
    // These lookups describe existing Nix objects; Rust never loads their internals.
    // Dotted paths select nested attributes without package-specific Rust bindings.
    let pkgs = Nixpkgs::new();
    let hello: PackageRef = pkgs.get("hello");
    let requests = pkgs.get("python312Packages.requests");
    let ssh: ModuleRef = pkgs.module("services/networking/ssh/sshd.nix");

    // Nix applies the overlay; a local input can supply packages and modules too.
    let overlay: OverlayRef = local.overlay("overlays.example");
    let overlaid = pkgs.with_overlay(overlay).get("rusnixOverlayHello");
    let external = local.package("packages.example");

    // Config::set is the escape hatch for an option path chosen at runtime.
    let arbitrary_path = ["services", "rusnixExternal", "enable"].join(".");
    let arbitrary = Config::new().set(arbitrary_path, true);

    // Each add remains an independent contribution, retaining NixOS merge and priority rules.
    // Imports reuse upstream modules and retain a Rust boundary if their Nix code fails.
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
    let args = nix_record! {
        "name": "example.conf",
        "text": "workers = 4\n",
        "executable": false,
        "passthru": nix_record! { "package": pkgs.get("hello") },
    };
    let file = pkgs.pkgs_function("writeTextFile").call(args);
    let generated = rusnix_nix::compile(&FunctionResult { result: file }.into_config()).unwrap();
    println!("{}", generated.source);
}
