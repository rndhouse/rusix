use crate::{Generated, SourceSpan};
use rusnix_ir::Origin;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticKind {
    Rust,
    Validation,
    NixEval,
    NixosModule,
    NixosType,
    NixosMerge,
    NixosAssertion,
    ExternalNix,
    Compiler,
    Tooling,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Provenance {
    RustValidation,
    ErrorContext,
    SourceMap,
    ModuleDefinition,
    AssertionMessage,
    ImportBoundary,
    Unavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OriginRole {
    Primary,
    ConflictingDefinition,
    ContributingDefinition,
    ImportedBoundary,
}

/// A causal source, distinct from the semantic ancestry in `related`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticOrigin {
    pub origin: Option<Origin>,
    pub role: OriginRole,
    pub provenance: Provenance,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nix_file: Option<String>,
}

/// Rusnix's own diagnostic contract. Nix's wire/text formats stop at this module.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Diagnostic {
    pub kind: DiagnosticKind,
    pub reason: String,
    pub primary: Option<Origin>,
    /// Authoritative causal set. `primary` remains a compatibility convenience.
    #[serde(default)]
    pub origins: Vec<DiagnosticOrigin>,
    pub related: Vec<Origin>,
    pub provenance: Provenance,
    pub raw_nix: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub option_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_file: Option<String>,
}

impl Diagnostic {
    pub fn validation(origin: Origin, reason: String) -> Self {
        Self {
            kind: DiagnosticKind::Validation,
            reason,
            primary: Some(origin),
            origins: vec![],
            related: vec![],
            provenance: Provenance::RustValidation,
            raw_nix: String::new(),
            option_path: None,
            external_file: None,
        }
        .with_origin_set()
    }

    pub fn tooling(reason: impl Into<String>) -> Self {
        Self {
            kind: DiagnosticKind::Tooling,
            reason: reason.into(),
            primary: None,
            origins: vec![],
            related: vec![],
            provenance: Provenance::Unavailable,
            raw_nix: String::new(),
            option_path: None,
            external_file: None,
        }
    }

    pub fn from_nix(kind: DiagnosticKind, raw: &str, generated: &Generated, file: &Path) -> Self {
        if let Some(event) = structured_error(raw) {
            return Self::from_structured(kind, raw, &event, generated, file);
        }

        let messages = decode_messages(raw);
        let text = strip_ansi(&messages.join("\n"));
        let reason = text
            .lines()
            .filter_map(|line| line.trim().strip_prefix("error:"))
            .next_back()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("Nix failed; inspect the retained original diagnostic")
            .to_owned();

        let mut related = Vec::new();
        let prefix = format!("at {}:", file.display());
        let local_spans: Vec<_> = text
            .lines()
            .rev()
            .filter_map(|line| {
                let rest = line.trim().strip_prefix(&prefix)?;
                let mut parts = rest.split(':');
                generated.span_at_position(parts.next()?.parse().ok()?, parts.next()?.parse().ok()?)
            })
            .collect();

        // Match trace-shaped lines, excluding ordinary source excerpts and
        // single-line throws. This legacy fallback is a heuristic, not a parser.
        // Trace order is outermost -> innermost on the tested Nix versions.
        for line in text.lines() {
            let trace = line
                .trim()
                .strip_prefix("… ")
                .or_else(|| line.trim().strip_prefix("... "));
            if let Some(id) = trace.and_then(|s| s.strip_prefix("rusnix-origin:"))
                && let Some(origin) = generated.origin(id.trim())
                && !related.iter().any(|o: &Origin| o.id == origin.id)
            {
                related.push(origin.clone());
            }
        }

        let (primary, provenance) =
            if kind == DiagnosticKind::Compiler || kind == DiagnosticKind::Tooling {
                // Syntax/static backend errors must never accuse a user's Rust line.
                related.clear();
                (None, Provenance::Unavailable)
            } else if let Some(origin) = related.last() {
                (Some(origin.clone()), Provenance::ErrorContext)
            } else {
                match local_spans.first() {
                    Some(span) => (Some(span.origin.clone()), Provenance::SourceMap),
                    None => (None, Provenance::Unavailable),
                }
            };

        Self {
            kind,
            reason,
            primary,
            origins: vec![],
            related,
            provenance,
            raw_nix: raw.into(),
            option_path: None,
            external_file: None,
        }
        .with_enclosing(generated, &local_spans)
    }

