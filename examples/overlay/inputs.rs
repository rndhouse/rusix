//! Declares the package and recipe fields that this overlay reads from nixpkgs.
//! Rust field names map to Nix attribute names; accessors describe deferred lookups.

/// Declares accessors for existing nixpkgs packages, builder attributes and package metadata.
/// These views describe the fields used by the overlay without supplying their implementations.
#[rusnix_ir::args]
mod views {
    use rusnix_ir::{
        Expr,
        interop::{NixList, Package},
    };

    /// Packages needed by the overlay, from either the final or preceding set.
    /// This declares lookup accessors for nixpkgs entries, not their implementations.
    #[rusnix(root)]
    struct Packages {
        /// Curl's recipe in the selected nixpkgs set, including its override methods.
        curl: Package,
    }

    /// Previous builder attributes needed to preserve curl's configure flags.
    #[rusnix(value)]
    struct BuildAttrs {
        /// Existing flags passed to curl's configure script.
        configure_flags: NixList<Expr<String>>,
    }

    /// Package metadata printed by the example without building curl.
    #[rusnix(value)]
    struct PackageMetadata {
        /// Store path of the build recipe, rather than the built package output.
        drv_path: String,
    }
}

pub use views::{BuildAttrs, PackageMetadata, Packages};
