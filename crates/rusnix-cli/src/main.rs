#[path = "../../../tests/support/nixos.rs"]
mod fixture_support;

mod fixtures;

mod merge_fixtures;

mod nixos_fixtures;

use rusnix_nix::{Diagnostic, Evaluation, Generated, NixSession, compile};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let args: Vec<_> = env::args().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "check-nixos") {
        return run_nixos(&args);
    }

    if args.as_slice() == ["version"] {
        let session = NixSession::new().map_err(|e| e.to_string())?;
        println!("{}", session.version().map_err(|e| e.to_string())?);
        return Ok(());
    }

    if !matches!(args.len(), 4 | 6)
        || !matches!(args[0].as_str(), "emit" | "check")
        || args[2] != "--out"
        || (args.len() == 6 && (args[0] != "check" || args[4] != "--select"))
    {
        return Err("usage: rusnix-cli <emit|check> <good|bad-port|nested|conflict|selective|codegen-bug|unmapped> --out <artifact-directory> [--select <attribute>]\n       rusnix-cli version".into());
    }

    let generated = match args[1].as_str() {
        // Fault injections live in the CLI harness, never the configuration API.
        "codegen-bug" => Ok(Generated {
            source: "{ broken = ; }\n".into(),
            spans: vec![],
        }),
        "unmapped" => Ok(Generated {
            source: "builtins.throw \"backend failure without metadata\"\n".into(),
            spans: vec![],
        }),
        name => compile(&fixtures::config(name).ok_or_else(|| format!("unknown fixture: {name}"))?),
    };
    let out = PathBuf::from(&args[3]);
    prepare_output(&out)?;
    let generated = retain_compilation(&out, generated)?;

    fs::write(out.join("generated.nix"), &generated.source).map_err(|e| e.to_string())?;
    write_json(&out.join("source-map.json"), &generated)?;

    if args[0] == "emit" {
        println!("{}", out.join("generated.nix").display());
        return Ok(());
    }

    let session = NixSession::new().map_err(|e| e.to_string())?;
    let result = if args.len() == 6 {
        session.evaluate_attribute(&generated, &args[5])
    } else {
        session.evaluate(&generated)
    };

    retain_evaluation(&out, result)
}

fn run_nixos(args: &[String]) -> Result<(), String> {
    use rusnix_nix::nixos::{DRIVER, compile_module, evaluation_source};

    if !matches!(args.len(), 4 | 6)
        || args[2] != "--out"
        || (args.len() == 6 && args[4] != "--select")
    {
        return Err("usage: rusnix-cli check-nixos <good|type|unknown|assertion|external|lazy|merge-two|merge-three|merge-ok|merge-mixed|merge-priority|merge-three-type> --out <dir> [--select option.path]".into());
    }

    let module = nixos_fixtures::module(&args[1])
        .or_else(|| merge_fixtures::module(&args[1]))
        .ok_or_else(|| format!("unknown NixOS fixture: {}", args[1]))?;

    let out = PathBuf::from(&args[3]);
    prepare_output(&out)?;
    let artifact = retain_compilation(&out, compile_module(&module))?;
    fs::write(out.join("module.nix"), &artifact.module.source).map_err(|e| e.to_string())?;
    write_json(&out.join("module-map.json"), &artifact)?;
    fs::write(out.join("nixos-driver.nix"), DRIVER).map_err(|e| e.to_string())?;

    let selection: Vec<_> = if args.len() == 6 {
        args[5].split('.').collect()
    } else {
        if args[1].starts_with("merge-") {
            merge_fixtures::selection(&args[1]).to_vec()
        } else {
            nixos_fixtures::selection(&args[1]).to_vec()
        }
    };

    let assertions = args[1] == "assertion";
    fs::write(
        out.join("evaluation.nix"),
        evaluation_source(&selection, assertions),
    )
    .map_err(|e| e.to_string())?;

    let session = NixSession::new().map_err(|e| e.to_string())?;

    retain_evaluation(
        &out,
        session.evaluate_nixos(&artifact, &selection, assertions),
    )
}

fn prepare_output(out: &Path) -> Result<(), String> {
    fs::create_dir_all(out).map_err(|e| e.to_string())?;

    // Both commands own this artifact set. Reusing a directory across modes must
    // not retain sources, maps or results from a previous compilation.
    for name in [
        "generated.nix",
        "source-map.json",
        "module.nix",
        "module-map.json",
        "nixos-driver.nix",
        "evaluation.nix",
        "value.json",
        "diagnostic.json",
        "diagnostic.txt",
        "nix.stderr",
    ] {
        match fs::remove_file(out.join(name)) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(())
}

fn retain_compilation<T>(out: &Path, result: Result<T, Box<Diagnostic>>) -> Result<T, String> {
    match result {
        Ok(artifact) => Ok(artifact),
        Err(diagnostic) => {
            save_diagnostic(out, &diagnostic)?;
            Err(diagnostic.render(Path::new(".")))
        }
    }
}

fn retain_evaluation(
    out: &Path,
    result: Result<Evaluation, Box<Diagnostic>>,
) -> Result<(), String> {
    match result {
        Ok(evaluation) => {
            write_json(&out.join("value.json"), &evaluation.value)?;
            fs::write(out.join("nix.stderr"), evaluation.raw_nix).map_err(|e| e.to_string())?;
            println!(
                "{}",
                serde_json::to_string_pretty(&evaluation.value).map_err(|e| e.to_string())?
            );
            Ok(())
        }
        Err(diagnostic) => {
            save_diagnostic(out, &diagnostic)?;
            Err(format!(
                "{}\nOriginal Nix diagnostic: {}",
                diagnostic.render(Path::new(".")),
                out.join("nix.stderr").display()
            ))
        }
    }
}

fn write_json(path: &Path, value: &impl serde::Serialize) -> Result<(), String> {
    let json = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    fs::write(path, format!("{json}\n")).map_err(|e| e.to_string())
}

fn save_diagnostic(out: &Path, diagnostic: &Diagnostic) -> Result<(), String> {
    write_json(&out.join("diagnostic.json"), diagnostic)?;
    fs::write(out.join("nix.stderr"), &diagnostic.raw_nix).map_err(|e| e.to_string())?;
    fs::write(
        out.join("diagnostic.txt"),
        diagnostic.render(Path::new(".")),
    )
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusnix_ir::{Config, nixos::NixosModule};
    use rusnix_nix::nixos::compile_module;

    #[test]
    fn module_compilation_failures_replace_stale_artifacts_with_diagnostics() {
        let out = tempfile::tempdir().unwrap();
        for name in ["module.nix", "module-map.json", "value.json"] {
            fs::write(out.path().join(name), "stale").unwrap();
        }
        prepare_output(out.path()).unwrap();
        let module = NixosModule::new(Config::new().set("", true));
        assert!(retain_compilation(out.path(), compile_module(&module)).is_err());

        let diagnostic: serde_json::Value =
            serde_json::from_slice(&fs::read(out.path().join("diagnostic.json")).unwrap()).unwrap();
        assert_eq!(diagnostic["kind"], "Validation");
        assert!(diagnostic["primary"].is_object());
        assert!(
            fs::read_to_string(out.path().join("diagnostic.txt"))
                .unwrap()
                .contains("nonempty")
        );
        assert!(fs::read(out.path().join("nix.stderr")).unwrap().is_empty());
        for name in ["module.nix", "module-map.json", "value.json"] {
            assert!(!out.path().join(name).exists());
        }
    }
}