    fn from_structured(
        kind: DiagnosticKind,
        raw: &str,
        event: &serde_json::Value,
        generated: &Generated,
        file: &Path,
    ) -> Self {
        let reason = strip_ansi(
            event["raw_msg"]
                .as_str()
                .unwrap_or("Nix failed; inspect the retained original diagnostic"),
        );
        let frames = event["trace"].as_array().map(Vec::as_slice).unwrap_or(&[]);
        let local_spans: Vec<_> = frames
            .iter()
            .chain(std::iter::once(event))
            .filter_map(|frame| {
                let line = usize::try_from(frame["line"].as_u64()?).ok()?;
                let column = usize::try_from(frame["column"].as_u64()?).ok()?;
                let reported = frame["file"].as_str()?;
                let path = file.to_string_lossy();
                if reported != path && reported != format!("{path}:{line}:{column}") {
                    return None;
                }
                generated.span_at_position(line, column)
            })
            .collect();

        let mut related = Vec::new();

        // JSON traces are innermost first, unlike the rendered message.
        for frame in frames.iter().rev() {
            if let Some(id) = frame["raw_msg"]
                .as_str()
                .and_then(|s| s.strip_prefix("rusnix-origin:"))
                && let Some(origin) = generated.origin(id)
                && !related.iter().any(|o: &Origin| o.id == origin.id)
            {
                related.push(origin.clone());
            }
        }

        let (primary, provenance) =
            if matches!(kind, DiagnosticKind::Compiler | DiagnosticKind::Tooling) {
                related.clear();
                (None, Provenance::Unavailable)
            } else if let Some(origin) = related.last() {
                (Some(origin.clone()), Provenance::ErrorContext)
            } else {
                // Use the most local generated frame, and never map an external file.
                let origin = local_spans.first().map(|span| span.origin.clone());
                let provenance = if origin.is_some() {
                    Provenance::SourceMap
                } else {
                    Provenance::Unavailable
                };
                (origin, provenance)
            };

        Self {
            kind,
            reason,
            primary,
            origins: vec![],
            related,
            provenance,
            raw_nix: raw.into(),
            option_path: None,
            external_file: None,
        }
        .with_enclosing(generated, &local_spans)
    }

    fn with_enclosing(mut self, generated: &Generated, local_spans: &[&SourceSpan]) -> Self {
        if let Some(primary) = &self.primary {
            // Prefer a concrete generated occurrence: an Expr cloned into two
            // assignments has one origin ID but two different enclosing paths.
            let span = local_spans
                .iter()
                .copied()
                .find(|span| span.origin.id == primary.id)
                .or_else(|| {
                    generated
                        .spans
                        .iter()
                        .find(|span| span.origin.id == primary.id)
                });

            if let Some(span) = span {
                let mut related = span.enclosing.clone();
                for origin in self.related.iter().chain(std::iter::once(primary)) {
                    if !related.iter().any(|known| known.id == origin.id) {
                        related.push(origin.clone());
                    }
                }
                self.related = related;
            }
        }

        self.with_origin_set()
    }

    pub(crate) fn with_origin_set(mut self) -> Self {
        if self.origins.is_empty()
            && let Some(origin) = &self.primary
        {
            self.origins.push(DiagnosticOrigin {
                origin: Some(origin.clone()),
                role: OriginRole::Primary,
                provenance: self.provenance,
                nix_file: self.external_file.clone(),
            });
        }
        self
    }

