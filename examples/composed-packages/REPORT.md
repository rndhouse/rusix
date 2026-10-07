# Connected package replacement: verified result

Rust authors OpenSSL, curl, Git and MariaDB package policy in a connected region.
The evaluator, derivations, output identities, fetchers, stdenv, builders, setup
hooks and unreplaced dependencies remain ordinary pinned nixpkgs/Nix.

```text
Rusix OpenSSL
      │
      ├────────► Rusix Git
      │
      ▼
Rusix curl
      │
      ▼
Rusix MariaDB (server + client)

Remaining dependency definitions: ordinary pinned nixpkgs
```

## Repository and commits

- Primary checkout was clean at `ce0a0675cc482555b58d3099334ecbd8eae62e1b`.
- All edits, evaluations, checks and commits took place in
  `/home/user/dev/worktrees/rusix-composed-packages`, branch `composed-packages`.
- The primary checkout and its HEAD remain unchanged. The worktree is retained.
- The supplied prerequisite object IDs `6242f8c…` and `c64d91d…` are absent from
  this repository's current object database. Their required changes were already
  present at the starting HEAD under `cf1f9bb2574d8c81936f25d413065d382f173b31`
  (PackageFunction/call_package) and `b743e50dc374005529692b67e1c52745d3bbdc25`
  (exact build/host equality). Both are ancestors of the starting HEAD. All 27
  focused package/API tests passed before package work. No integration was needed.
- nixpkgs remains `8b27c1239e5c421a2bbc2c65d52e4a6fbf2ff296`. The new worktree's
  checkout was cloned from the existing local submodule, with shared Git objects,
  without a network fetch. The evaluator verifies its revision and clean state.

Implementation commits, in order:

1. `92a75b25b7c407daa031a49f8b2f130fd4098d8b` — complete OpenSSL family.
2. `1e2068af5bf7f72193ff7cd68ab14c1388b01abe` — OpenSSL → curl/Git.
3. `110b0190261586f0d5ecc1b7239bfae871b8a16d` — complete MariaDB family.
4. `2f25f141b1838bc63040642dabec8ba5317e948f` — connected three-node chain.
5. `63bd3b1455e1e91d7000b00a812e6cbb601060ea` — boundary stress/reverse consumers.

This report and final documentation are committed after the final checks; the
final response identifies that last commit.

## Source and complexity

| Family | Exact pinned source | Lines / nonblank | Public arguments / defaults | Variants |
| --- | --- | --- | --- | --- |
| OpenSSL | [pkgs/development/libraries/openssl/default.nix](../../vendor/nixpkgs/pkgs/development/libraries/openssl/default.nix) | 337 / 302 | 20 / 8 | 1.1.1w, 3.0.15, 3.3.2 |
| MariaDB | [pkgs/servers/sql/mariadb/default.nix](../../vendor/nixpkgs/pkgs/servers/sql/mariadb/default.nix) | 269 / 238 | 51 / 4 | 10.5.27, 10.6.20, 10.11.10, 11.4.4; client + server for each |

OpenSSL's private shared function additionally has five recipe parameters,
three with defaults. Ordinary Rust release policy and a shared attribute
constructor preserve the supported members without copying their recipes.
The 1.1 branch retains its insecure/EOL metadata. MariaDB's version/hash remain
required generic arguments; its four public booleans retain lazy defaults.

Two new package definitions/families were rewritten in this task, representing
11 release/output-role derivations. Git and curl were already Rust authored.
The connected default region has four package nodes and three explicit
Rust-to-Rust edges; MariaDB also exposes its separate client derivation.

The final example structures are:

```text
examples/openssl-nixpkg/       examples/mariadb-nixpkg/
├── main.rs                   ├── main.rs
├── mod.rs                    ├── mod.rs
├── model.rs                  ├── model.rs
├── inputs.rs                 ├── inputs.rs
├── lowering.rs               ├── lowering.rs
├── scripts.rs                ├── scripts.rs
└── README.md                 └── README.md

examples/composed-packages/
├── main.rs
├── composition.rs
├── README.md
└── REPORT.md
```

Both families share policy using ordinary Rust functions, records and enums.
The generated graph shares values through existing lazy NixValue callback
bindings. No generic core API, schema, CMake DSL, dependency builder, platform
framework or shell DSL was added. Dynamic attribute lookup, foreign library
objects, metadata, emulators and backend test references use appropriate existing
NixValue interop. The only existing curl implementation change exposes its
factory for reuse; Git/curl also gain small Rust module entry points.

OpenSSL's bootstrap fetchurl relationship is preserved: source fetching uses the
same fetchurl argument and patches use checked-in files. No fetchpatch dependency
is introduced into this low-level recipe.

## Exact default derivation identities

Every entry below has an identical upstream/Rusix ATerm recipe and output paths.
These are logical store identities produced inside disposable local stores;
the host store was never selected.

