//! The ONLY Nix subprocess boundary. The caller cannot select a store or flags.
use crate::{Diagnostic, DiagnosticKind, Generated, render::quote};
use std::{
    fs, io,
    path::Path,
    process::{Command, Output},
};
use tempfile::TempDir;

pub struct NixSession {
    disposable: TempDir,
    pub(crate) full_source: std::sync::OnceLock<std::sync::Arc<crate::interop::FullSource>>,
}

#[derive(Debug)]
pub struct Evaluation {
    pub value: serde_json::Value,
    pub raw_nix: String,
}

impl NixSession {
    pub fn new() -> io::Result<Self> {
        let disposable = tempfile::Builder::new().prefix("rusnix-").tempdir()?;
        for directory in ["store", "config", "cache", "state", "home"] {
            fs::create_dir(disposable.path().join(directory))?;
        }
        Ok(Self {
            disposable,
            full_source: std::sync::OnceLock::new(),
        })
    }

    pub fn root(&self) -> &Path {
        self.disposable.path()
    }

    // Private; store selection is structural and not an optional caller flag.
    fn command(&self, parse: bool) -> Command {
        let store = self.root().join("store");
        assert!(store.is_absolute() && store.starts_with(self.root()));
        let mut command = Command::new(if parse { "nix-instantiate" } else { "nix" });
        command.arg("--store").arg(&store).args([
            "--extra-experimental-features",
            "nix-command",
            "--log-format",
            "internal-json",
            "--show-trace",
            "--option",
            "substituters",
            "",
            "--option",
            "builders",
            "",
            "--option",
            "build-users-group",
            "",
            "--option",
            "allow-import-from-derivation",
            "false",
        ]);
        // Do not inherit host store selection, remote builders, includes, daemon
        // configuration, or the user's Nix cache/state directories.
        command
            .env_remove("NIX_REMOTE")
            .env_remove("NIX_STORE_DIR")
            .env_remove("NIX_STATE_DIR")
            .env_remove("NIX_LOG_DIR")
            .env_remove("NIX_DAEMON_SOCKET_PATH")
            .env_remove("NIXOS_LABEL")
            .env_remove("NIXOS_LABEL_VERSION")
            .env("NIX_CONFIG", "")
            .env("NIX_PATH", "")
            .env("NIX_CONF_DIR", self.root().join("config"))
            .env("NIX_USER_CONF_FILES", self.root().join("config/nix.conf"))
            .env("XDG_CONFIG_HOME", self.root().join("config"))
            .env("XDG_CACHE_HOME", self.root().join("cache"))
            .env("XDG_STATE_HOME", self.root().join("state"))
            .env("XDG_DATA_HOME", self.root().join("state"))
            .env("HOME", self.root().join("home"))
            .env("NO_COLOR", "1")
            .env("TERM", "dumb");
        command
    }