    /// Stable snapshot surface: no Nix stack layout, temporary paths, or IDs.
    pub fn summary(&self) -> String {
        let mut out = format!("error[{}]: {}\n", self.code(), self.message());

        if self.origins.len() > 1 || (self.primary.is_none() && !self.origins.is_empty()) {
            for source in &self.origins {
                if let Some(origin) = &source.origin {
                    out.push_str(&format!(
                        "  --> {}:{}:{}\n   = origin: {}\n",
                        origin.file, origin.line, origin.column, origin.purpose
                    ));
                }
                out.push_str(&format!(
                    "   = role: {:?}; provenance: {:?}\n",
                    source.role, source.provenance
                ));
                if let Some(file) = &source.nix_file {
                    out.push_str(&format!("   = Nix definition: {file}\n"));
                }
            }
        } else if let Some(origin) = &self.primary {
            out.push_str(&format!(
                "  --> {}:{}:{}\n   = origin: {}\n",
                origin.file, origin.line, origin.column, origin.purpose
            ));
        } else if self.kind == DiagnosticKind::NixEval {
            out.push_str("   = Rust origin unavailable\n");
        }

        if let Some(path) = &self.option_path {
            out.push_str(&format!("   = option: {path}\n"));
        }

        if let Some(file) = &self.external_file {
            out.push_str(&format!("   = imported Nix: {file}\n"));
        }

        out.push_str(&format!(
            "   = {}: {}\n",
            if self.kind == DiagnosticKind::Validation {
                "Rusnix"
            } else {
                "Nix"
            },
            self.reason
        ));
        out.push_str(&format!("   = provenance: {:?}\n", self.provenance));

        out
    }

    pub fn render(&self, source_root: &Path) -> String {
        let mut out = format!("error[{}]: {}\n", self.code(), self.message());

        if self.origins.len() > 1 || (self.primary.is_none() && !self.origins.is_empty()) {
            for source in &self.origins {
                out.push('\n');
                if let Some(origin) = &source.origin {
                    render_location(&mut out, origin, source_root);
                    out.push_str(&format!("   = origin: {}\n", origin.purpose));
                }
                out.push_str(&format!(
                    "   = {}\n",
                    match source.role {
                        OriginRole::Primary => "originating operation",
                        OriginRole::ConflictingDefinition => "conflicting definition",
                        OriginRole::ContributingDefinition => "contributing definition",
                        OriginRole::ImportedBoundary => "imported module boundary",
                    }
                ));
                if let Some(file) = &source.nix_file {
                    out.push_str(&format!("   = Nix definition: {file}\n"));
                }
            }
            if let Some(path) = &self.option_path {
                out.push_str(&format!("\n   = option: {path}\n"));
            }
        } else if let Some(origin) = &self.primary {
            render_location(&mut out, origin, source_root);
            out.push_str(&format!("   = origin: {}\n", origin.purpose));
            if let Some(path) = &self.option_path {
                out.push_str(&format!("   = option: {path}\n"));
            } else if let Some(assignment) = self
                .related
                .iter()
                .rev()
                .find(|o| o.purpose.starts_with("set "))
            {
                out.push_str(&format!(
                    "   = option: {}\n",
                    assignment.purpose.trim_start_matches("set ")
                ));
            }
        } else if self.kind == DiagnosticKind::NixEval {
            out.push_str(
                "   = Rust origin unavailable; inspect generated source and raw Nix diagnostic\n",
            );
        }

        if let Some(file) = &self.external_file {
            out.push_str(&format!("   = imported Nix: {file}\n"));
        }

        out.push_str(&format!(
            "   = {}: {}\n",
            if self.kind == DiagnosticKind::Validation {
                "Rusnix"
            } else {
                "Nix"
            },
            self.reason.trim().replace('\n', "\n     ")
        ));

        out
    }