| Node | Exact `.drv` |
| --- | --- |
| OpenSSL 3.3.2 | `/nix/store/mqf1h79k4p5y723yvn0869ni24ifpnpn-openssl-3.3.2.drv` |
| curl 8.11.0 (normal pkgs.curl flavour) | `/nix/store/m9lkswh8vqpcpvj43mlp243jqgd3ka7q-curl-8.11.0.drv` |
| Git 2.47.0 | `/nix/store/qk30rjnnsz9lxdp9b6k5hbjj7p8w17pg-git-2.47.0.drv` |
| MariaDB server 10.11.10 | `/nix/store/ph4f1jlld7627mn1xqb05d357gw0l663-mariadb-server-10.11.10.drv` |
| MariaDB client 10.11.10 | `/nix/store/gd3f8yw68jnsh5m09rkylxa093pqa7s2-mariadb-client-10.11.10.drv` |

OpenSSL 1.1.1w is
`/nix/store/yv5h5fjng2wx74dapkzjv0wcfnfh6yjn-openssl-1.1.1w.drv`;
OpenSSL 3.0.15 is
`/nix/store/6pd6xasnjw79pgi54spvz44i3l15b5b4-openssl-3.0.15.drv`.

The independent curl factory default is curlMinimal, whose matching identity
remains `/nix/store/cb2y179hgas7837a8wnx08gxawn61p1m-curl-8.11.0.drv`.
The composition explicitly selects the normal upstream curl flavour's IDN, PSL,
Zstd and conditional Brotli policy, because MariaDB depends on pkgs.curl.

## Equivalence matrices

| Suite | Rust tests | Recorded equality cases | Main package recipe comparisons |
| --- | --- | --- | --- |
| OpenSSL | 8 | 100 | 94 |
| MariaDB | 8 | 110 | 210 (105 server/client pairs) |
| Composition | 16 | 18 | 85, plus 3 reverse Nix consumer comparisons |

Total: **392 exact main package recipe comparisons**, across parameterized
cases, with no differing recipes. This counts comparisons, not distinct store
paths, and excludes additional matching source and passthru-test recipes.

OpenSSL covers all three releases, all seven independent feature switches in
both directions (42 cases), conf and attribute overrides, all argument/default
interfaces and the complete family interface. Its 39 platform/static cases span
six non-default native systems, six cross systems and a real static package set
for every release: ARM Linux, Darwin x86/ARM, armv7, PowerPC, RISC-V,
Linux/musl, MinGW, FreeBSD and MIPS. It also tests unsupported configuration
rejection on both sides, invalid fetcher provenance, excluded dependency laziness,
unrelated library overrides and the legacy branch's unneeded KTLS default.
The recursive finalAttrs pkg-config test matches for all releases and overrides.

MariaDB covers all four releases, all 16 storage/embedded/NUMA combinations for
each (64 cases), 24 supported native/cross cases, eight version/attribute override
cases, family/default interfaces, four sets of supported real NixOS test recipes,
and excluded dependency/client-sibling laziness. Native Linux ARM/i686 and both
Darwin systems, plus Linux ARM and musl cross builds, match. Ten logical rejection
cases are checked on both sides (20 evaluations): four FreeBSD emulator limits,
four broken autobackup tests, an invalid fetcher and unsupported Solaris libc.

The existing full Git (42 tests), curl (23 tests), PostgreSQL (57 tests) and
PostgreSQL schema (4 tests) suites remain passing. After OpenSSL injection,
Git/curl default recipes match, tagged inputs are observable, and live OpenSSL
withZlib variation changes their derivations as well as MariaDB server/client
while matching the equivalently wired upstream graph.

## Composition evidence and boundaries

The readable authoring implementation is [composition.rs](composition.rs), rather than a
test-only graph constructor. It instantiates the Rust OpenSSL factory using
Nixpkgs::call_package; explicit records supply that value to Rust curl and Git.
Another explicit curl record supplies the constructed Rust curl to Rust MariaDB.

Tags added through normal overrideAttrs passthru records are visible in curl's
OpenSSL passthru, Git's actual OpenSSL buildInput and MariaDB's actual curl
buildInput. The tags are absent on default nixpkgs dependencies. Reverse Nix
consumers also observe these tags. This detects accidental dependency fallback
independently of identical default derivations.

Seven distinct authoring-boundary configurations pass:

| Configuration | Dependency authoring |
| --- | --- |
| A | nixpkgs OpenSSL → Rust curl → Rust MariaDB |
| B | Rust OpenSSL → Rust curl → Rust MariaDB |
| C | Rust OpenSSL → Rust Git, with curl/MariaDB deliberately throwing but unforced |
| D | nixpkgs curl → Rust MariaDB |
| E | Rust OpenSSL → ordinary Nix curl |
| F | Rust curl (consuming Rust OpenSSL) → ordinary Nix MariaDB |
| G | Rust OpenSSL → ordinary Nix Git |

In addition, the connected graph tests all eight pairings of OpenSSL 3.0/3.3
with the four MariaDB releases, live OpenSSL and MariaDB argument overrides,
tagged control values, and an excluded OpenSSL node through curl/MariaDB.
The 18 recorded equality cases include these variations and repeated default
controls. The standalone composition uses the existing x86_64-linux scope;
independent package factories additionally accept and test native/cross caller
scopes. No new package-set/platform configuration API is claimed.

