//! Optional Rust-native choices; the public Nix factory still accepts upstream booleans.
use rusnix_ir::interop::NixValue;

/// A concrete Rust choice can select at most one TLS implementation.
/// Ordinary Nix callers retain independent booleans, checked by the factory in Nix.
pub enum TlsBackend {
    /// Excludes all four TLS backends, including the default OpenSSL selection.
    Disabled,

    /// Uses OpenSSL for encrypted connections.
    OpenSsl,

    /// Uses GnuTLS for encrypted connections.
    GnuTls,

    /// Uses wolfSSL for encrypted connections.
    WolfSsl,

    /// Uses the existing Rustls FFI package; Rust does not model its internals.
    Rustls,
}

/// A small authoring surface for useful choices, not a second copy of every Nix argument.
pub struct Curl {
    /// `None` leaves the dependent TLS defaults to Nix; `Some` chooses one backend.
    pub tls: Option<TlsBackend>,
    /// Adds the two upstream HTTP/3 dependencies and their configure flags.
    pub http3: bool,
    /// Enables curl's experimental websocket protocol support.
    pub websocket: bool,
}

impl Curl {
    /// Produce explicit callPackage arguments while leaving all other defaults in Nix.
    pub fn arguments(self) -> NixValue {
        let mut fields = vec![
            ("http3Support", self.http3.into()),
            ("websocketSupport", self.websocket.into()),
        ];

        if let Some(tls) = self.tls {
            let selected = match tls {
                TlsBackend::Disabled => [false, false, false, false],
                TlsBackend::OpenSsl => [true, false, false, false],
                TlsBackend::GnuTls => [false, true, false, false],
                TlsBackend::WolfSsl => [false, false, true, false],
                TlsBackend::Rustls => [false, false, false, true],
            };
            fields.extend(
                [
                    "opensslSupport",
                    "gnutlsSupport",
                    "wolfsslSupport",
                    "rustlsSupport",
                ]
                .into_iter()
                .zip(selected.map(NixValue::from)),
            );
        }

        NixValue::record(fields)
    }
}

/// Chooses OpenSSL and HTTP/3 without changing the ordinary Nix package interface.
pub fn model() -> Curl {
    Curl {
        tls: Some(TlsBackend::OpenSsl),
        http3: true,
        websocket: false,
    }
}
