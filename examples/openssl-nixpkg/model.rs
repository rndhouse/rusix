//! Upstream release policy, expressed once in ordinary Rust.
#[derive(Clone, Copy, Debug)]
pub enum Release {
    Legacy,
    Lts,
    Preview,
}

impl Release {
    pub const ALL: [Self; 3] = [Self::Legacy, Self::Lts, Self::Preview];

    pub fn attribute(self) -> &'static str {
        match self {
            Self::Legacy => "openssl_1_1",
            Self::Lts => "openssl_3",
            Self::Preview => "openssl_3_3",
        }
    }

    pub fn version(self) -> &'static str {
        match self {
            Self::Legacy => "1.1.1w",
            Self::Lts => "3.0.15",
            Self::Preview => "3.3.2",
        }
    }

    pub fn hash(self) -> &'static str {
        match self {
            Self::Legacy => "sha256-zzCYlQy02FOtlcCEHx+cbT3BAtzPys1SHZOSUgi3asg=",
            Self::Lts => "sha256-I8Zm0O3yDxQkmz2PA2isrumrWFsJ4d6CEHxm4fPslTM=",
            Self::Preview => "sha256-LopAsBl5r+i+C7+z3l3BxnCf7bRtbInBDaEUq1/D0oE=",
        }
    }
}
