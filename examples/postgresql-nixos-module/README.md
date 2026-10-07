# PostgreSQL NixOS module

This example rewrites a complete upstream NixOS module in Rust/Rusix: both its
public option declarations and its configuration implementation. Ordinary NixOS
modules can configure it through the same `services.postgresql.*` interface.
PostgreSQL packages still come from nixpkgs through opaque Nix handles.

The compatibility reference is the nixpkgs 24.11 release's Git commit
`8b27c1239e5c421a2bbc2c65d52e4a6fbf2ff296`. This commit ID pins the exact nixpkgs
source tree containing the original
[postgresql.nix](https://github.com/NixOS/nixpkgs/blob/8b27c1239e5c421a2bbc2c65d52e4a6fbf2ff296/nixos/modules/services/databases/postgresql.nix),
NixOS library and package definitions used in comparisons. It is not a PostgreSQL
version number. Tests use the pinned `vendor/nixpkgs` submodule offline.
The minimal staged subset retains its SHA-256 checksums in
[nixpkgs-pin.json](../../crates/rusix/src/nixos/nixpkgs-pin.json).

| File | Purpose |
|---|---|
| [model.rs](model.rs) | User-defined Rust types and an example PostgreSQL configuration |
| [schema.rs](schema.rs) | Public NixOS option declarations, defaults, documentation and migrations |
| [options.rs](options.rs) | Finite symbolic dependencies on final merged NixOS values |
| [lowering.rs](lowering.rs) | PostgreSQL compatibility logic using those dependencies and existing Nix helpers |
| [main.rs](main.rs) | Composes the contributions and prints generated Nix |

Start with `model.rs`. Rust models express provisioning intent; symbolic option
references let dependent outputs follow ordinary downstream NixOS overrides.
NixOS remains responsible for checking and merging the generated module.

From the repository root:

```bash
cargo run --locked -p rusix --example postgresql-nixos-module
```

[Implementation tests](../../crates/rusix/tests/postgresql.rs) and
[schema tests](../../crates/rusix/tests/postgresql_schema.rs) compare evaluated
results against the pinned upstream module in disposable isolated Nix stores.
The candidate imports no original PostgreSQL module. This experiment covers
module evaluation; it does not build or start PostgreSQL, and runtime equivalence
remains a separate milestone.
