#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

cargo run --locked --quiet -p rusnix-derive --bin check-rust-spacing

# Real evaluator round-trips compare the committed Rusnix diagnostic snapshots.
cargo test --locked -p rusnix-nix --lib context_audit
cargo test --locked -p rusnix-nix --lib render_audit
cargo test --locked -p rusnix-nix --lib render::tests
cargo test --locked -p rusnix-nix --test roundtrip
cargo test --locked -p rusnix-nix --test nixos
cargo test --locked -p rusnix-nix --test merges
cargo test --locked -p rusnix-nix --test authoring
cargo test --locked -p rusnix-nix --test symbolic_options
cargo test --locked -p rusnix-nix --test derive
cargo test --locked -p rusnix-nix --test config_module
cargo test --locked -p rusnix-nix --test options --test args
cargo test --locked -p rusnix-nix --test postgresql
cargo test --locked -p rusnix-nix --test schema --test postgresql_schema
cargo test --locked -p rusnix-nix --test symbolic_text
cargo test --locked -p rusnix-nix --test structured_interop --test library
cargo test --locked -p rusnix-nix --test package --test package_functions --test git --test curl --test nix_operations
cargo test --locked -p rusnix-nix --test typed_examples --test ui --test interop

for example in enum-option typed-submodule invalid-states function-contracts exhaustive-match typed-values layered-validation nix-interop symbolic-option postgresql-nixos-module git-nixpkg curl-nixpkg; do
    cargo run --locked --quiet -p rusnix-nix --example "$example"
done

# Existing generic escape-hatch/compiler example (evaluation is in roundtrip.rs).
cargo run --locked --quiet -p rusnix-nix --example ssh

# Preserve reviewable outputs from the executable harness as well.
cargo run --locked --quiet -p rusnix-cli -- check good --out target/diagnostic-fixtures/good
cargo run --locked --quiet -p rusnix-cli -- check selective --out target/diagnostic-fixtures/selective-good --select good
for fixture in bad-port nested conflict codegen-bug unmapped selective; do
    out="target/diagnostic-fixtures/$fixture"
    selection=()
    if [ "$fixture" = selective ]; then selection=(--select bad); fi
    if cargo run --locked --quiet -p rusnix-cli -- check "$fixture" --out "$out" "${selection[@]}"; then
        echo "Fixture $fixture unexpectedly succeeded" >&2
        exit 1
    else
        status=$?
        [ "$status" -eq 1 ] || exit "$status"
    fi
    case "$fixture" in
        conflict) kind=Validation ;;
        codegen-bug) kind=Compiler ;;
        *) kind=NixEval ;;
    esac
    diagnostic=$(cat "$out/diagnostic.json")
    expected="\"kind\": \"$kind\""
    case "$diagnostic" in
        *"$expected"*) ;;
        *) echo "Unexpected diagnostic kind for $fixture; expected $kind" >&2; exit 1 ;;
    esac
done

for fixture in merge-ok merge-priority; do
    cargo run --locked --quiet -p rusnix-cli -- check-nixos "$fixture" --out "target/diagnostic-fixtures/$fixture"
done
for fixture in merge-two merge-three merge-mixed merge-three-type; do
    out="target/diagnostic-fixtures/$fixture"
    if cargo run --locked --quiet -p rusnix-cli -- check-nixos "$fixture" --out "$out"; then
        echo "Merge fixture $fixture unexpectedly succeeded" >&2
        exit 1
    else
        status=$?
        [ "$status" -eq 1 ] || exit "$status"
    fi
    if [ "$fixture" = merge-three-type ]; then kind=NixosType; else kind=NixosMerge; fi
    diagnostic=$(cat "$out/diagnostic.json")
    expected="\"kind\": \"$kind\""
    case "$diagnostic" in
        *"$expected"*) ;;
        *) echo "Unexpected merge diagnostic kind for $fixture; expected $kind" >&2; exit 1 ;;
    esac
done

cargo run --locked --quiet -p rusnix-cli -- check-nixos good --out target/diagnostic-fixtures/nixos-good
cargo run --locked --quiet -p rusnix-cli -- check-nixos lazy --out target/diagnostic-fixtures/nixos-lazy
for fixture in type unknown assertion external; do
    out="target/diagnostic-fixtures/nixos-$fixture"
    if cargo run --locked --quiet -p rusnix-cli -- check-nixos "$fixture" --out "$out"; then
        echo "NixOS fixture $fixture unexpectedly succeeded" >&2
        exit 1
    else
        status=$?
        [ "$status" -eq 1 ] || exit "$status"
    fi
    case "$fixture" in
        type) kind=NixosType ;;
        unknown) kind=NixosModule ;;
        assertion) kind=NixosAssertion ;;
        external) kind=ExternalNix ;;
    esac
    diagnostic=$(cat "$out/diagnostic.json")
    expected="\"kind\": \"$kind\""
    case "$diagnostic" in
        *"$expected"*) ;;
        *) echo "Unexpected NixOS diagnostic kind for $fixture; expected $kind" >&2; exit 1 ;;
    esac
done
