//! Minimal actual NixOS module evaluation, with definition-file provenance.
use crate::{
    Diagnostic, DiagnosticKind, DiagnosticOrigin, Evaluation, Generated, NixSession, OriginRole,
    Provenance,
    ast::{NixExpr, NixKind},
    lower_value, render,
    render::quote,
};
use rusnix_ir::{Origin, nixos::NixosModule};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fs, path::Path, sync::OnceLock};

pub const NIXPKGS_REVISION: &str = "8b27c1239e5c421a2bbc2c65d52e4a6fbf2ff296";

pub const DRIVER: &str = include_str!("nixos-driver.nix");

pub fn evaluation_source(selection: &[&str], check_assertions: bool) -> String {
    let selection = selection
        .iter()
        .map(|s| quote(s))
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "(import ./nixos-driver.nix) {{ nixpkgs = ./nixpkgs; module = ./module.nix; selection = [ {selection} ]; checkAssertions = {check_assertions}; }}\n"
    )
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Boundary {
    pub path: String,
    pub origin: Origin,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NixosArtifact {
    pub module: Generated,
    pub definitions: Vec<Boundary>,
    pub assertions: Vec<Boundary>,
    pub imports: Vec<Boundary>,
}

fn string(value: impl Into<String>) -> NixExpr {
    NixExpr::plain(NixKind::String(value.into()))
}

fn attrs(bindings: Vec<(&str, NixExpr)>) -> NixExpr {
    NixExpr::plain(NixKind::AttrSet(
        bindings
            .into_iter()
            .map(|(key, value)| (vec![key.into()], value))
            .collect(),
    ))
}

pub fn compile_module(module: &NixosModule) -> Result<NixosArtifact, Box<Diagnostic>> {
    let (ast, mut artifact) = lower_module(module)?;
    artifact.module = render(&ast);
    Ok(artifact)
}

fn lower_module(module: &NixosModule) -> Result<(NixExpr, NixosArtifact), Box<Diagnostic>> {
    module
        .config
        .validate()
        .map_err(|error| Diagnostic::validation(error.origin, error.message))?;
    let mut imports = Vec::new();
    for imported in &module.imports {
        if imported.path.is_empty()
            || imported.path.contains('\0')
            || Path::new(&imported.path)
                .components()
                .any(|c| !matches!(c, std::path::Component::Normal(_)))
        {
            return Err(Diagnostic::validation(
                imported.origin.clone(),
                "module paths must be relative, nonempty paths without parent traversal".into(),
            )
            .into());
        }
        // Paths are structured data, never injected Nix source. A string suffix
        // also permits spaces and other characters that aren't Nix path tokens.
        imports.push(NixExpr::attributed(
            NixKind::Binary(
                crate::ast::BinaryOp::Add,
                Box::new(NixExpr::plain(NixKind::Path("./nixpkgs".into()))),
                Box::new(string(format!("/{}", imported.path))),
            ),
            imported.origin.clone(),
        ));
    }
    for (module_ref, origin) in &module.opaque_imports {
        module_ref
            .reference()
            .validate()
            .map_err(|e| Diagnostic::validation(e.origin, e.message))?;
        imports.push(crate::interop::module(module_ref.reference(), origin));
    }
    for assignment in &module.config.assignments {
        let config = NixExpr::plain(NixKind::AttrSet(vec![(
            assignment.path_segments().to_vec(),
            match module.priority.override_priority() {
                None => lower_value(&assignment.value),
                Some(priority) => attrs(vec![
                    ("_type", string("override")),
                    (
                        "priority",
                        NixExpr::plain(NixKind::Int(i64::from(priority))),
                    ),
                    ("content", lower_value(&assignment.value)),
                ]),
            },
        )]));
        // One module per definition makes even unknown-option diagnostics carry
        // the precise introducing operation, without demanding the option value.
        let body = attrs(vec![
            (
                "_file",
                string(format!("rusnix-definition:{}", assignment.origin.id)),
            ),
            ("config", config),
        ]);
        imports.push(NixExpr::attributed(body.kind, assignment.origin.clone()));
    }
    for assertion in &module.assertions {
        if assertion.name.is_empty()
            || assertion.name.contains('\0')
            || assertion.message.contains('\0')
        {
            return Err(Diagnostic::validation(
                assertion.origin.clone(),
                "assertion name must be nonempty and assertion strings must not contain NUL".into(),
            )
            .into());
        }
        let value = attrs(vec![
            ("assertion", lower_value(&assertion.condition)),
            (
                "message",
                string(format!(
                    "[rusnix-assertion:{}] {}",
                    assertion.origin.id, assertion.message
                )),
            ),
        ]);
        let config = attrs(vec![(
            "assertions",
            NixExpr::plain(NixKind::List(vec![value])),
        )]);
        let body = attrs(vec![
            (
                "_file",
                string(format!("rusnix-assertion:{}", assertion.origin.id)),
            ),
            ("config", config),
        ]);
        imports.push(NixExpr::attributed(body.kind, assertion.origin.clone()));
    }
    // Each child validates independently. Duplicate paths across modules belong
    // to NixOS's merge semantics, not the single-Config duplicate-path check.
    let mut children = Vec::new();
    for child in &module.modules {
        let (ast, artifact) = lower_module(child)?;
        imports.push(ast);
        children.push(artifact);
    }
    let body = attrs(vec![("imports", NixExpr::plain(NixKind::List(imports)))]);
    let needs_config = module
        .config
        .assignments
        .iter()
        .any(|a| crate::option_reference(&a.value).is_some())
        || module
            .assertions
            .iter()
            .any(|a| crate::option_reference(&a.condition).is_some());
    let kind = if needs_config {
        let mut arguments = vec!["config".into()];
        if module
            .config
            .assignments
            .iter()
            .any(|a| crate::module_package_reference(&a.value).is_some())
            || module
                .assertions
                .iter()
                .any(|a| crate::module_package_reference(&a.condition).is_some())
        {
            arguments.push("pkgs".into());
        }
        NixKind::Function(arguments, Box::new(body))
    } else {
        body.kind
    };
    let ast = NixExpr::attributed(kind, module.config.origin.clone());
    let mut artifact = NixosArtifact {
        module: Generated::default(),
        definitions: module
            .config
            .assignments
            .iter()
            .map(|a| Boundary {
                path: a.path.clone(),
                origin: a.origin.clone(),
                file: None,
            })
            .collect(),
        assertions: module
            .assertions
            .iter()
            .map(|a| Boundary {
                path: format!("assertions.{}", a.name),
                origin: a.origin.clone(),
                file: None,
            })
            .collect(),
        imports: module
            .imports
            .iter()
            .map(|i| Boundary {
                path: i.path.clone(),
                origin: i.origin.clone(),
                file: None,
            })
            .collect(),
    };
    for (module_ref, origin) in &module.opaque_imports {
        use rusnix_ir::interop::Source;
        let (path, file) = match &module_ref.reference().source {
            Source::ModuleFile { path } => (
                format!("nixos/modules/{path}"),
                Some(format!("nixpkgs-full/nixos/modules/{path}")),
            ),
            Source::Input { file, .. } => {
                let path = std::path::absolute(file)
                    .expect("configuration working directory")
                    .to_string_lossy()
                    .into_owned();
                (path.clone(), Some(path))
            }
            _ => unreachable!("opaque module constructors use module files or inputs"),
        };
        artifact.imports.push(Boundary {
            path,
            file,
            origin: origin.clone(),
        });
    }
    for child in children {
        artifact.definitions.extend(child.definitions);
        artifact.assertions.extend(child.assertions);
        artifact.imports.extend(child.imports);
    }
    Ok((ast, artifact))
}

#[derive(Deserialize)]
struct Pin {
    revision: String,
    files: std::collections::BTreeMap<String, String>,
}

type PinnedFiles = Vec<(String, Vec<u8>)>;

fn pinned_files() -> Result<&'static PinnedFiles, String> {
    static SNAPSHOT: OnceLock<Result<PinnedFiles, String>> = OnceLock::new();
    SNAPSHOT
        .get_or_init(|| {
            let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/nixpkgs");
            let pin: Pin = serde_json::from_slice(
                &fs::read(root.join("PIN.json")).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            if pin.revision != NIXPKGS_REVISION {
                return Err("nixpkgs pin revision mismatch".into());
            }
            pin.files
                .into_iter()
                .map(|(name, hash)| {
                    if Path::new(&name)
                        .components()
                        .any(|c| !matches!(c, std::path::Component::Normal(_)))
                    {
                        return Err("invalid snapshot path".into());
                    }
                    let bytes = fs::read(root.join(&name))
                        .map_err(|e| format!("missing pinned {name}: {e}"))?;
                    if format!("{:x}", Sha256::digest(&bytes)) != hash {
                        return Err(format!("pinned nixpkgs hash mismatch: {name}"));
                    }
                    Ok((name, bytes))
                })
                .collect()
        })
        .as_ref()
        .map_err(Clone::clone)
}

impl NixSession {
    pub(crate) fn stage_pinned(&self) -> Result<std::path::PathBuf, Box<Diagnostic>> {
        let files = pinned_files().map_err(Diagnostic::tooling)?;
        let pin_root = self.root().join("nixpkgs");
        for (name, bytes) in files {
            let path = pin_root.join(name);
            fs::create_dir_all(path.parent().unwrap())
                .map_err(|e| Diagnostic::tooling(e.to_string()))?;
            fs::write(path, bytes).map_err(|e| Diagnostic::tooling(e.to_string()))?;
        }
        Ok(pin_root)
    }

    /// Run a caller-owned evaluation/projection driver against module.nix.
    /// Drivers are backend/test infrastructure, not Rust authoring expressions.
    /// The generated module is separately parsed and diagnostics use its origins.
    pub fn evaluate_nixos_with_driver(
        &self,
        artifact: &NixosArtifact,
        driver: &Generated,
    ) -> Result<Evaluation, Box<Diagnostic>> {
        let pin_root = self.stage_pinned()?;
        self.stage_interop()?;
        let parse_stderr = self.validate_generated(&artifact.module, "module.nix")?;
        self.evaluate(driver).map_err(|error| {
            if error.kind != DiagnosticKind::NixEval {
                return error;
            }
            let diagnostic = Diagnostic::from_nix(
                DiagnosticKind::NixEval,
                &format!("{parse_stderr}{}", error.raw_nix),
                &artifact.module,
                &self.root().join("module.nix"),
            );
            Box::new(translate(diagnostic, artifact, &pin_root))
        })
    }

    pub fn evaluate_nixos(
        &self,
        artifact: &NixosArtifact,
        selection: &[&str],
        check_assertions: bool,
    ) -> Result<Evaluation, Box<Diagnostic>> {
        self.evaluate_nixos_mode(artifact, selection, check_assertions, false, false)
    }

    pub fn evaluate_nixos_interop(
        &self,
        artifact: &NixosArtifact,
        selection: &[&str],
        check_assertions: bool,
    ) -> Result<Evaluation, Box<Diagnostic>> {
        self.evaluate_nixos_mode(artifact, selection, check_assertions, true, false)
    }

    /// Demand the real package option, but serialize only metadata, never outPath
    /// or drvPath. Nix may create derivation records; no outputs are built.
    pub fn evaluate_system_packages(
        &self,
        artifact: &NixosArtifact,
    ) -> Result<Evaluation, Box<Diagnostic>> {
        self.evaluate_nixos_mode(
            artifact,
            &["environment", "systemPackages"],
            false,
            true,
            true,
        )
    }

    fn evaluate_nixos_mode(
        &self,
        artifact: &NixosArtifact,
        selection: &[&str],
        check_assertions: bool,
        interop: bool,
        summaries: bool,
    ) -> Result<Evaluation, Box<Diagnostic>> {
        let pin_root = self.stage_pinned()?;
        if interop {
            self.stage_interop()?;
        }
        let parse_stderr = self.validate_generated(&artifact.module, "module.nix")?;
        fs::write(self.root().join("nixos-driver.nix"), DRIVER)
            .map_err(|e| Diagnostic::tooling(e.to_string()))?;
        if selection.iter().any(|s| s.contains('\0')) {
            return Err(Diagnostic::tooling("NUL in NixOS selection").into());
        }
        let source = if interop {
            let pkgs = crate::render(&crate::interop::source(
                &rusnix_ir::interop::Source::Packages { overlays: vec![] },
            ))
            .source;
            format!(
                "(import ./nixos-driver.nix) {{ nixpkgs = ./nixpkgs; module = ./module.nix; selection = [ {} ]; checkAssertions = {check_assertions}; pkgs = {pkgs}; packageSummary = {summaries}; }}\n",
                selection
                    .iter()
                    .map(|s| quote(s))
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        } else {
            evaluation_source(selection, check_assertions)
        };
        let wrapper = Generated {
            source,
            ..Generated::default()
        };
        self.evaluate(&wrapper)
            .map(|mut evaluation| {
                evaluation.raw_nix = format!("{parse_stderr}{}", evaluation.raw_nix);
                evaluation
            })
            .map_err(|error| {
                if error.kind != DiagnosticKind::NixEval {
                    return error;
                }
                let diagnostic = Diagnostic::from_nix(
                    DiagnosticKind::NixEval,
                    &format!("{parse_stderr}{}", error.raw_nix),
                    &artifact.module,
                    &self.root().join("module.nix"),
                );
                Box::new(translate(diagnostic, artifact, &pin_root))
            })
    }
}

fn boundary_file(boundary: &Boundary, pin_root: &Path) -> std::path::PathBuf {
    match &boundary.file {
        Some(file) => pin_root.parent().unwrap().join(file),
        None => pin_root.join(&boundary.path),
    }
}

fn matches_file(reported: &str, path: &Path) -> bool {
    [
        path.to_owned(),
        path.canonicalize().unwrap_or_else(|_| path.to_owned()),
    ]
    .iter()
    .any(|path| {
        let prefix = path.to_string_lossy();
        reported == prefix || reported.starts_with(&format!("{prefix}:"))
    })
}

fn translate(mut diagnostic: Diagnostic, artifact: &NixosArtifact, pin_root: &Path) -> Diagnostic {
    let evidence = crate::diagnostic::nix_evidence(&diagnostic.raw_nix);
    let in_module_system = evidence.iter().any(|(file, _)| {
        file.as_deref().is_some_and(|file| {
            [
                pin_root.to_owned(),
                pin_root.parent().unwrap().join("nixpkgs-full"),
            ]
            .iter()
            .any(|root| matches_file(file, &root.join("lib/modules.nix")))
        })
    });
    if in_module_system && let Some(failure) = crate::diagnostic::module_failure(&diagnostic.reason)
    {
        diagnostic.kind = failure.kind;
        diagnostic.option_path = Some(failure.option.clone());
        let mut origins = Vec::new();
        for file in &failure.files {
            let source = if let Some(id) = file.strip_prefix("rusnix-definition:") {
                artifact
                    .definitions
                    .iter()
                    .find(|b| b.origin.id == id)
                    .map(|boundary| {
                        // Keep precise NixOS paths within opaque records. Only
                        // synthetic list-definition suffixes need the IR path.
                        if !failure.option.starts_with(&format!("{}.", boundary.path))
                            || failure.option.contains("[definition ")
                        {
                            diagnostic.option_path = Some(boundary.path.clone());
                        }
                        DiagnosticOrigin {
                            origin: Some(boundary.origin.clone()),
                            role: if failure.kind == DiagnosticKind::NixosMerge {
                                OriginRole::ConflictingDefinition
                            } else {
                                OriginRole::ContributingDefinition
                            },
                            provenance: Provenance::ModuleDefinition,
                            nix_file: None,
                        }
                    })
            } else {
                let relative = Path::new(file)
                    .strip_prefix(pin_root)
                    .ok()
                    .map(|p| p.to_string_lossy().into_owned());
                // Retain an upstream file even when no directly imported Rust
                // boundary matches it. Never map it as a generated source span.
                let boundary = artifact
                    .imports
                    .iter()
                    .find(|b| matches_file(file, &boundary_file(b, pin_root)));
                Some(DiagnosticOrigin {
                    origin: boundary.map(|b| b.origin.clone()),
                    role: OriginRole::ImportedBoundary,
                    provenance: if boundary.is_some() {
                        Provenance::ImportBoundary
                    } else {
                        Provenance::Unavailable
                    },
                    nix_file: Some(
                        boundary
                            .map(|b| b.path.clone())
                            .or(relative)
                            .unwrap_or_else(|| file.clone()),
                    ),
                })
            };
            if let Some(source) = source
                && !origins.contains(&source)
            {
                origins.push(source);
            }
        }
        if !origins.is_empty() {
            let first = origins
                .iter()
                .find(|o| o.provenance == Provenance::ModuleDefinition)
                .or_else(|| origins.iter().find(|o| o.origin.is_some()));
            diagnostic.primary = first.and_then(|o| o.origin.clone());
            diagnostic.provenance = first.map_or(Provenance::Unavailable, |o| o.provenance);
            diagnostic.related = origins.iter().filter_map(|o| o.origin.clone()).collect();
            diagnostic.origins = origins;
        }
    }
    for boundary in &artifact.definitions {
        let marker = format!("rusnix-definition:{}", boundary.origin.id);
        diagnostic.reason = diagnostic.reason.replace(
            &marker,
            &format!(
                "Rust {}:{}:{}",
                boundary.origin.file, boundary.origin.line, boundary.origin.column
            ),
        );
    }
    if evidence
        .iter()
        .any(|(_, message)| message == "rusnix-stage:nixos-assertions")
    {
        diagnostic.kind = DiagnosticKind::NixosAssertion;
        let mut origins = Vec::new();
        for boundary in &artifact.assertions {
            let marker = format!("[rusnix-assertion:{}]", boundary.origin.id);
            if diagnostic.reason.contains(&marker) {
                origins.push(DiagnosticOrigin {
                    origin: Some(boundary.origin.clone()),
                    role: OriginRole::ContributingDefinition,
                    provenance: Provenance::AssertionMessage,
                    nix_file: None,
                });
                if origins.len() == 1 {
                    diagnostic.primary = Some(boundary.origin.clone());
                    diagnostic.option_path = Some(boundary.path.clone());
                    diagnostic.provenance = Provenance::AssertionMessage;
                }
                diagnostic.reason = diagnostic.reason.replace(&format!("{marker} "), "");
            }
        }
        if !origins.is_empty() {
            if origins.len() > 1 {
                diagnostic.option_path = Some("assertions".into());
            }
            diagnostic.related = origins.iter().filter_map(|o| o.origin.clone()).collect();
            diagnostic.origins = origins;
        }
    }
    if diagnostic.primary.is_none() {
        for (file, _) in &evidence {
            for boundary in &artifact.imports {
                let external = boundary_file(boundary, pin_root);
                if file
                    .as_deref()
                    .is_some_and(|file| matches_file(file, &external))
                {
                    diagnostic.kind = DiagnosticKind::ExternalNix;
                    diagnostic.primary = Some(boundary.origin.clone());
                    diagnostic.external_file = Some(boundary.path.clone());
                    diagnostic.provenance = Provenance::ImportBoundary;
                    diagnostic.origins = vec![DiagnosticOrigin {
                        origin: Some(boundary.origin.clone()),
                        role: OriginRole::ImportedBoundary,
                        provenance: Provenance::ImportBoundary,
                        nix_file: Some(boundary.path.clone()),
                    }];
                    break;
                }
            }
            if diagnostic.primary.is_some() {
                break;
            }
        }
    }
    if diagnostic.option_path.is_none() {
        diagnostic.option_path = diagnostic
            .related
            .iter()
            .rev()
            .find_map(|o| o.purpose.strip_prefix("set ").map(str::to_owned));
    }
    if diagnostic.option_path.is_none() {
        diagnostic.option_path = evidence.iter().find_map(|(_, message)| {
            message
                .strip_prefix("while evaluating the option `")
                .and_then(|s| s.split_once('\''))
                .map(|(path, _)| path.to_owned())
        });
    }
    // Remove disposable staging paths from display only. The exact original
    // diagnostic remains in raw_nix, including all external trace positions.
    diagnostic.reason = diagnostic
        .reason
        .replace(&format!("{}/", pin_root.display()), "");
    diagnostic.with_origin_set()
}