    fn code(&self) -> &'static str {
        match self.kind {
            DiagnosticKind::Rust => "rust",
            DiagnosticKind::Validation => "validation",
            DiagnosticKind::NixEval => "nix-eval",
            DiagnosticKind::NixosModule => "nixos-module",
            DiagnosticKind::NixosType => "nixos-type",
            DiagnosticKind::NixosMerge => "nixos-merge",
            DiagnosticKind::NixosAssertion => "nixos-assertion",
            DiagnosticKind::ExternalNix => "external-nix",
            DiagnosticKind::Compiler => "codegen",
            DiagnosticKind::Tooling => "nix-tooling",
        }
    }

    fn message(&self) -> &'static str {
        match self.kind {
            DiagnosticKind::Rust => "Rust rejected the configuration",
            DiagnosticKind::Validation => "invalid Rust configuration IR",
            DiagnosticKind::NixEval => "generated configuration was rejected by Nix",
            DiagnosticKind::NixosModule => "NixOS rejected the generated module",
            DiagnosticKind::NixosType => "NixOS rejected an option value",
            DiagnosticKind::NixosMerge => "conflicting definitions for a NixOS option",
            DiagnosticKind::NixosAssertion => "NixOS configuration assertion failed",
            DiagnosticKind::ExternalNix => "imported NixOS module evaluation failed",
            DiagnosticKind::Compiler => "generated Nix is invalid (Rusnix compiler bug)",
            DiagnosticKind::Tooling => "could not run the isolated Nix evaluator",
        }
    }
}

fn render_location(out: &mut String, origin: &Origin, source_root: &Path) {
    out.push_str(&format!(
        "  --> {}:{}:{}\n",
        origin.file, origin.line, origin.column
    ));
    if let Ok(source) = std::fs::read_to_string(source_root.join(&origin.file))
        && let Some(line) = source.lines().nth(origin.line.saturating_sub(1) as usize)
    {
        out.push_str(&format!(
            "   |\n{:>3} | {line}\n   | {}^\n",
            origin.line,
            " ".repeat(origin.column.saturating_sub(1) as usize)
        ));
    }
}

/// NixOS exposes definition lists only inside raw_msg, not as JSON fields.
/// Keep the pinned showDefs/mergeEqualOption message adapter at the wire boundary.
pub(crate) struct ModuleFailure {
    pub kind: DiagnosticKind,
    pub option: String,
    pub files: Vec<String>,
}

pub(crate) fn module_failure(reason: &str) -> Option<ModuleFailure> {
    let (kind, rest) = if let Some(rest) = reason.strip_prefix("A definition for option `") {
        if !rest.split_once("' ")?.1.starts_with("is not of type") {
            return None;
        }
        (DiagnosticKind::NixosType, rest)
    } else {
        let rest = reason.strip_prefix("The option `")?;
        let tail = rest.split_once("' ")?.1;
        let kind = if tail.starts_with("has conflicting definition values:") {
            DiagnosticKind::NixosMerge
        } else if tail.starts_with("does not exist.") {
            DiagnosticKind::NixosModule
        } else {
            return None;
        };
        (kind, rest)
    };

    let option = rest.split_once('\'')?.0.to_owned();
    let files = reason
        .lines()
        .filter_map(|line| {
            line.strip_prefix("- In `")?
                .split_once("':")
                .map(|(file, _)| file.to_owned())
        })
        .collect();

    Some(ModuleFailure {
        kind,
        option,
        files,
    })
}

fn structured_error(raw: &str) -> Option<serde_json::Value> {
    raw.lines()
        .filter_map(|line| {
            let event: serde_json::Value =
                serde_json::from_str(line.strip_prefix("@nix ")?).ok()?;
            (event["action"] == "msg" && event["level"] == 0 && event["raw_msg"].is_string())
                .then_some(event)
        })
        .next_back()
}

/// External frame evidence; callers do not depend on Nix's JSON schema.
pub(crate) fn nix_evidence(raw: &str) -> Vec<(Option<String>, String)> {
    if let Some(event) = structured_error(raw) {
        event["trace"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[])
            .iter()
            .map(|frame| {
                (
                    frame["file"].as_str().map(str::to_owned),
                    strip_ansi(frame["raw_msg"].as_str().unwrap_or("")),
                )
            })
            .collect()
    } else {
        vec![]
    }
}

