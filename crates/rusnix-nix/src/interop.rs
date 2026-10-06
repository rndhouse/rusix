//! Opaque semantic references lower into ordinary AST imports/lookups/calls.
use crate::{
    Diagnostic, Evaluation, Generated, NixSession,
    ast::{BinaryOp, Builtin, NixExpr, NixKind},
};
use rusnix_ir::interop::{AttrPath, Reference, Source};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock, Weak},
};
use tempfile::TempDir;

fn string(value: impl Into<String>) -> NixExpr {
    NixExpr::plain(NixKind::String(value.into()))
}

pub(crate) fn select(mut root: NixExpr, path: &AttrPath) -> NixExpr {
    for segment in path.parts() {
        root = NixExpr::plain(NixKind::Call(Builtin::GetAttr, vec![string(segment), root]));
    }
    root
}

pub(crate) fn source(source: &Source) -> NixExpr {
    let imported = |path| NixExpr::plain(NixKind::Call(Builtin::Import, vec![path]));

    match source {
        Source::Packages { overlays } => NixExpr::plain(NixKind::Apply(
            Box::new(imported(NixExpr::plain(NixKind::Path(
                "./nixpkgs-full".into(),
            )))),
            Box::new(NixExpr::plain(NixKind::AttrSet(vec![
                (vec!["system".into()], string("x86_64-linux")),
                (
                    vec!["config".into()],
                    NixExpr::plain(NixKind::AttrSet(vec![])),
                ),
                (
                    vec!["overlays".into()],
                    NixExpr::plain(NixKind::List(
                        overlays.iter().map(lower_reference).collect(),
                    )),
                ),
            ]))),
        )),
        Source::NixosPackages { overlays } => overlays.iter().fold(
            NixExpr::plain(NixKind::Variable("pkgs".into())),
            |pkgs, overlay| {
                NixExpr::plain(NixKind::Apply(
                    Box::new(select(pkgs, &AttrPath::dotted("extend"))),
                    Box::new(lower_reference(overlay)),
                ))
            },
        ),
        Source::Library => imported(NixExpr::plain(NixKind::Path("./nixpkgs/lib".into()))),
        Source::PinnedPath { path } => NixExpr::plain(NixKind::Binary(
            BinaryOp::Add,
            Box::new(NixExpr::plain(NixKind::Path("./nixpkgs-full".into()))),
            Box::new(string(format!("/{path}"))),
        )),
        Source::ModuleFile { path } => {
            let path = NixExpr::plain(NixKind::Binary(
                BinaryOp::Add,
                Box::new(NixExpr::plain(NixKind::Path("./nixpkgs-full".into()))),
                Box::new(string(format!("/nixos/modules/{path}"))),
            ));
            imported(path)
        }
        Source::Input { file, .. } => imported(NixExpr::plain(NixKind::Call(
            Builtin::ToPath,
            vec![string(
                std::path::absolute(file)
                    .expect("configuration working directory")
                    .to_string_lossy(),
            )],
        ))),
    }
}

pub(crate) fn lower_reference(reference: &Reference) -> NixExpr {
    let root = source(&reference.source);
    let value = match &reference.path {
        Some(path) => select(root, path),
        None => root,
    };

    if matches!(reference.source, Source::PinnedPath { .. }) {
        // Appending validated literal path data introduces no external evaluation.
        NixExpr::attributed(value.kind, reference.origin.clone())
    } else {
        // Imported values may fail entirely inside Nix even after a call returns.
        NixExpr::contextual(value.kind, reference.origin.clone())
    }
}

// Only ordinary source files are shared; each NixSession still owns its own
// store/eval-store. Weak caching lets the last session remove the expanded tree.
pub(crate) struct FullSource {
    _temporary: TempDir,
    pub(crate) path: PathBuf,
}