All other explicit arguments and transitive edges retain normal nixpkgs wiring.
This includes MariaDB's other direct OpenSSL argument, Git's curl argument,
bootstrap fetchers and the existing test graph. Exact default recipes yield the
same store identities even where ordinary and Rust definitions coexist.

## Laziness, provenance and generated source

- Unused graph nodes and factories remain lazy. Rust Git can be demanded with a
  throwing curl/MariaDB branch; MariaDB server/client can be demanded with a
  throwing OpenSSL graph node excluded by curl's TLS choice.
- OpenSSL's legacy branch does not force its irrelevant KTLS default. Excluded
  cryptodev/zlib and MariaDB storage/NUMA/platform dependencies stay lazy.
- Selecting MariaDB's client does not demand invalid server-only arguments.
- OpenSSL/curl finalAttrs recursion and overrides preserve upstream behavior.
  MariaDB has lexical common attributes instead: argument overrides reconstruct
  both siblings; outer overrideAttrs drops the post-mkDerivation sibling fields,
  exactly as upstream does.
- Failure inside OpenSSL's fetcher operation survives the full chain with an
  OpenSSL Rust origin. curl consuming malformed supplied OpenSSL maps to curl's
  Rust consuming operation. MariaDB demanding a failing supplied Rust curl maps
  to that child's curl operation. Raw Nix traces and generated artifacts are
  saved under `target/composed-equivalence/`.
- A delayed dependency type check entirely within stdenv has no generated child
  frame. A literal integer supplied as MariaDB's curl therefore maps only to the
  outer mariadb.drvPath demand, while raw Nix names the sixth buildInput. This
  limitation is explicitly tested and documented in the main README. No invented
  child blame or eager forcing is added.

The generated default graph was inspected. Each OpenSSL/curl/MariaDB source hash
occurs once; factory bodies and dependency definitions are shared, named arguments
remain lexical, and common MariaDB attributes are reused by both roles. There are
no reconstructed full named-argument records, deepSeq or broad seq wrappers in
the connected artifact. Existing operation-level diagnostic boundaries remain;
no generic codegen defect requiring a compiler change was found. Imports/library
lookups retain the existing backend's emission strategy.

Patch comparison checks actual bytes and Nix store paths, rather than temporary
source-symlink spelling. Metadata source position is omitted from projections
because authoring moved to Rust. **Derivation recipes are not normalized.**

## Compatibility and remaining work

The exact pin has two preserved MariaDB limitations: FreeBSD's emulator cannot
run the target, and mysql-autobackup references nonexistent
nodes.machine.automysqlbackup. Both sides reject identically; raw diagnostics for
every release are saved. The other four real NixOS tests match. Solaris's existing
unknown-libc rejection is covered too. OpenSSL 1.1 retains its original insecure
metadata; equivalence tests explicitly permit that package without changing policy.

No default recipe/identity compatibility gaps were found. The largest demonstrated
obstacle for wider authoring migration is useful Rust provenance through delayed,
backend-only validation. Faithful bootstrap and dependency-splicing policy also
requires care; this run needed no core API expansion. Whole-closure migration,
global overlays, builds and arbitrary-platform composed graphs are not claimed.

No Nix package was built. No network fetch, activation, profile operation,
deployment, host GC or Nix configuration change occurred. Every Nix subprocess
used the existing evaluation.rs infrastructure and a disposable local store.

## Final verification

All final checks passed from the requested worktree:

| Check | Result / retained log |
| --- | --- |
| `cargo fmt --check` and `cargo fmt --all --check` | Passed |
| `cargo run --locked --quiet -p rusix-derive --bin check-rust-spacing` | 161 Rust files, zero gaps |
| `RUSTDOCFLAGS='-D warnings' cargo test --workspace --locked` | 495 passed, zero failed/ignored, 37 test groups; `target/final-workspace.log` |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed; `target/final-clippy.log` |
| `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --locked` | Passed; `target/final-rustdoc.log` |
| `cargo build --workspace --examples --locked` | All 15 examples built; `target/final-examples-build.log` |
| `bash scripts/check-fixtures.sh` | Passed, including all 15 example runs and all new suites; `target/final-fixtures.log` |
| UI/compile-fail | All 62 expectation fixtures passed through the workspace/fixture harness |
| Full PostgreSQL + schema, Git, curl, OpenSSL, MariaDB, composition suites | 57 + 4, 42, 23, 8, 8 and 16 tests passed, respectively |

The maintained fixture workflow includes the new examples and suites. Original
Nix diagnostic snapshots were unchanged. CLI artifacts are retained under
`target/diagnostic-fixtures/`; equality/provenance artifacts are under
`target/{openssl,mariadb,composed}-equivalence/`. Every milestone's focused and
workspace checks and warnings-denied Clippy completed successfully too.

After the final documentation commit the retained worktree is clean, the primary
checkout remains clean at its original HEAD, and the nixpkgs checkout is clean at
the requested pin.