fn decode_messages(raw: &str) -> Vec<String> {
    raw.lines()
        .filter_map(|line| {
            if let Some(json) = line.strip_prefix("@nix ") {
                match serde_json::from_str::<serde_json::Value>(json) {
                    Ok(event) => event.get("msg").and_then(|v| v.as_str()).map(str::to_owned),
                    Err(_) => Some(line.into()),
                }
            } else {
                Some(line.into())
            }
        })
        .collect()
}

fn strip_ansi(text: &str) -> String {
    let mut result = String::new();
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for code in chars.by_ref() {
                if ('@'..='~').contains(&code) {
                    break;
                }
            }
        } else {
            result.push(c);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SourceSpan;

    #[test]
    fn module_adapter_reads_reported_definition_lines_without_scanning_value_excerpts() {
        let reason = "The option `services.openssh.authorizedKeysCommandUser' has conflicting definition values:\n- In `rusnix-definition:rn-a': \"root\"\n- In `/pinned/sshd.nix': {\n    text = \"- In `innocent.nix': value\";\n}\nUse `lib.mkForce value` or `lib.mkDefault value` to change the priority on any of these definitions.";
        let failure = module_failure(reason).unwrap();
        assert_eq!(failure.kind, DiagnosticKind::NixosMerge);
        assert_eq!(failure.option, "services.openssh.authorizedKeysCommandUser");
        assert_eq!(
            failure.files,
            ["rusnix-definition:rn-a", "/pinned/sshd.nix"]
        );
        assert!(
            module_failure(
                "The option `x' returned a string mentioning ' has conflicting definition values:"
            )
            .is_none()
        );
        assert!(module_failure("unrecognized future NixOS format").is_none());
    }

    #[test]
    fn external_definition_without_rust_boundary_still_renders_its_nix_file() {
        let mut d = Diagnostic::tooling("upstream conflict");
        d.kind = DiagnosticKind::NixosMerge;
        d.origins.push(DiagnosticOrigin {
            origin: None,
            role: OriginRole::ImportedBoundary,
            provenance: Provenance::Unavailable,
            nix_file: Some("upstream/module.nix".into()),
        });
        assert!(
            d.render(Path::new("."))
                .contains("Nix definition: upstream/module.nix")
        );
    }

    #[test]
    fn maps_json_context_without_matching_a_spoofed_throw_or_excerpt() {
        let origin = Origin::new("config.rs", 12, 9, "integer division");
        let generated = Generated {
            source: "builtins.div 1 0".into(),
            spans: vec![SourceSpan {
                start: 0,
                end: 16,
                origin: origin.clone(),
                enclosing: vec![],
            }],
        };
        let msg = format!(
            "error:\n … rusnix-origin:{}\n error: division by zero",
            origin.id
        );
        let raw = format!(
            "@nix {}",
            serde_json::json!({"action":"msg", "msg": msg, "level":0})
        );
        let diagnostic = Diagnostic::from_nix(
            DiagnosticKind::NixEval,
            &raw,
            &generated,
            Path::new("/tmp/generated.nix"),
        );
        assert_eq!(diagnostic.primary, Some(origin.clone()));
        assert_eq!(diagnostic.reason, "division by zero");
        for raw in [
            format!("error: rusnix-origin:{}", origin.id),
            format!("1| # rusnix-origin:{}", origin.id),
        ] {
            assert!(
                Diagnostic::from_nix(DiagnosticKind::NixEval, &raw, &generated, Path::new("x"))
                    .primary
                    .is_none()
            );
        }
    }

    #[test]
    fn unknown_protocol_degrades_without_losing_original() {
        let raw = "@nix {broken wire format}";
        let diagnostic = Diagnostic::from_nix(
            DiagnosticKind::NixEval,
            raw,
            &Generated::default(),
            Path::new("x"),
        );
        assert_eq!(diagnostic.provenance, Provenance::Unavailable);
        assert_eq!(diagnostic.raw_nix, raw);
    }
}
