//! Compile the actual example-owned types and check stable rustc codes and
//! causal primary spans, not full wording. Example mains are never evaluated.
use serde::Deserialize;
use std::{fs, path::Path, process::Command};

#[derive(Deserialize)]
struct Case {
    file: String,
    code: Option<String>,
    count: usize,
    source: String,
    reason_tokens: Vec<String>,
}

#[test]
fn documented_compile_fail_cases_fail_for_the_stated_reasons() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    // Use the same Cargo target/profile directory as this integration test.
    // Multiple cached artifacts may exist; Cargo's current build is newest.
    let executable = std::env::current_exe().unwrap();
    let deps = executable.parent().unwrap();
    let library = |name: &str| {
        fs::read_dir(deps)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| {
                p.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with(&format!("lib{name}-"))
                    && p.extension().is_some_and(|s| s == "rlib")
            })
            .max_by_key(|p| p.metadata().unwrap().modified().unwrap())
            .unwrap_or_else(|| panic!("Cargo-built {name} library"))
    };
    let libraries: Vec<_> = ["rusnix_ir", "rusnix_nix", "serde_json"]
        .into_iter()
        .map(|name| (name, library(name)))
        .collect();
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("../../../tests/ui/expected.json")).unwrap();
    let mut registered: Vec<_> = cases.iter().map(|case| case.file.clone()).collect();
    registered.sort();
    let mut fixtures: Vec<_> = fs::read_dir(root.join("tests/ui"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|e| e == "rs"))
        .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    fixtures.sort();
    assert_eq!(
        registered, fixtures,
        "every UI fixture must have a diagnostic expectation"
    );
    let scratch = tempfile::tempdir().unwrap();
    let artifacts = root.join("target/typed-examples/ui");
    fs::create_dir_all(&artifacts).unwrap();
    for case in cases {
        let mut command = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()));
        for (name, path) in &libraries {
            command
                .arg("--extern")
                .arg(format!("{name}={}", path.display()));
        }
        let output = command
            .arg(root.join("tests/ui").join(&case.file))
            .args([
                "--edition=2024",
                "--emit=metadata",
                "--error-format=json",
                "--cap-lints=allow",
            ])
            .arg("-L")
            .arg(format!("dependency={}", deps.display()))
            .arg("--out-dir")
            .arg(scratch.path())
            .output()
            .unwrap();
        assert!(!output.status.success(), "{} compiled", case.file);
        let raw = String::from_utf8(output.stderr).unwrap();
        fs::write(artifacts.join(format!("{}.jsonl", case.file)), &raw).unwrap();
        let errors: Vec<serde_json::Value> = raw
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .filter(|d| {
                d["level"] == "error"
                    && (d["code"]["code"].is_string()
                        || d["spans"]
                            .as_array()
                            .is_some_and(|spans| spans.iter().any(|s| s["is_primary"] == true)))
            })
            .collect();
        assert_eq!(errors.len(), case.count, "{}: {raw}", case.file);
        for error in errors {
            assert_eq!(
                error["code"]["code"].as_str(),
                case.code.as_deref(),
                "{}: {raw}",
                case.file
            );
            let primary: Vec<_> = error["spans"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|s| s["is_primary"] == true)
                .collect();
            let text = primary
                .iter()
                .flat_map(|s| s["text"].as_array().unwrap())
                .filter_map(|t| t["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n");
            assert!(text.contains(&case.source), "{}: {raw}", case.file);
            let labels = primary
                .iter()
                .filter_map(|s| s["label"].as_str())
                .collect::<Vec<_>>()
                .join("\n");
            let labels = format!("{labels}\n{}", error["message"].as_str().unwrap_or(""));
            for token in &case.reason_tokens {
                assert!(labels.contains(token), "{}: {raw}", case.file);
            }
        }
    }
}