    pub fn version(&self) -> io::Result<String> {
        let output = self.command(false).arg("--version").output()?;
        if !output.status.success() {
            return Err(io::Error::other(
                String::from_utf8_lossy(&output.stderr).into_owned(),
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().into())
    }

    pub fn evaluate(&self, generated: &Generated) -> Result<Evaluation, Box<Diagnostic>> {
        self.evaluate_selection(generated, None)
    }

    /// Select one top-level attribute without demanding its siblings.
    pub fn evaluate_attribute(
        &self,
        generated: &Generated,
        attribute: &str,
    ) -> Result<Evaluation, Box<Diagnostic>> {
        if attribute.contains('\0') {
            return Err(Diagnostic::tooling("NUL is not supported in attribute selection").into());
        }
        self.evaluate_selection(generated, Some(attribute))
    }

    fn evaluate_selection(
        &self,
        generated: &Generated,
        attribute: Option<&str>,
    ) -> Result<Evaluation, Box<Diagnostic>> {
        let file = self.root().join("generated.nix");
        let parse_stderr = self.validate_generated(generated, "generated.nix")?;
        let output = self.run(&file, false, attribute)?;
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        if !output.status.success() {
            return Err(Diagnostic::from_nix(
                DiagnosticKind::NixEval,
                &format!("{parse_stderr}{stderr}"),
                generated,
                &file,
            )
            .into());
        }
        let value = serde_json::from_slice(&output.stdout).map_err(|e| {
            let mut diagnostic = Diagnostic::tooling(format!("Nix returned invalid JSON: {e}"));
            diagnostic.raw_nix = stderr.clone();
            diagnostic
        })?;
        Ok(Evaluation {
            value,
            raw_nix: format!("{parse_stderr}{stderr}"),
        })
    }

    pub(crate) fn validate_generated(
        &self,
        generated: &Generated,
        name: &str,
    ) -> Result<String, Box<Diagnostic>> {
        let file = self.root().join(name);
        fs::write(&file, &generated.source).map_err(|e| Diagnostic::tooling(e.to_string()))?;
        // A true parse-only subprocess. `nix eval --apply 'x: true'` still forces
        // the file in Nix 2.34.8 and cannot distinguish syntax from user errors.
        let parse = self.run(&file, true, None)?;
        let parse_stderr = String::from_utf8_lossy(&parse.stderr);
        if !parse.status.success() {
            let diagnostic =
                Diagnostic::from_nix(DiagnosticKind::Compiler, &parse_stderr, generated, &file);
            // Recognize parser/static-binding failures narrowly. Store startup
            // or CLI failures remain tooling errors, not invented compiler bugs.
            let kind = if diagnostic.reason.contains("syntax error")
                || diagnostic.reason.contains("undefined variable")
                || diagnostic.reason.contains("already defined")
            {
                DiagnosticKind::Compiler
            } else {
                DiagnosticKind::Tooling
            };
            return Err(Diagnostic::from_nix(kind, &parse_stderr, generated, &file).into());
        }
        Ok(parse_stderr.into_owned())
    }

    fn run(
        &self,
        file: &Path,
        parse_only: bool,
        attribute: Option<&str>,
    ) -> Result<Output, Box<Diagnostic>> {
        let mut command = if parse_only {
            self.command(true)
        } else {
            self.eval_command(attribute)
        };
        if parse_only {
            command.arg("--parse").arg(file);
        } else {
            command.arg("--file").arg(file);
        }
        command
            .output()
            .map_err(|e| Diagnostic::tooling(format!("cannot execute Nix subprocess: {e}")).into())
    }

    fn eval_command(&self, attribute: Option<&str>) -> Command {
        let mut command = self.command(false);
        command
            .args(["--offline", "eval", "--json", "--eval-store"])
            .arg(self.root().join("store"));
        if let Some(attribute) = attribute {
            // Attribute names are escaped Nix string data, never CLI flags or
            // raw source. Store selection cannot be replaced by a selection.
            command.arg("--apply").arg(format!(
                "value: builtins.getAttr {} value",
                quote(attribute)
            ));
        }
        command
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    #[test]
    fn every_command_explicitly_selects_the_owned_disposable_store() {
        let session = NixSession::new().unwrap();
        for command in [session.command(true), session.command(false)] {
            let args: Vec<_> = command.get_args().collect();
            assert_eq!(args[0], "--store");
            assert_eq!(Path::new(args[1]), session.root().join("store"));
            for key in [
                "NIX_REMOTE",
                "NIX_STORE_DIR",
                "NIX_STATE_DIR",
                "NIX_DAEMON_SOCKET_PATH",
                "NIXOS_LABEL",
                "NIXOS_LABEL_VERSION",
            ] {
                assert!(
                    command
                        .get_envs()
                        .any(|(name, value)| name == key && value.is_none())
                );
            }
        }
        let command = session.eval_command(Some("good"));
        let args: Vec<_> = command.get_args().collect();
        for flag in ["--store", "--eval-store"] {
            assert_eq!(
                args.iter().filter(|arg| **arg == OsStr::new(flag)).count(),
                1
            );
            let index = args
                .iter()
                .position(|arg| *arg == OsStr::new(flag))
                .unwrap();
            let store = Path::new(args[index + 1]);
            assert_eq!(store, session.root().join("store"));
            assert!(store.is_absolute());
            assert_ne!(store, Path::new("/nix/store"));
        }
        assert!(args.windows(2).any(|pair| pair
            == [
                OsStr::new("--apply"),
                OsStr::new("value: builtins.getAttr \"good\" value")
            ]));
        assert!(args.windows(3).any(|v| v
            == [
                OsStr::new("--option"),
                OsStr::new("substituters"),
                OsStr::new("")
            ]));
        assert!(args.windows(3).any(|v| v
            == [
                OsStr::new("--option"),
                OsStr::new("allow-import-from-derivation"),
                OsStr::new("false")
            ]));
        assert!(
            command
                .get_envs()
                .any(|(key, value)| key == "NIX_CONFIG" && value == Some(OsStr::new("")))
        );
        let root = session.root().to_owned();
        drop(session);
        assert!(!root.exists());
    }
}
