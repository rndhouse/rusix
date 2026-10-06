use std::{fs, process::Command};

#[test]
fn cli_merge_diagnostic_serializes_both_origins_and_priority_case_succeeds() {
    let artifacts = tempfile::tempdir().unwrap();
    let run = |fixture: &str| {
        Command::new(env!("CARGO_BIN_EXE_rusnix-cli"))
            .args(["check-nixos", fixture, "--out"])
            .arg(artifacts.path())
            .output()
            .unwrap()
    };

    let failure = run("merge-two");
    assert_eq!(failure.status.code(), Some(1));
    let d: serde_json::Value =
        serde_json::from_slice(&fs::read(artifacts.path().join("diagnostic.json")).unwrap())
            .unwrap();
    assert_eq!(d["kind"], "NixosMerge");
    assert_eq!(d["origins"].as_array().unwrap().len(), 2);
    assert_eq!(
        String::from_utf8_lossy(&failure.stderr)
            .matches("   = conflicting definition")
            .count(),
        2
    );

    assert!(run("merge-priority").status.success());
    assert!(!artifacts.path().join("diagnostic.json").exists());
    assert_eq!(
        fs::read_to_string(artifacts.path().join("value.json")).unwrap(),
        "\"sshd\"\n"
    );
}

#[test]
fn cli_nixos_fixtures_retain_module_map_and_raw_errors() {
    let artifacts = tempfile::tempdir().unwrap();
    let run = |fixture: &str| {
        Command::new(env!("CARGO_BIN_EXE_rusnix-cli"))
            .args(["check-nixos", fixture, "--out"])
            .arg(artifacts.path())
            .output()
            .unwrap()
    };
    assert!(run("good").status.success());
    assert_eq!(run("type").status.code(), Some(1));
    assert!(!artifacts.path().join("value.json").exists());

    let diagnostic: serde_json::Value =
        serde_json::from_slice(&fs::read(artifacts.path().join("diagnostic.json")).unwrap())
            .unwrap();
    assert_eq!(diagnostic["kind"], "NixosType");
    assert_eq!(
        diagnostic["raw_nix"],
        fs::read_to_string(artifacts.path().join("nix.stderr")).unwrap()
    );

    let artifact: serde_json::Value =
        serde_json::from_slice(&fs::read(artifacts.path().join("module-map.json")).unwrap())
            .unwrap();
    assert_eq!(
        artifact["module"]["source"],
        fs::read_to_string(artifacts.path().join("module.nix")).unwrap()
    );
    assert!(
        !artifact["module"]["source"]
            .as_str()
            .unwrap()
            .contains("# rn-")
    );
    assert!(artifacts.path().join("evaluation.nix").exists());
    assert!(run("good").status.success());
    assert!(!artifacts.path().join("diagnostic.json").exists());
}

#[test]
fn cli_retains_artifacts_and_replaces_stale_results() {
    let artifacts = tempfile::tempdir().unwrap();
    let run = |fixture: &str| {
        Command::new(env!("CARGO_BIN_EXE_rusnix-cli"))
            .args(["check", fixture, "--out"])
            .arg(artifacts.path())
            .output()
            .unwrap()
    };

    let good = run("good");
    assert!(
        good.status.success(),
        "{}",
        String::from_utf8_lossy(&good.stderr)
    );
    assert!(artifacts.path().join("value.json").exists());

    let bad = run("nested");
    assert_eq!(bad.status.code(), Some(1));
    assert!(!artifacts.path().join("value.json").exists());

    let diagnostic: serde_json::Value =
        serde_json::from_slice(&fs::read(artifacts.path().join("diagnostic.json")).unwrap())
            .unwrap();
    assert_eq!(diagnostic["kind"], "NixEval");
    assert_eq!(diagnostic["primary"]["purpose"], "integer division");
    let stderr = fs::read_to_string(artifacts.path().join("nix.stderr")).unwrap();
    assert_eq!(diagnostic["raw_nix"], stderr);

    let generated = fs::read_to_string(artifacts.path().join("generated.nix")).unwrap();
    let map: serde_json::Value =
        serde_json::from_slice(&fs::read(artifacts.path().join("source-map.json")).unwrap())
            .unwrap();
    assert_eq!(map["source"], generated);
    assert!(!generated.contains("# rn-"));

    let good = run("good");
    assert!(good.status.success());
    assert!(!artifacts.path().join("diagnostic.json").exists());
    assert!(!artifacts.path().join("diagnostic.txt").exists());
}

#[test]
fn cli_can_select_good_and_bad_from_the_same_configuration() {
    let artifacts = tempfile::tempdir().unwrap();
    let run = |attribute: &str| {
        Command::new(env!("CARGO_BIN_EXE_rusnix-cli"))
            .args(["check", "selective", "--out"])
            .arg(artifacts.path())
            .args(["--select", attribute])
            .output()
            .unwrap()
    };

    let good = run("good");
    assert!(
        good.status.success(),
        "{}",
        String::from_utf8_lossy(&good.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&good.stdout), "42\n");

    let generated = fs::read_to_string(artifacts.path().join("generated.nix")).unwrap();
    assert!(!generated.contains("deepSeq"));
    assert!(!generated.contains("# rn-"));

    let bad = run("bad");
    assert_eq!(bad.status.code(), Some(1));
    assert_eq!(
        fs::read_to_string(artifacts.path().join("generated.nix")).unwrap(),
        generated
    );

    let diagnostic: serde_json::Value =
        serde_json::from_slice(&fs::read(artifacts.path().join("diagnostic.json")).unwrap())
            .unwrap();
    assert_eq!(diagnostic["kind"], "NixEval");
    assert_eq!(diagnostic["primary"]["purpose"], "integer division");
    assert!(String::from_utf8_lossy(&bad.stderr).contains("= option: bad"));
    assert_eq!(
        diagnostic["raw_nix"],
        fs::read_to_string(artifacts.path().join("nix.stderr")).unwrap()
    );
}

#[test]
fn switching_commands_clears_all_owned_artifacts_and_preserves_unrelated_files() {
    let artifacts = tempfile::tempdir().unwrap();
    fs::write(artifacts.path().join("notes.txt"), "keep me").unwrap();
    let run = |command: &str, fixture: &str| {
        Command::new(env!("CARGO_BIN_EXE_rusnix-cli"))
            .args([command, fixture, "--out"])
            .arg(artifacts.path())
            .output()
            .unwrap()
    };

    assert!(run("check", "good").status.success());
    assert!(artifacts.path().join("generated.nix").exists());
    assert!(run("check-nixos", "good").status.success());
    assert!(!artifacts.path().join("generated.nix").exists());
    assert!(!artifacts.path().join("source-map.json").exists());
    assert!(artifacts.path().join("module.nix").exists());

    assert!(run("emit", "good").status.success());
    for name in [
        "module.nix",
        "module-map.json",
        "nixos-driver.nix",
        "evaluation.nix",
        "value.json",
        "nix.stderr",
    ] {
        assert!(!artifacts.path().join(name).exists(), "stale {name}");
    }
    assert!(artifacts.path().join("generated.nix").exists());
    assert_eq!(
        fs::read_to_string(artifacts.path().join("notes.txt")).unwrap(),
        "keep me"
    );

    // Reject unknown fixtures before clearing a previously valid artifact set.
    for command in ["check", "check-nixos"] {
        assert_eq!(run(command, "missing").status.code(), Some(1));
        assert!(artifacts.path().join("generated.nix").exists());
    }
}
