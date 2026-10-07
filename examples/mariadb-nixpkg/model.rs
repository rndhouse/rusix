//! The four pinned release variants share the same Rust recipe.
use rusnix_ir::{
    IntoRusnixValue,
    interop::{Nixpkgs, Package},
};

#[derive(Clone, Copy, Debug)]
pub enum Release {
    V105,
    V106,
    V1011,
    V114,
}

impl Release {
    pub const ALL: [Self; 4] = [Self::V105, Self::V106, Self::V1011, Self::V114];

    pub fn attribute(self) -> &'static str {
        match self {
            Self::V105 => "mariadb_105",
            Self::V106 => "mariadb_106",
            Self::V1011 => "mariadb_1011",
            Self::V114 => "mariadb_114",
        }
    }

    pub fn version(self) -> &'static str {
        match self {
            Self::V105 => "10.5.27",
            Self::V106 => "10.6.20",
            Self::V1011 => "10.11.10",
            Self::V114 => "11.4.4",
        }
    }

    pub fn hash(self) -> &'static str {
        match self {
            Self::V105 => "sha256-76ZPpfczuCKrGmeV4evBecFgwIaQ7Ncduh6w9hru3RQ=",
            Self::V106 => "sha256-R/BavH2+uz8msx51AisbOWnbvEdSxK5wH4kgpgSJbUE=",
            Self::V1011 => "sha256-sGp0ZQuDoWqpqwmJhEgrAo51sABnSxH/KIdyxhmm8CI=",
            Self::V114 => "sha256-lvvS5uk/t+izc+6nXYW2/qV8DhEaAgkMu+/tUlmdx3s=",
        }
    }

    pub fn arguments(self) -> Arguments {
        Arguments {
            version: self.version(),
            hash: self.hash(),
            core_services: Nixpkgs::new()
                .get("darwin.apple_sdk.frameworks.CoreServices")
                .into(),
        }
    }
}

/// Pinned release inputs; the framework stays a deferred package dependency.
#[derive(IntoRusnixValue)]
pub struct Arguments {
    version: &'static str,
    hash: &'static str,
    #[rusnix(rename = "CoreServices")]
    core_services: Package,
}
