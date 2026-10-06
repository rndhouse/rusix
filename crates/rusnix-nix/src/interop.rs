//! Opaque semantic references lower into ordinary AST imports/lookups/calls.
use crate::{
    Diagnostic, Evaluation, Generated, NixSession,
    ast::{BinaryOp, Builtin, NixExpr, NixKind},
};
use rusnix_ir::interop::{AttrPath, Reference, Source};
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex, OnceLock, Weak},
};

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
        Source::Builtins => NixExpr::plain(NixKind::Variable("builtins".into())),
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

    if matches!(reference.source, Source::Builtins) {
        // A builtin's definition location can appear in its later argument-type
        // failure. Keep lookup spans, but treat the consuming application as the
        // operation; a definition span must not override that runtime boundary.
        NixExpr::attributed(NixKind::Group(Box::new(value)), reference.origin.clone())
    } else if matches!(reference.source, Source::PinnedPath { .. }) {
        // Appending validated literal path data introduces no external evaluation.
        NixExpr::attributed(value.kind, reference.origin.clone())
    } else {
        // Imported values may fail entirely inside Nix even after a call returns.
        NixExpr::contextual(value.kind, reference.origin.clone())
    }
}

// Only the checked source checkout is shared; each NixSession still owns its
// own store/eval-store. Revalidate when no sessions retain the source handle.
pub(crate) struct FullSource {
    pub(crate) path: PathBuf,
}

fn check_source(path: &Path, revision: &str) -> Result<(), String> {
    if !path.join(".git").exists() || !path.join("default.nix").is_file() {
        return Err(
            "nixpkgs submodule is missing; run git submodule update --init --depth=1".into(),
        );
    }

    let git = |args: &[&str]| -> Result<String, String> {
        let output = Command::new("git")
            .arg("-C")
            .arg(path)
            .args(args)
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .output()
            .map_err(|e| format!("cannot verify nixpkgs submodule: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "cannot verify nixpkgs submodule: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }

        String::from_utf8(output.stdout).map_err(|e| e.to_string())
    };

    if git(&["rev-parse", "HEAD"])?.trim() != revision {
        return Err("nixpkgs submodule revision mismatch; run git submodule update --init".into());
    }
    if !git(&[
        "status",
        "--porcelain",
        "--untracked-files=all",
        "--ignored",
    ])?
    .is_empty()
    {
        return Err("nixpkgs submodule has modified or untracked files".into());
    }

    Ok(())
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

    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/nixpkgs");
    check_source(&path, crate::nixos::NIXPKGS_REVISION).map_err(Diagnostic::tooling)?;

    let source = Arc::new(FullSource { path });

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

    /// Evaluate generated Nix that uses the pinned nixpkgs packages or library.
    /// The checked local inputs are staged offline into this session’s workspace.
    /// The result must be JSON-compatible; select data from a package or function
    /// when the object itself cannot be serialized. Nix may construct build recipes,
    /// but package outputs are never built.
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

#[cfg(test)]
mod source_tests {
    use super::*;

    #[test]
    fn initialized_submodule_must_match_the_pinned_revision() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/nixpkgs");
        check_source(&path, crate::nixos::NIXPKGS_REVISION).unwrap();
        assert!(
            check_source(&path, "0000000000000000000000000000000000000000")
                .unwrap_err()
                .contains("revision mismatch")
        );
    }

    #[test]
    fn missing_checkout_has_initialization_advice() {
        let missing = tempfile::tempdir().unwrap();
        assert!(
            check_source(missing.path(), crate::nixos::NIXPKGS_REVISION)
                .unwrap_err()
                .contains("git submodule update --init --depth=1")
        );
    }

    #[test]
    fn incomplete_or_untracked_checkout_is_rejected() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/nixpkgs");
        let checkout = tempfile::tempdir().unwrap();
        // Share existing local objects without downloading or creating commits.
        let clone = Command::new("git")
            .args(["clone", "--quiet", "--shared", "--no-checkout", "--"])
            .arg(path)
            .arg(checkout.path())
            .output()
            .unwrap();
        assert!(
            clone.status.success(),
            "{}",
            String::from_utf8_lossy(&clone.stderr)
        );
        std::fs::write(checkout.path().join("default.nix"), "{}\n").unwrap();
        assert!(
            check_source(checkout.path(), crate::nixos::NIXPKGS_REVISION)
                .unwrap_err()
                .contains("modified or untracked files")
        );
    }
}