pub(crate) fn full_source() -> Result<Arc<FullSource>, Box<Diagnostic>> {
    static CACHE: OnceLock<Mutex<Weak<FullSource>>> = OnceLock::new();

    let mut cache = CACHE
        .get_or_init(|| Mutex::new(Weak::new()))
        .lock()
        .map_err(|e| Diagnostic::tooling(e.to_string()))?;

    if let Some(source) = cache.upgrade() {
        return Ok(source);
    }

    let archive = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/nixpkgs-source.tar.gz");
    let bytes = fs::read(archive).map_err(|e| Diagnostic::tooling(e.to_string()))?;
    if format!("{:x}", Sha256::digest(&bytes))
        != "b4e794d1b935c1960e95526db7ed394f886064b4d974684dbc4a59d4012e1028"
    {
        return Err(Diagnostic::tooling("full nixpkgs archive hash mismatch").into());
    }

    let temporary = tempfile::Builder::new()
        .prefix("rusnix-source-")
        .tempdir()
        .map_err(|e| Diagnostic::tooling(e.to_string()))?;
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(bytes.as_slice()));
    tar.unpack(temporary.path())
        .map_err(|e| Diagnostic::tooling(e.to_string()))?;
    let path = temporary
        .path()
        .join(format!("nixpkgs-{}", crate::nixos::NIXPKGS_REVISION));
    if !path.join("default.nix").is_file() {
        return Err(Diagnostic::tooling("full nixpkgs archive revision/root mismatch").into());
    }

    let source = Arc::new(FullSource {
        _temporary: temporary,
        path,
    });

    *cache = Arc::downgrade(&source);
    Ok(source)
}

impl NixSession {
    pub(crate) fn stage_interop(&self) -> Result<(), Box<Diagnostic>> {
        self.stage_pinned()?;
        let source = if let Some(source) = self.full_source.get() {
            source.clone()
        } else {
            let source = full_source()?;
            let _ = self.full_source.set(source.clone());
            source
        };

        let link = self.root().join("nixpkgs-full");
        if !link.exists() {
            std::os::unix::fs::symlink(&source.path, link)
                .map_err(|e| Diagnostic::tooling(e.to_string()))?;
        }

        Ok(())
    }

    /// Stage pinned library/package inputs offline, then evaluate a generic JSON result.
    /// Native objects remain in Nix; select serializable attributes when functions
    /// or packages cannot cross JSON directly. No package outputs are built.
    pub fn evaluate_interop(&self, generated: &Generated) -> Result<Evaluation, Box<Diagnostic>> {
        self.stage_interop()?;
        self.evaluate(generated)
    }
}

// Check only the broad module category with the upstream predicate. This forces
// the handle's head, not its option values. Metadata survives NixOS merge errors.
pub(crate) fn module(reference: &Reference, origin: &rusnix_ir::Origin) -> NixExpr {
    let variable = || NixExpr::plain(NixKind::Variable("__rusnix_module".into()));
    let check = select(
        source(&Source::Library),
        &AttrPath::dotted("types.deferredModule.check"),
    );

    let checked = NixExpr::contextual(
        NixKind::Let(
            "__rusnix_module".into(),
            Box::new(lower_reference(reference)),
            Box::new(NixExpr::plain(NixKind::If(
                Box::new(NixExpr::plain(NixKind::Apply(
                    Box::new(check),
                    Box::new(variable()),
                ))),
                Box::new(variable()),
                Box::new(NixExpr::plain(NixKind::Call(
                    Builtin::Throw,
                    vec![string(
                        "opaque module handle did not resolve to a NixOS module",
                    )],
                ))),
            ))),
        ),
        origin.clone(),
    );

    let file = match &reference.source {
        Source::ModuleFile { path } => NixExpr::plain(NixKind::Call(
            Builtin::ToString,
            vec![NixExpr::plain(NixKind::Binary(
                BinaryOp::Add,
                Box::new(NixExpr::plain(NixKind::Path("./nixpkgs-full".into()))),
                Box::new(string(format!("/nixos/modules/{path}"))),
            ))],
        )),
        Source::Input { file, .. } => string(
            std::path::absolute(file)
                .expect("working directory")
                .to_string_lossy(),
        ),
        _ => unreachable!("module handle constructors identify file or local input"),
    };

    NixExpr::attributed(
        NixKind::AttrSet(vec![
            (vec!["_file".into()], file),
            (
                vec!["imports".into()],
                NixExpr::plain(NixKind::List(vec![checked])),
            ),
        ]),
        origin.clone(),
    )
}
