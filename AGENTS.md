# Rusnix development

Keep the Rust → semantic IR → Nix AST → Nix → Rust diagnostic loop small.
Users edit Rust; generated Nix is compiler output.

Commit after each reasonable, coherent, verified batch of work. Use the configured
human Git identity only; never add AI/LLM/assistant/agent/harness/tool/vendor
co-author or attribution trailers.

All Nix subprocesses, including version checks, must go through
`crates/rusnix-nix/src/isolated.rs`. Each command explicitly selects a fresh
disposable local store. Never invoke Nix directly against the host store, even
for evaluation. Do not run activation, deployment, profile, rebuild, host GC,
privileged operations, or change host Nix configuration. Do not add network
fetches, flakes, or builders to this POC. The explicitly requested NixOS
experiment uses the checked, pinned `vendor/nixpkgs` Git submodule (a staged minimal
subset and the same full checkout for interoperability); evaluation is offline.

Run `cargo test --workspace --locked`, `cargo fmt --all --check`, and
`cargo clippy --workspace --all-targets --locked -- -D warnings` after changes.
The tests require `nix` and `nix-instantiate`; missing tooling is a failure,
not a reason to silently skip the round-trip tests.

Rust readability: separate distinct item declarations at the same level with
one blank line, including items in inline modules, impls, traits, and function
blocks. Keep comments/docs and attributes attached to their item. Consecutive
plain `use` declarations may stay together as an import group; separate the
group from other items. Keep tightly related statements and fields compact.
Use blank lines between conceptual blocks in non-trivial Rust functions; keep tightly related statements together.
Apply the convention to macro templates and documentation examples by review.

Stable rustfmt remains the canonical formatter. It preserves item spacing but
does not enforce it. Run the structural spacing check (also run by workspace
tests and the fixture script):

```bash
cargo run --locked --quiet -p rusnix-derive --bin check-rust-spacing
```

After rustfmt, its `--fix` option inserts missing blank lines. The checker parses
all repository `.rs` files except `target/`, `.git`, and nested Git checkouts;
it checks AST item lists, not opaque macro tokens or strings/docs.

`bash scripts/check-fixtures.sh` compares the evaluator diagnostic snapshots and
saves CLI artifacts under `target/diagnostic-fixtures/`. Snapshots live in
`tests/snapshots/`. Update an expected snapshot only after understanding the
behavior change. Preserve original Nix diagnostics and explicit unmapped cases.

Keep upstream diagnostic parsing in `diagnostic.rs`. Prefer JSON trace fields;
test the text fallback too. Syntax/static generated errors must remain compiler
failures with no Rust blame. Record provenance limitations in README.md.

Generated values must stay lazy: do not reintroduce broad `deepSeq`/`seq`
forcing. Runtime contexts belong on fallible operations; source-map ancestry
recovers enclosing paths. Keep the selective good/bad tests and the existing
nested operation diagnostic passing. Selection must use escaped attribute data
through the same isolated helper, never user-supplied command flags.
