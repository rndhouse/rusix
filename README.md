# Rusnix

A small experiment in using ordinary Rust as a typed frontend for Nix. The
working loop is Rust type checking → semantic IR → validation → Nix AST →
generated Nix → isolated evaluation → a diagnostic pointing back to Rust.
Generated Nix is compiler output. Edit Rust configuration functions.

For API documentation, run `cargo doc -p rusnix-ir --no-deps --open`. Start with
`rusnix_ir`: Rust values describe configuration now, while `Expr`, `OptionRef`
and `NixValue` describe expressions that Nix evaluates later. Its rustdoc
introduces packages, utility functions and NixOS modules before their Rusnix
wrappers. `rusnix_nix` documents compilation and isolated evaluation.

[Typed deferred package authoring](docs/typed-package-values.md) preserves package,
function, list, record and scalar interfaces through lazy bindings and composition.
The OpenSSL, curl, Git and MariaDB examples use these interfaces; dynamic interop
continues to use `NixValue`.

```text
User-defined Rust domain model
        ↓
Rusnix IntoConfig / generic configuration machinery
        ↓
semantic IR → Nix AST → Nix backend
        ↓
Nix/NixOS ecosystem
```

Rusnix uses Rust's existing type system rather than defining a new configuration
type system. Domain types belong to users and libraries. Core provides composition,
lowering, Nix interoperability, diagnostics and generic validation; it intentionally
does not ship a catalogue of networking, TLS, account or service models.

This POC proves error provenance for finite configuration data, deferred integer
operations, and a small actual NixOS module evaluation surface. It preserves
selective evaluation without forcing siblings. The original plain-attribute-set
fixtures remain; the separate NixOS harness uses real OpenSSH option declarations.

The [typed configuration showcase](examples/README.md) demonstrates static
semantic types, unrepresentable field combinations, caller contracts, and
exhaustive model consumers. The examples define their own domain types and
distinguish user models, opaque Nix objects and the generic escape hatch.
Executable [Nix comparisons](tests/comparisons/) and registered
[UI fixtures](tests/ui/expected.json), including interop category safety and
missing TLS keys, back their claims. The examples explain which guarantees are
static, which require IR checks, and which remain NixOS checks; generic
configuration/IR escape hatches remain explicit.

## Connected package authoring

The [composed package example](examples/composed-packages/README.md) rewrites a
connected region of pinned nixpkgs authoring:

```text
Rusnix OpenSSL ──► Rusnix Git
      │
      ▼
Rusnix curl
      │
      ▼
Rusnix MariaDB (server and client)
```

A whole dependency closure does not need to be rewritten. Each package can first
consume ordinary nixpkgs dependencies, then accept rewritten values one by one.
Explicit Rust `Nixpkgs::call_package` override records construct these three
rewritten edges. For matched defaults, the OpenSSL, curl, Git and MariaDB
server/client derivation recipes and identities match ordinary pinned nixpkgs.
Nix still supplies evaluation and store semantics; nixpkgs supplies stdenv,
fetchers, builders, hooks and all unreplaced dependencies. Verification only
constructs derivations offline in disposable stores.

[OpenSSL](examples/openssl-nixpkg/README.md) and
[MariaDB](examples/mariadb-nixpkg/README.md) preserve their full pinned package
families. Their suites cover argument interfaces, features, outputs, overrides,
platform behavior, laziness and provenance; the composed suite additionally
checks tagged dependency values and live override propagation.

Package-operation failures retain useful child Rust origins across the graph.
A delayed type check wholly inside stdenv can have no generated child frame:
for example, a literal integer supplied as MariaDB's curl maps only to the outer
`mariadb.drvPath` demand. The original diagnostic still names MariaDB's sixth
buildInput. This limitation is tested explicitly; no broad forcing or invented
child blame is added to compensate.

## Run

Prerequisites: Git, Rust 1.88+ and `nix` plus `nix-instantiate` on PATH. Tested here
with Rust 1.97.1 and Nix 2.34.8 on Linux. The live structured-diagnostic tests
expect the `raw_msg`/`trace` fields provided by that Nix version. Older diagnostic
formats have adapter coverage, not a tested cross-version compatibility promise.

Initialize the pinned nixpkgs submodule before testing or evaluation:

```bash
git submodule update --init --depth=1
```

The submodule is configured for shallow clones. A normal Rusnix clone contains
only this project; initialization downloads the pinned nixpkgs source tree without
its full history. Once initialized, evaluation is offline. See [vendor/README.md](vendor/README.md).

```bash
cargo test --workspace --locked
cargo run --locked -p rusnix-nix --example nix-interop
cargo run --locked -p rusnix-cli -- check good --out target/demo/good
cargo run --locked -p rusnix-cli -- check nested --out target/demo/nested
```

The last command deliberately exits 1, with a diagnostic like:

```text
error[nix-eval]: generated configuration was rejected by Nix
  --> crates/rusnix-cli/src/fixtures.rs:15:47
   |
 15 |             vec![Expr::int(22), Expr::int(44).divide(Expr::int(0))],
   |                                               ^
   = origin: integer division
   = option: services.openssh.ports
   = Nix: division by zero
```

`check` saves `generated.nix`, `source-map.json`, and `nix.stderr`, plus either
`value.json` or `diagnostic.json` and `diagnostic.txt`. The source map includes
the exact generated text, byte spans, Rust origins, and enclosing semantic origins. The JSON diagnostic is
Rusnix's own representation and retains the original Nix stderr. Reusing an
output directory clears compiler-owned artifacts from both plain and NixOS
commands, including stale sources, maps and results. Unrelated files are
preserved. Unknown fixtures are rejected before cleanup, and compilation failures
in either mode save diagnostics.

`emit` generates artifacts without running Nix. `version` probes Nix through
the isolated helper too. `rusnix-cli` is a fixture harness; it does not dynamically
load or sandbox arbitrary Rust programs. To configure something new, edit
`examples/*.rs`, the fixture functions, or call these libraries from Rust.

Select a single top-level attribute with the isolated evaluator:

```bash
cargo run --locked -p rusnix-cli -- check selective --out target/selective/good --select good
cargo run --locked -p rusnix-cli -- check selective --out target/selective/bad --select bad
```

Both commands compile the same Rust fixture:

```rust
Config::new()
    .set("good", Expr::int(42))
    .set("bad", Expr::int(44).divide(Expr::int(0)))
```

Selecting `good` returns `42` without encountering the failure in `bad`.
Selecting `bad` exits 1 and identifies `.divide()` at
`crates/rusnix-cli/src/fixtures.rs:22:39`, retaining `option: bad`.
The backend API is `NixSession::evaluate_attribute(&generated, "good")`.
It adds an escaped `--apply 'value: builtins.getAttr "good" value'` through the
existing private command factory. Attribute names are literal data, not CLI
flags or arbitrary Nix source. Selection is a single attribute name, not a
dot-separated path. The complete source is still parsed/static-checked first;
JSON serialization demands only the selected value and its children.

## Rust API and boundaries

```rust
use rusnix_ir::{self as rusnix, nixos::NixosModule};

#[rusnix::config]
mod config {
    #[rusnix(root)]
    pub struct MyConfig { services: Services }

    struct Services { example: ExampleService }

    struct ExampleService { enable: bool, port: u16 }

    pub fn model() -> MyConfig {
        MyConfig { services: Services {
            example: ExampleService { enable: true, port: 8080 },
        } }
    }
}
let module = NixosModule::empty().add(config::model());
```

Struct nesting defines Nix attribute nesting: this contribution defines
`services.example.enable` and `services.example.port`. The fictional service
needs an ordinary NixOS option declaration for module evaluation. Plain-data
examples can also use `rusnix_nix::compile(&config::model().into_config())`
with the IntoConfig trait imported.
No prefix strings are needed. Rust `snake_case` fields lower to Nix-style
`lowerCamelCase` by default. Struct-level `#[rusnix(rename_all = "PascalCase")]`
selects a convention for schemas such as systemd; explicit field renames are
reserved for exceptions. Each nested struct names its own fields independently.

The module attribute is reexported as `rusnix_ir::config` (aliased above). It adds
the existing derives to immediate local structs and IntoConfig to explicitly
marked roots; multiple roots are supported. It retains explicit conversions
spelled with the canonical IntoConfig/IntoRusnixValue names and imports external
values through their existing traits. Aliased or macro-generated conversion
implementations stay on the fine-grained path outside the boundary. Unit enums
lower automatically to lowerCamelCase strings: Server → "server", ReadOnly →
"readOnly". Reusable unit enums use the same IntoRusnixValue derive. Data-carrying
enums still require explicit semantic mappings. Named structs and transparent
single-field newtypes have the same mapping rules as fine-grained derives.

Only inline modules are supported: no file loading, child-module traversal or
function-local/macro-produced type discovery. Reusable types and multi-file
configurations keep the fine-grained APIs below. Normal Rust visibility applies;
there is no global schema, type catalogue or alternate IR.

`IntoConfig` derive generates both rooted contribution conversion and nested
`IntoRusnixValue` conversion. Prefer the latter for reusable values: a parent
places an Endpoint, rather than Endpoint knowing a global service path.
Single-field tuple newtypes derive IntoRusnixValue transparently; named structs
become records. Generics and borrowed strings are supported. Do not derive both
traits on the same struct, since IntoConfig already supplies the value impl.

Struct/enum-level rename_all supports only `lowerCamelCase` (the default) and
`PascalCase`. Field/variant-level `#[rusnix(rename = "...")]` overrides that convention;
`#[rusnix(flatten)]` and `#[rusnix(skip)]` retain their structural meanings.
Rename is **one literal attribute name**, so
`rename = "a.b"` does not create a prefix. Flatten combines record fields within
one contribution; conflicting leaf paths are IR validation errors. Skip excludes
local state from conversion and its trait bounds. Unknown mappings,
invalid/duplicate rename_all, rename_all on transparent newtypes and incompatible
field attributes are compile errors.

The public unsealed IntoRusnixValue trait returns an opaque `RusnixValue`.
Extensions can delegate to another value's `into_value()`, or use `leaf`/`record`
constructors; raw Node/Nix AST construction is unnecessary. Leaf enum mappings
and structural alternatives remain explicit exhaustive Rust matches—there is
no tagged-enum convention. Records inside lists remain structured semantic IR.
`Expr`, OptionRef, PackageRef and opaque NixValue leaves retain their native
semantics and previously captured origins. Module/function/overlay handles also
retain references as values; module imports still use NixosModule::import_ref,
not an `imports` configuration field. InputRef is lookup context, not a value.

Derived leaves carry stable path-specific IDs and conversion/add caller locations.
Captured operation/reference locations survive; ordinary primitive field
initializers do not acquire separate source spans. Contributions are never
flattened across NixosModule::add calls, preserving NixOS priorities and
multi-origin conflicts. Handwritten IntoConfig remains available for custom
adapters, and Config::set remains the explicit generic escape hatch.
The [examples](examples/README.md) use `#[rusnix::config]` for local trees,
fine-grained derives for reusable values, and explicit implementations for
semantic conversions. The nine small examples contain no handwritten IntoConfig
impls; PostgreSQL adds one semantic adapter for optional inputs and ownership.
Nix-interop uses Config::set for its dynamic escape hatch, and curl uses it to
export its factory and package. Layered-validation failure assemblies live in tests.

`Expr<i64>` and `Expr<bool>` are distinct Rust types. Passing a boolean
expression to integer division fails Rust type checking. Ordinary Rust vectors
remain homogeneous; `NixValue::list` explicitly permits mixed boundary values.
Plain bools, strings, signed integers, `u16`, f64 and nested Rust vectors are
supported. NixValue adds nulls, maps, and structured opaque arguments.
The sealed value conversion trait deliberately keeps the API small.

| Component | Responsibility |
| --- | --- |
| `rusnix-ir` | Structural conversion, semantic nodes, origin capture, validation |
| `rusnix-derive` | Structural authoring/reference macros, conversion derives, text interpolation |
| `rusnix-nix/ast.rs` | Backend expression syntax, separate from semantic values |
| `rusnix-nix/lib.rs` | Semantic lowering into the AST |
| `rusnix-nix/render.rs` | Escaped Nix source, optional inspection comments, contexts, byte source map |
| `rusnix-nix/isolated.rs` | Sole Nix subprocess boundary and disposable store owner |
| `rusnix-nix/diagnostic.rs` | JSON/text adaptation into owned Rusnix diagnostics |
| `rusnix-cli` | Reviewable fixture artifacts and exit status |

Build the public API documentation with `cargo doc --workspace --no-deps`;
start at `target/doc/rusnix_ir/index.html` for authoring or
`target/doc/rusnix_nix/index.html` for compilation and isolated evaluation.
The library crates warn on missing public docs. Documentation verification uses
`RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`.

Every semantic node, assignment, and configuration root has an origin. Public
constructors and operations propagate `#[track_caller]`; primitives supplied to
`.set()` inherit that call's location. Explicit expression constructors capture
their own location, so the nested failure identifies `.divide()`, rather than
the list or its assignment. Rust validation rejects duplicate/prefix-conflicting
paths, empty path segments, and unsupported NUL bytes before lowering.

`in_range()` represents a deferred domain constraint, lowered to Nix comparisons
and a conditional throw. It intentionally exercises backend validation;
Rusnix does not implement Nix evaluation in Rust. Local Nix let bindings share
subexpressions without duplicating their rendered source.

## Provenance findings

Rusnix identifies source origins with IDs such as `rn-e160b9de21c72674`.
Source maps and diagnostic JSON retain that ID unchanged, including all 16
hexadecimal digits. Generated Nix uses it at selected `addErrorContext` failure
boundaries and in NixOS metadata. Normal output omits fine-grained origin comments.
For manual inspection, pass `RenderOptions { origin_comments: true }` to
`compile_with_options`, `nixos::compile_module_with_options`, or the advanced
`render_with_options` API. The [precedence-aware renderer](docs/nix-expression-rendering.md) and
[width-aware pretty-printer](docs/generated-nix-layout.md)
use a 100-character target and record spans while emitting formatted text.
Each mode computes source spans for its own output;
comments add observability without changing runtime contexts or NixOS metadata.

Four mechanisms are exercised:

1. **Source spans:** byte ranges in generated Nix map to Rust origins. The
   smallest enclosing range provides a fallback for a generated Nix frame.
   External-file positions are never mapped as though they came from this file.
   Each span also stores enclosing configuration/assignment/value origins,
   allowing an option path to survive even when its runtime context has unwound.
2. **Optional inspection markers:** annotated output contains `# rn-...` comments
   for manually matching expressions to Rust origins. Diagnostics never parse
   these comments; each rendering has its own correctly computed source spans.
3. **Evaluation contexts:** opaque calls/imported values, final NixOS option
   dependencies, explicit validation, prefix coercion and shallow record union retain
   `builtins.addErrorContext "rn-..." (...)`. Ordinary selections,
   conditions, equality, division and `toString` use generated positions instead.
   Record union retains a boundary because Nix can blame only the containing field
   for an invalid operand. Literals, lists, record construction and assignments
   have no runtime context.
   Static ancestry supplies the enclosing option path independently of demand.
4. **Structured diagnostics:** `--log-format internal-json --show-trace` on
   Nix 2.34.8 provides a message, underlying `raw_msg`, and structured `trace`.
   Rusnix prefers those fields. JSON trace order is innermost first; the rendered
   text lists the outermost context first. An adapter test changes the rendered
   message completely while preserving the correct structured mapping.

The important discovery is that `addErrorContext` alone forces only the outer
value. A context around `{ nested = throw "..."; }` has already unwound when
JSON serialization evaluates `nested`. A regression test demonstrates this
lost provenance. The first implementation used `deepSeq` within each context
to keep outer contexts alive. The selective-evaluation experiment removed
**all generated `deepSeq` calls** and the per-node forcing let bindings.
Runtime markers are now reserved for intentional failure/interop boundaries.
A division renders without a runtime wrapper:

```nix
(builtins.div 44 0)
```

Nix's generated builtin frame identifies the consuming Rust division, including
when delayed inside a list or an opaque library call. The source map supplies
its enclosing option path. A concrete generated error position is preferred to
callback-definition frames; a mapped child operation can outrank an enclosing
runtime marker when the evaluator identifies the inner failure. Static ancestry
supplies its contribution and path, including across separately generated modules.

The [runtime instrumentation audit](docs/runtime-diagnostics.md) records the
complete policy and before/after failure matrix. Git now has 301 runtime markers
instead of 1,226. Origin comments are available explicitly for inspection.
Prefix coercion and final
NixOS references retain narrow markers because live tests demonstrate missing
or misleading generated positions in those cases. No container forcing is added.

For an expression cloned into two assignments, concrete generated positions
choose the correct occurrence's ancestry. If positions are absent, the ID-only
lookup still chooses the first matching span and may report an ambiguous path.
The primary diagnostic's `Provenance` describes its location mapping method;
`related` origins can also include static ancestry, not only runtime frames.

Generated Nix is simpler: synthetic assignment lets and eager forcing wrappers
are gone. Source maps grow because each span stores its ancestry. Normal
selective demand is preserved for the tested scalar/list cases on Nix 2.34.8.
There is no claim about arbitrary lazy functions. The NixOS experiment below
adds a deliberately limited imported-module boundary mapping.

`nix-instantiate --parse` runs before evaluation to detect generated syntax and
static binding errors. These become `error[codegen]` with no Rust primary
location, even if a source span exists. Recognized parser failures are distinct
from tooling failures. `nix eval --apply 'x: true'` was experimentally unsuitable
as a parse check: Nix still forces the file. A valid parsed expression that
throws or divides by zero becomes `error[nix-eval]`.

For older JSON logs containing only `msg`, or plain stderr, one adapter strips
ANSI escapes and extracts trace-shaped contexts and generated positions. That
text fallback and parser-failure classification are intentionally narrow
heuristics. Upstream format changes can reduce provenance or require adapter
updates; raw stderr remains available. This is not a security boundary.

Origins use specified FNV-1a hashing of Rust file, line, column, and semantic
purpose. IDs are deterministic across runs at unchanged source locations. They
are **origin IDs**, not unique runtime instances: repeated executions of the
same constructor call site can share one. Line edits, moved files, and Rust
path remapping change IDs. Wrapper libraries must propagate `#[track_caller]`
when they want their own caller rather than their helper body recorded.

Location capture gives a point, not a Rust expression's full span. Underlines
are a single caret; tabs/Unicode can affect visual alignment. Missing source
files still produce file/line/column diagnostics. Unannotated backend failures
explicitly report unavailable provenance; synthetic scaffolding may only map
to an enclosing origin. Errors far from a generated value can still lack a precise Rust expression;
the NixOS experiment below recovers definition or import boundaries in tested cases.
Missing-attribute selection failures occur in the evaluator's small `--apply`
expression and currently have no Rust primary origin. A shallow container
context still loses runtime provenance; static ancestry only helps when an
operation origin or mapped generated frame can be recovered.

## Store isolation

**The normal Nix store is never selected.** `NixSession` owns a fresh temporary
root, and its private command factory adds `--store <root>/store` before every
Nix subprocess, including parse and version commands. Evaluation additionally
sets `--eval-store` to the same path. These flags cannot be omitted or overridden
through the public session API. There is no public arbitrary-command helper.

Host Nix configuration, store-selection environment variables, and Nix search
paths are cleared or redirected. HOME and XDG directories are private to the
child session. Evaluation is offline, substituters/builders are empty, and no
frontend operation fetches, builds, activates, or deploys anything. Temporary
store roots are removed when the session drops; process termination can leave
an abandoned temporary directory.

Evaluations sharing a `NixSession` run serially, including staging, parsing and
diagnostic translation. This prevents concurrent calls from overwriting the
session's input files. Separate sessions keep separate stores. Checked minimal
nixpkgs files are staged once per session and reused by later evaluations;
[session tests](crates/rusnix-nix/tests/session.rs) cover concurrent calls and
staging reuse.

Nix's chroot local store still uses logical `/nix/store` names. The **physical**
store is `<root>/store/nix/store`, with its database inside that same root.
One regression test uses `builtins.toFile` through the isolated helper and
checks the physical file/database and subsequent cleanup. A separate command
construction test asserts store flags for parsing, evaluation, and version
construction, including selected evaluation, along with removal of host
store-selection variables. Literal-selection tests cover quotes, interpolation,
and attribute names resembling store-selection flags. This prevents accidental
default-store selection through this helper; it is not filesystem sandboxing.

## Verification

```bash
cargo test --workspace --locked
bash scripts/check-fixtures.sh
cargo fmt --all --check
cargo run --locked --quiet -p rusnix-derive --bin check-rust-spacing
cargo clippy --workspace --all-targets --locked -- -D warnings
```

The suite contains a Rust compile-fail doctest (`E0308`), a valid enum-bearing
configuration, a bad port constraint, a nested division failure, IR path
conflicts, generated syntax/static-binding failures, and an explicitly unmapped
error. Tests also cover lazy-context loss, structured/text/source-map adapters,
escaping/interpolation, integer boundaries, store isolation, and CLI artifacts.
Selective scalar/list evaluation, cloned-expression path recovery, and a live
source-map-only failure are covered too. Committed [diagnostic snapshots](tests/snapshots/)
compare the stable Rust-facing diagnostic surface, including actual source
locations and underlying reasons. Missing Nix tooling
fails tests instead of silently skipping the central experiment.

The fixture script reruns snapshots and saves all CLI cases under
`target/diagnostic-fixtures/`. Failed fixtures are expected and their diagnostic
kinds are checked. Rust's type failure is covered by `cargo test` rather than
being passed to Nix. The [UI expectation manifest](tests/ui/expected.json) is the
source of truth for compile-fail cases and expected errors. Its harness compiles
the original fixtures through Cargo with locked, offline dependencies and the
active target, profile and compiler flags; it checks codes, primary spans and
causal labels rather than full compiler wording.
Cargo's first dependency acquisition may need network access; once cached,
append `--offline` to Cargo commands. Nix expressions never fetch network data.

The selective-evaluation experiment proved local operation provenance without
eagerly forcing enclosing values; the module experiment extends it to NixOS
definitions, assertion messages, and a specific imported-module boundary. Core
provides generic package interop; package policy lives in the examples. A full
NixOS API, package-specific core bindings, flake APIs, derivation building,
deployment and alternate backends remain outside this experiment.

## NixOS module experiment

The module frontend lives in `rusnix-ir/nixos.rs`; lowering, pinned source staging,
and module-specific diagnostic translation live in `rusnix-nix/nixos.rs`.
The existing IR, AST, renderer, source maps, and subprocess boundary are reused.

```rust
use rusnix_ir::{Config, nixos::NixosModule};
// Explicit generic access, without a Rust wrapper for the service.
let module = NixosModule::empty()
    .add(Config::new()
        .set("services.openssh.enable", false)
        .set("services.openssh.ports", vec![22]))
    .import("nixos/modules/services/networking/ssh/sshd.nix");
```

`NixosModule` also exposes `.assertion(name, Expr<bool>, message)` and
`.import(path)` for a validated path relative to the pinned source tree.
General ecosystem objects use `.import_ref(ModuleRef)`. Core does not enumerate
upstream modules. The legacy test fixtures define their own tiny OpenSsh helper;
its `enable` takes `bool` and `ports` takes `Vec<u16>`. The generic
`Config::set` remains an explicit escape; the type fixture uses it to supply
strings to the genuine upstream integer-list option. Local typed setters capture
caller locations through to their semantic assignments. IR module imports and
assertions have their own origins; no Nix syntax is exposed by this API.
Assertion conditions and generated imports receive scoped IR validation before
lowering, so escaped callback parameters and invalid paths produce validation
diagnostics with Rust origins.

**Evaluation surface:** pinned nixpkgs commit
`8b27c1239e5c421a2bbc2c65d52e4a6fbf2ff296` (24.11), with its complete library
and four modules staged from the unmodified upstream submodule. See
`vendor/README.md` and `vendor/nixpkgs-pin.json`. The backend reads the revision
and file hashes from that manifest. Every listed file is SHA-256 checked once per
process, then copied once into each disposable session. The library is imported
from there offline.

`nixos-driver.nix` calls actual `lib.evalModules`, imports the upstream assertions
option declaration and generated module, and selects `result.config` using an
escaped list of attribute names. `specialArgs.pkgs = {}`; seven freeform
support namespaces accommodate OpenSSH's incidental definitions. OpenSSH's
`services.openssh` types and unknown-option checks remain real. This is a
partial NixOS module evaluation harness, not evaluation of `nixosSystem` or
all NixOS modules. The original minimal mode cannot select package/service implementation values;
interop mode below supplies real pkgs and selected package metadata.
The real `label.nix` default fails because its normally imported version module
is deliberately absent. Label-related environment overrides are cleared.

The assertion declaration module does **not** enforce assertions. Enforcement
normally lives in system-building `top-level.nix`. The driver copies its tiny
failed-assertion filtering and throw policy, enabled only by `checkAssertions`;
it never imports the builder. Thus the assertion case proves provenance through
the real assertion option/merge machinery and the same enforcement policy,
not through full system top-level evaluation.

All actual subprocesses still use the single isolated factory:

```text
nix-instantiate --store <session>/store [isolated flags] --parse <session>/module.nix
nix-instantiate --store <session>/store [isolated flags] --parse <session>/generated.nix
nix --store <session>/store [isolated flags] --offline eval --json
    --eval-store <session>/store --file <session>/generated.nix
```

The wrapper imports the driver with `nixpkgs = ./nixpkgs; module = ./module.nix;`
and `selection = [ "services" "openssh" "ports" ];`. No Nix fetch, build,
activation, or host-store command was used. The original store-construction and
physical-write tests remain. A new test checks staged module files, isolated
store database, and session cleanup.

**Module provenance:** NixOS evaluates an option's type after a value-level context
would have unwound; unknown options can fail without a useful generated frame.
The compiler therefore emits **one inline imported module per assignment**:

```nix
{
  "_file" = "rn-cb0f3390b6cbe7a2";
  "config" = {
    "services"."openssh"."ports" = [ "twenty-two" ];
  };
}
```

NixOS carries `_file` into definition error messages. Rusnix resolves its marker
to the exact Rust `.set()` and option path. The actual raw type error includes:

```text
A definition for option `services.openssh.ports."[definition 1-entry 1]"' is not of type `16 bit unsigned integer; between 0 and 65535 (both inclusive)'. Definition values:
- In `rn-cb0f3390b6cbe7a2': "twenty-two"
```

The displayed reason replaces the marker with the Rust location; raw stderr is
retained unchanged. This avoids blaming the list container or imported SSH code.
Assertion messages carry `[rn-<16 hex digits>]`; the driver adds one scalar
`addErrorContext "rusnix-stage:nixos-assertions"` around the failed-assertion throw.
The marker maps to `.assertion()` and `assertions.<name>`, and is removed only
from the displayed reason. Expression failures use the boundary/source-map
policy described above; no generated `deepSeq` or broad forcing was added.

| Fixture | Observed reason | Rust mapping | Lost precision |
| --- | --- | --- | --- |
| `type` | ports entry is not a 16-bit unsigned integer | `.set`, `services.openssh.ports`, definition marker | individual list-entry Rust point |
| `unknown` | `services.openssh.rusnixMissing` does not exist | introducing `.set`, definition marker | no value-level runtime context |
| `assertion` | Failed assertions: SSH port policy rejected | `.assertion`, message marker | false boolean itself did not throw |
| `external` | attribute `version` missing inside `label.nix` | `.import(NixosLabel)`, external trace frame | no exact upstream-causal Rust expression |
| `lazy` | ports nested division by zero | `.divide`, generated source span + static ancestry | none in the tested path |

Run reviewable fixtures (the four error fixtures exit 1):

```bash
cargo run --locked -p rusnix-cli -- check-nixos good --out target/nixos/good
cargo run --locked -p rusnix-cli -- check-nixos type --out target/nixos/type
cargo run --locked -p rusnix-cli -- check-nixos unknown --out target/nixos/unknown
cargo run --locked -p rusnix-cli -- check-nixos assertion --out target/nixos/assertion
cargo run --locked -p rusnix-cli -- check-nixos external --out target/nixos/external
cargo run --locked -p rusnix-cli -- check-nixos lazy --out target/nixos/lazy
cargo run --locked -p rusnix-cli -- check-nixos lazy --out target/nixos/lazy-bad --select services.openssh.ports
```

Artifacts are `module.nix`, `module-map.json` (source spans and definition/import/
assertion tables), `nixos-driver.nix`, `evaluation.nix`, and result/diagnostic/raw
files. Paths in these generated snippets refer to the helper's staged tree;
the output directory alone is not a standalone Nix environment. Re-evaluate
through `NixSession::evaluate_nixos`, preserving isolation.

Selecting `enable` from the lazy fixture returns `false`; selecting `ports`
fails at `.divide()` while preserving `services.openssh.ports`. Captured real
module traces also map via source spans when runtime frames are removed.
Assertions remain unevaluated unless explicitly checked. Normal NixOS structural
unknown-option checks may reject a module even when another option is selected;
this is upstream module behavior, not Rusnix global value forcing.

`DiagnosticKind` distinguishes Rust, validation, compiler, NixEval, NixosModule,
NixosType, NixosMerge, NixosAssertion, ExternalNix, and Tooling. Rust compilation failures
are exercised by UI fixtures and generic compile-fail doctests; the CLI does not ingest rustc diagnostics.
JSON Nix traces still contain **no structured exception category or option-path
field**. Type/unknown classification combines a verified `lib/modules.nix`
frame with narrow reason patterns; option paths otherwise come from option-trace
messages or static ancestry. These upstream text fragments and `_file` formatting
remain brittle. Module-specific external-frame classification currently requires
structured traces; legacy-text adaptation has only the existing plain-eval coverage.
Owned assertion stage/message markers are stronger evidence. Marker messages are
metadata, not an authenticated diagnostic protocol.

Imported runtime-frame boundaries work for listed directly imported files.
The multi-origin experiment below also uses upstream definition filenames and
retains external filenames when no Rust boundary matches. Transitive causal
dependency graphs remain unproven. Hashing/staging adds a
small dependency (`sha2`) and assumes the initialized submodule is available relative
to the build workspace. The harness does not prove installed binary portability.
One module per definition adds scaffolding without eager forcing. The core
design remains Rust IR → backend AST → source map/metadata.

## Multi-origin module diagnostics

The final provenance experiment composes independent `NixosModule` values:

```rust
pub fn a() -> NixosModule {
    NixosModule::new(Config::new().set("services.openssh.authorizedKeysCommandUser", "root"))
}

pub fn b() -> NixosModule {
    NixosModule::new(Config::new().set("services.openssh.authorizedKeysCommandUser", "nobody"))
}

pub fn c() -> NixosModule {
    NixosModule::new(Config::new().set("services.openssh.authorizedKeysCommandUser", "sshd"))
}
let config = NixosModule::new(Config::new())
    .import("nixos/modules/services/networking/ssh/sshd.nix").module(a()).module(b());
```

Each child validates its own `Config`; duplicate paths *across* modules are left
for NixOS to process. Lowering recurses into the AST and emits nested imports,
never concatenated generated source. Definition tables aggregate all child
origins. Each assignment retains its `_file` identity and static ancestry.
For A and B, the generated definitions include:

```nix
{
  "_file" = "rn-85c0e94174be752e";
  "config" = {
    "services"."openssh"."authorizedKeysCommandUser" = "root";
  };
}
{
  "_file" = "rn-4da1cd4d48cf31f8";
  "config" = {
    "services"."openssh"."authorizedKeysCommandUser" = "nobody";
  };
}
```

Both definitions evaluate successfully alone. Combined, the real upstream
string option's `mergeEqualOption` rejects them. Its raw reason is:

```text
The option `services.openssh.authorizedKeysCommandUser' has conflicting definition values:
- In `rn-4da1cd4d48cf31f8': "nobody"
- In `rn-85c0e94174be752e': "root"
Use `lib.mkForce value` or `lib.mkDefault value` to change the priority on any of these definitions.
```

Rusnix renders both source points, with source excerpts/carets and role labels:

```text
error[nixos-merge]: conflicting definitions for a NixOS option

  --> crates/rusnix-cli/src/merge_fixtures.rs:12:36
   = origin: set services.openssh.authorizedKeysCommandUser
   = conflicting definition

  --> crates/rusnix-cli/src/merge_fixtures.rs:9:36
   = origin: set services.openssh.authorizedKeysCommandUser
   = conflicting definition

   = option: services.openssh.authorizedKeysCommandUser
```

The complete output also preserves NixOS's reason, replacing generated marker
filenames with Rust locations. Full golden outputs, including source excerpts
and reasons, live in `tests/snapshots/merge-*.txt`.

| Case | Definitions / actual result | Recovered causal sources |
| --- | --- | --- |
| `merge-two` | command user A=root, B=nobody; conflict | B and A, from `_file` markers |
| `merge-ok` | ports A=`[22]`, B=`[2222]`; result `[2222,22]` | successful merge, no error invented |
| `merge-three` | command user A=root, B=nobody, C=sshd; conflict | C and B only, exactly as NixOS reports |
| `merge-three-type` | ports=`"invalid-a"`, `"invalid-b"`, `"invalid-c"`; type failure | all three reported definition origins |
| `merge-mixed` | generated default label=`"rusnix-label"`, upstream default label=`"24.11"`; conflict | generated `.set()` plus Rust `.import(NixosLabel)` and upstream `label.nix` filename |
| `merge-priority` | A=mkDefault(root), B=nobody, C=mkForce(sshd) | success: `"sshd"`; NixOS chooses the winner |

**Three-definition limit:** pinned `lib/options.nix:284` folds definitions until
its first unequal pair and calls `showDefs [ first def ]`. The three valid
string definitions produce only C/B markers; A never appears in the error.
Rusnix does not accuse A from its mere presence in the artifact. In contrast,
the real module type-check failure's `allInvalid` list reports all three invalid
ports definitions. All three are retained, rendered, and JSON-round-tripped.
This proves a diagnostic set larger than two without claiming the merge error
contains a complete participation graph.

**Mixed case:** Rust first provides `system.nixos.version = "24.11"` so the real
label module's default can evaluate. It imports `NixosLabel`, then adds another
module setting `system.nixos.label = "rusnix-label"` at default priority.
Upstream `label.nix` also defines the label with `lib.mkDefault`; equal priorities
leave both values for the genuine string merge check. The important raw lines
are (the temporary prefix is abbreviated here):

```text
The option `system.nixos.label' has conflicting definition values:
- In `<session>/nixpkgs/nixos/modules/misc/label.nix': "24.11"
- In `rn-5c963c20bc78fbd8': "rusnix-label"
```

The final output identifies `merge_fixtures.rs:34:14` as the imported boundary,
`nixos/modules/misc/label.nix` as the upstream definition, and
`merge_fixtures.rs:36:48` as the conflicting Rust setter. Definition metadata
has a filename, not the upstream assignment line. No Rust version-setting
origin is inferred from the upstream default's dependency; this is boundary
provenance, not a dependency-graph reconstruction.

**Priority semantics:** the experiment adds only a module-local
`DefinitionPriority::{Normal,Default,Force,Override(u16)}` for Config assignments.
It emits structured AST attribute sets matching pinned `lib.mkOverride`'s
actual payload (`_type="override"`, `priority`, `content`). Default=1000 and
Force=50 match the pinned library; Normal remains unwrapped. NixOS performs all
priority filtering. Tests verify ordinary definitions beat defaults, force beats
ordinary, and override 40 beats force 50. A losing division-by-zero expression
remains unevaluated. Assertions and child modules keep their own policies.

**Diagnostic contract:** `origins: Vec<DiagnosticOrigin>` is now the authoritative
causal set. Each source records optional Rust `Origin`, `OriginRole`, its own
`Provenance`, and optional upstream `nix_file`. Roles are Primary,
ConflictingDefinition, ContributingDefinition, and ImportedBoundary. Optional
Rust locations allow external definitions to survive without a known Rust import.
`primary` remains a compatibility convenience; it prefers the first reported
Rust definition, while `related` still supports semantic ancestry. Single-origin
rendering and old snapshots remain simple and unchanged. Multiple assertion
messages now also preserve every matching Rust origin.

**Extraction:** Nix's internal JSON still has `raw_msg` and `trace`; it has no
structured list of contributing definitions. A dedicated adapter in
`diagnostic.rs` reads recognized pinned NixOS reason prefixes and anchored
`- In \`FILE':` definition lines from `raw_msg`, not from the decorated `msg`.
Module classification additionally requires structured module-system frame
evidence. `_file` is sufficient to map the definitions *reported* in these
errors; it neither preserves every participant nor turns metadata into a stable
machine protocol. Per-origin roles come from Rusnix's classification, not Nix.

Limitations: option paths and definition lines remain upstream text conventions;
quoted attribute names or changed quote/indentation rules may defeat the adapter.
There is no promise for read-only/unique-option merge formats not exercised here.
The legacy text adapter still extracts one underlying error line, so these new
multi-definition mappings currently require the tested structured Nix format.
Unknown marker IDs, transitive files without a matching direct import, and repeated
constructor call sites remain limitations. Repeated sites can share origin IDs
and `_file` markers; this is source identity, not unique module-instance identity.
All original raw diagnostics remain retained unchanged. Single `primary` users
must migrate to `origins` to avoid discarding contributors.

An unselected conflicting option leaves selected ports usable. The generated
modules retain lazy evaluation, operation contexts and source-map ancestry.
Run cases with `check-nixos <fixture> --out <directory>`, or run
`scripts/check-fixtures.sh` for diagnostic snapshots and reviewable artifacts.
The [typed showcase](examples/README.md) demonstrates frontend invariants that
reject invalid configurations before Nix evaluation.

## References

- [Nix local stores](https://nix.dev/manual/nix/2.34/store/types/local-store.html)
  documents physical store rooting.
- [Nix builtins](https://nix.dev/manual/nix/2.34/language/builtins.html)
  documents deep forcing and error contexts.
- [Nix 2.34.8 JSON logger implementation](https://github.com/NixOS/nix/blob/2.34.8/src/libutil/logging.cc)
  defines the observed `raw_msg`, `trace`, and position serialization.
- [nix-instantiate](https://nix.dev/manual/nix/2.34/command-ref/nix-instantiate.html)
  documents the parse-only operation.


## Existing Nix ecosystem interoperability

The [interop example](examples/README.md#nix-interop) authors references to pinned
nixpkgs `hello` and `python312Packages.requests`, an opaque upstream OpenSSH
module alongside typed Rust definitions, `lib.toUpper`, a local
Nix overlay, and a local external-input-shaped package/module set. No package
bindings or metadata are duplicated in Rust. The example prints generated Nix;
integration tests evaluate these objects and also exercise `lib.getName`. Run:

```bash
cargo run --locked -p rusnix-nix --example nix-interop
cargo test --locked -p rusnix-nix --test interop
```

The [overlay example](examples/overlay/README.md) authors ordinary
`final: prev: { ... }` customization code in Rust. It appends `--disable-dict`
through `prev.curl.overrideAttrs`, keeping the existing pinned nixpkgs package
definition. Existing deferred callbacks and nixpkgs' `extend` suffice; Nix
evaluates the fixed point. Tests compare complete recipes with the handwritten
overlay, including the ordinary downstream `curlpp` dependency, and verify
`final`/`prev`, laziness and Rust operation provenance. The package examples
replace package definitions; this example replaces overlay/customization code.

`rusnix_ir::interop` owns distinct PackageRef/ModuleRef/OverlayRef/NixFunction
handles, InputRef, and an explicit NixValue escape hatch. A reference carries its
source identity, structured attribute segments, and Rust lookup origin. It
lowers to AST imports, escaped getAttr operations, and function applications.
Nix owns existence, object schemas, overlay semantics, and actual package type
checking. `NixosModule::system_packages` accepts only PackageRef; `import_ref`
accepts only ModuleRef. Typed user-defined models can coexist with these handles.

Interop uses the full `vendor/nixpkgs` submodule at the existing revision,
verified against the expected Git commit and checked for modified or untracked
files before use. Sessions link the checkout as `nixpkgs-full`; evaluation never
fetches or updates it. The original minimal tests retain their small staged source
subset. Full package roots use explicit `system = "x86_64-linux"`, `config = {}`, and handle-scoped overlays;
this is not yet a platform or nixpkgs-configuration API. The evaluator imports
upstream `config/system-path.nix` to get the real systemPackages schema, forces
only that option's definition priority to exclude unrelated full-system defaults,
and projects package name/version/type metadata. It neither selects system.path
nor serializes whole derivations. Evaluation may materialize `.drv` records in
the isolated store; packages are never built. The helper additionally disables
import-from-derivation, and every Nix process retains explicit disposable store
and matching eval-store selection.

Runtime contexts on imported lookups/calls supplement source spans on ordinary
selections, preserving immediate opaque-boundary failures. Imported module functions retain file boundaries through existing trace
and `_file` adapters; a small guard uses upstream types.deferredModule.check to
catch a category-invalid module before NixOS loses its location. It inspects the
head only. If an opaque function returns a lazy container whose child later
throws, the original call context has unwound: the tested explicit `.select()`
reports the selection origin, not the original call. Deep failures without a
matching directly imported file or consuming generated span can remain unmapped.
There is no new diagnostic parser or broad forcing. Full raw traces remain saved.

### Structured opaque calls

`NixValue::literal`, `null`, `list`, and `record` construct structured semantic
IR. Records accept an iterator of literal keys and values, including a
`BTreeMap<String, NixValue>`; keys are escaped names, never dotted option paths.
An opaque record in a derived contribution remains one value instead of being
flattened into option definitions. Duplicate keys and NULs are IR errors.
Primitive values, Expr leaves and opaque handles support explicit `.into()`;
`Option<T>` converts Some through the same boundary and None to null. This is
boundary conversion, not a general serialization framework or Option derive.

```rust
let pkgs = Nixpkgs::new();
let args = NixValue::record([
    ("name", "example.conf".into()),
    ("text", rusnix_ir::nix_text!("workers = {workers}\n", workers = 4)),
    ("executable", false.into()),
    ("passthru", rusnix_ir::nix_record! {
        "package": pkgs.get("hello"),
        "labels": NixValue::list(["a".into(), "b".into()]),
    }),
]);
let file = pkgs.pkgs_function("writeTextFile").call(args);
let curried = pkgs.pkgs_function("writeText")
    .apply(["postgresql.conf".into(), "workers = 4\n".into()]);
```

`pkgs_function` looks up functions in the real package set, retaining its
overlays; the existing `function` looks in nixpkgs/lib. Each `.call` is ordinary
Nix application; `.apply([args...])` handles currying on either NixFunction or
NixValue and preserves the Rust application call site. Results stay `NixValue`,
without automatic PackageRef inference. Existing package functions/overrides can also be
selected through `as_value().select(...)` and called. Function schemas and errors
belong to Nix. No builder or package-specific Rust code is involved.

`PackageFunction<R>` describes a nixpkgs package definition with a result interface:
typically a function such as `{ stdenv, lib, openssl, ... }: stdenv.mkDerivation { ... }`, whose named
arguments are dependencies and feature options. Construct it with
`PackageFunction::from_function_attrs(arguments, build)`, using the same lazy
named defaults and body construction as `NixValue::function_attrs`.

```rust
use rusnix_ir::{Config, Expr, interop::{NixValue, Nixpkgs, PackageFunction}};

let factory: PackageFunction<Expr<String>> =
    PackageFunction::from_function_attrs(["curl", "label"], |args| {
        (
            vec![("label", args.clone().select("curl.pname"))],
            args.select("label").into_expr(),
        )
    });
let result = Nixpkgs::new().call_package(&factory, NixValue::record([] as [(&str, NixValue); 0]));
let config = Config::new().set("factory", factory).set("result", result);
```

`call_package<R>(&PackageFunction<R>, impl Into<NixValue>) -> R` represents the
real pinned `pkgs.callPackage factory overrides`. nixpkgs inspects the argument
names and supplies matching dependencies; explicit caller arguments take
precedence. Rust does not reimplement dependency injection. Package recipes return
`PackageFunction<Package>`; families may return `PackageFunction<NixAttrs<Package>>`.
The default result type is still `NixValue` for dynamic callers. These are declared
symbolic interfaces; Nix validates the actual arguments and result when demanded.
Packages retain real nixpkgs `.override` and `.overrideAttrs` behavior through
`override_arguments` and `override_attrs`.

`try_call_package` accepts any `IntoRusnixValue` argument record and returns
structural conversion errors. Authors can pass derived structs containing `Package`
dependencies directly, without lowering them before the call. `NixExpression::bind`
keeps a factory or package's Rust interface on its lazy lexical parameter:

```rust
let family = factory.bind(|factory| {
    NixAttrs::new([("member", pkgs.call_package(&factory, arguments))])
});
```

Here the callback parameter is still a `PackageFunction<R>`, and the family keeps
its member interface. No `as_value` conversion or raw callback parameter is needed.

The old `package_function` selector was renamed to `pkgs_function`, because it
selected arbitrary helpers from `pkgs`, including curried functions and
`callPackage` itself. Those are generic `NixFunction` references, not package
definitions. `Nixpkgs::function` still selects functions from `lib`.
`PackageFunction` wraps the existing deferred function expression rather than
changing the reference-only `NixFunction` representation. Only a
`PackageFunction` can be passed to `call_package`; generic functions and
unmarked `NixValue` expressions fail at Rust compile time.

`PackageFunction::as_value()` and `From<PackageFunction<R>> for NixValue` remain
escape hatches for dynamic inspection such as `functionArgs`. Ordinary calls,
records and bindings accept the typed factory directly. The existing
`ConfigValue` and `IntoRusnixValue` conversions also accept package functions in
`Config::set` and derived configuration fields. The wrapper and `call_package`
forward source locations into the existing function and application machinery:
construction, missing-dependency call boundaries, body operations, and caller
argument failures retain the current diagnostic policy. External nixpkgs frames
remain subject to the existing provenance limitations. No new forcing
is added; unused defaults/dependencies and unselected branches stay lazy.

Use derived Rust structs for meaningful fixed schemas and `NixValue::record`
arrays/iterators for open records. A derived value's `try_into_nix_value()?`
converts it to one Nix record without expanding it into option definitions.
`RusnixValue` is the structural intermediate used by custom conversion
implementations; normal authors need not handle it. Conversion can fail when a
`flatten` field produces a scalar, null, or opaque value instead of a structural
record, even on a derived type. Other IR checks happen during compilation, and
Nix still checks function arguments and NixOS option types during evaluation.
`nix_record!` stays useful for tiny records; it accepts literal keys, key variables
or parenthesized dynamic key expressions, never raw Nix source.

`nix_text!("postgres --port={port}", port = port)` interpolates named values into
a deferred Nix string. Arguments may be Rust literals, Expr/OptionRef expressions,
opaque Nix values or package/derivation handles. Each argument is constructed once
and coerced with Nix `builtins.toString`; repeated holes reuse its graph. This is
symbolic interpolation, not Rust `format!`. Nix string dependency contexts and
child origins survive through the existing IR and text concatenation operations.

Templates beginning with a newline use block dedenting: remove that first newline,
discard the indentation-only closing line, and strip the exact common space/tab
prefix of nonblank lines. Relative indentation, blank lines and content newlines
remain; tabs are not expanded. Other templates stay verbatim. Interpolated values
are never dedented or reindented.

```rust
let command = rusnix_ir::nix_text!(
    r#"
        if test -d {data}; then
          echo ready
        fi
    "#,
    data = data,
);
```

Keep the closing delimiter on the last content line to omit a trailing newline.
Escape literal braces with `{{` and `}}`
(for example, `${{PATH}}` produces literal shell `${PATH}`). Only explicit named
arguments are supported; unknown holes, malformed braces, duplicate/unused
arguments and formatting syntax produce compile-time errors. Width, precision,
debug formatting, positional arguments and expressions inside holes are unsupported.

The original `nix_text!(part, other_part, ...)` fragment form remains available;
its parts must already be strings, with explicit `.to_text()` for other values.
A single string literal is now a template and follows the brace-escaping rules.
`NixValue::concat_text(parts)` is the iterator form, and
`NixValue::join_text(separator, opaque_list)` also accepts lists produced by
deferred Nix callbacks. None of these operations evaluates deferred values in Rust.

NixOS definition helpers live in `rusnix_ir::nixos`: `nixos::merge(values)` emits
`mkMerge`; `value.when(condition)` emits `mkIf`; `value.priority(priority)` uses
the existing DefinitionPriority enum; `.before()` and `.after()` emit ordering
metadata. NixOS decides merge, priority and ordering semantics. `.when` describes
a deferred module definition; `NixValue::if_else` selects a plain value. Neither
performs eager forcing. Each `NixosModule::add` still creates an independent
contribution. PostgreSQL uses these helpers locally without adding domain types
to core; settings/SQL formatting, option dependencies and nullable file handling
remain in its compatibility adapter.

References embedded in structures remain Nix lookups. Expr/OptionRef leaves
retain their nodes, contexts and origins: a structured `text` argument can
contain `config.services.postgresql.dataDir`. The generated-file smoke test
evaluates one artifact with `/var/lib/postgresql`, then changes only an ordinary
Nix module to force `/srv/postgresql`; both the final text and derivation path
follow the override. Its tiny option schema is test data, not a PostgreSQL rewrite.
Unused calls and nested fallible values remain lazy. Immediate call failures
identify the Rust call; nested expression failures retain their operation origin.

[structured_interop.rs](crates/rusnix-nix/tests/structured_interop.rs) exercises
writeText, writeTextFile, runCommand, lib.recursiveUpdate and lib.isFloat with
mixed values, arbitrary escaped keys, scopes, laziness and failure provenance.
It evaluates derivation attributes/paths in disposable stores and verifies that
outputs remain absent. It never builds or reads generated-file contents: the
tested `text` is the derivation's evaluated input. Reviewable artifacts live in
`target/structured-interop/`. Nix JSON coerces values with `outPath` to strings;
metadata projections deliberately use `output` as their own key.

Integers retain Nix's signed 64-bit semantics. Float leaves are Rust f64 values,
rendered with a decimal point even when integral. NaN, infinities and subnormal
f64 literals are rejected before code generation: the tested Nix parser rejects
subnormals (for example `5.0e-324`). Finite normal values and signed zero are
supported, including the tested minimum normal and maximum f64. Nix numeric/JSON
semantics remain authoritative; this is not a decimal or arbitrary-precision API.

This proves a small interoperability surface, not universal compatibility with
all packages/modules/flake schemas. InputRef imports a local Nix value; it is not
a flake evaluator. Handle categories are caller declarations, checked against
actual Nix values at evaluation. A PackageRef pointed at a module is rejected by
the upstream NixOS package type; explicit as_value and raw Config/IR remain escape
hatches. Overlays affect lookups on their Nixpkgs handle, not the module driver's
separately supplied pkgs or a NixOS nixpkgs.overlays option. Sources are currently
filesystem-relative compiler artifacts, local input paths, and one fixed pinned
package root. Rust-origin IDs and multi-origin diagnostics remain unchanged.


Examples print generated source without Nix evaluation or artifact writes.
Tests reuse the actual authoring models for comparison, combined interop behavior
and symbolic artifact reuse. Executable Nix comparisons live in
`tests/comparisons/`; the local opaque Nix input is
`tests/fixtures/nix-interop-input.nix`.

`IntoConfig::into_config(self) -> Config` is tracked and unsealed.
`NixosModule::empty().add(component)` stores a separate child module for each
component; bindings are never flattened across components. Config retains
its duplicate-path validation and also implements IntoConfig. Explicit
per-contribution priority uses
`module(NixosModule::new(component.into_config()).priority(...))`.
Conversion-generated bindings identify the into_config/add authoring call;
untracked adapter helpers stop caller forwarding, so tracked lowering helpers
remain important. During priority filtering Nix can inspect the head of an
unwrapped definition; an override wrapper keeps its discarded content lazy.

Domain models belong to the examples. Fixture-only SSH/module helpers live in
`tests/support/nixos.rs`. Pinned imports carry validated relative paths as data,
rendered through the Nix AST with escaped string addition. UI fixtures compile
actual example-owned types or deliberately evolved test-local enums and check
codes, spans and type labels. See [verification](#verification) for the shared
checks and authoritative fixture manifest.

## Explicit symbolic NixOS dependencies

Run the single-file [symbolic-option example](examples/symbolic-option.rs) with
`cargo run --locked -p rusnix-nix --example symbolic-option` to see a concrete
Rust command and generated symbolic module. Tests verify the default and ordinary
Nix override against the same generated artifact; the example does not invoke Nix.

A concrete Rust value is computed before lowering. A
`rusnix_ir::nixos::OptionRef<T>` instead declares a dependency resolved by NixOS
after merging. It has no operation to read or resolve the value in Rust.
The author's expected type controls expression composition; it does not prove
the schema of a string-named NixOS option. NixOS remains authoritative there.

```rust
// Types are defined in examples/symbolic-option.rs; nesting determines placement.
let port = OptionRef::<i64>::new("services.example.port");
let command = port.into_expr().to_text().with_prefix("example --port=");
let service = ExampleService {
    services: Services { example: ExampleOptions { enable: true } },
    systemd: Systemd { services: Units { example: Unit {
        service_config: ServiceConfig { exec_start: command },
    } } },
};
let module = NixosModule::empty().add(service);
```

For compatibility adapters, `#[rusnix::options]` replaces repeated accessor code
with a local declaration of finite dependencies. `#[rusnix::config]` declares
what a module defines; `#[rusnix::options]` declares which final options it
symbolically depends on. Neither creates upstream option schemas or bindings.

```rust
#[rusnix::options]
mod options {
    #[rusnix(root)]
    struct Root { services: Services }

    struct Services { postgresql: Postgresql }

    struct Postgresql {
        enable: bool,
        data_dir: String,
        #[rusnix(rename = "enableJIT")]
        enable_jit: bool,
        settings: Settings,
    }

    #[rusnix(value)]
    struct Settings { port: i64, jit: String }
}
let pg = options::root().services.postgresql;
let command = pg.settings.port().to_text().with_prefix("--port=");
let complete_settings = pg.settings.as_value();
```

The macro generates public navigation fields and tracked accessor methods inside
the annotated module. Its declarations are reference descriptions, not concrete
Rust data structs. The enclosing module controls visibility. Scalar leaves
`bool`, `String`, and `i64` return their existing Expr types. Core expression
handles such as `Package`, `Stdenv`, `NixLibrary`, `NixCallable<R>`,
`Overridable<T>`, `NixAttrs<T>`, `NixList<T>` and `PackageFunction<R>` retain
their declared Rust interfaces. NixValue, Option<T>,
Vec<T>, BTreeMap<K, V> and HashMap<K, V> return opaque NixValue references; Rust
never receives their deferred contents. A nested local struct creates a view;
only one marked `#[rusnix(value)]` exposes the whole subtree. Roots cannot have
that marker or method. Names reuse the same lowerCamelCase/PascalCase/rename
rules as configuration lowering. Paths are literal segments, including renamed
keys containing dots. `OptionRef::from_segments(parts)` also exposes this safe
construction directly; the existing dotted `OptionRef::new(path)` remains.

Leaf calls capture their caller, not root construction or navigation. Subsequent
operations retain their own origins. NixOS owns actual option existence, types,
merging and priorities. The first version supports one explicit root in an inline
module containing named view structs and imports. Generic, conditional, recursive
or ambiguous views, unsupported types and external aliases are rejected; aliases
and one-off references can use OptionRef directly. Container derives, skip and
flatten are intentionally unsupported in reference declarations. There is no
runtime traversal or whole-config handle. The [view tests](crates/rusnix-nix/tests/options.rs)
exercise opaque/typed leaves, exact keys, provenance, lazy evaluation and ordinary
Nix overrides of the same artifact; UI fixtures check the compile-time boundary.

`lib` is nixpkgs' utility library, a record of functions separate from the Nix
language's `builtins`. Bind its supported Rust helpers to a caller's library with
`NixLibrary::from_value(inputs.lib.as_value())`. The `optional`, `optionals`,
`optional_text`, `all`, `concat_lists`, version comparisons, output selection and
text replacement methods call that exact library, including caller overrides.
`throw_if_not(condition, message, value)`
validates an expression when Nix evaluates it; it is separate from the NixOS
assertion collection. Arbitrary functions remain accessible through
`lib.as_value().clone().select("makeBinPath").call(packages)`.
Named helpers grow from demonstrated real-world usage; arbitrary nixpkgs `lib`
access remains available through the generic `NixValue` escape hatch.
For the pinned library, explicitly choose `Nixpkgs::library()`.
Boolean `!`, `.and()`, `.or()` and `.implies()` work on both `Expr<bool>` and opaque
`NixValue`, providing lazy boolean operations independent of library overrides.
They check demanded operands as booleans in Nix. `NixValue::concat_lists` joins lists through
`builtins.concatLists`, keeping element values lazy and remaining independent of
`lib.concatLists`. `value.replace_text([(".", "_")])` likewise uses the builtin;
`lib.replace_text(value, pairs)` uses the supplied library. Replacement pairs
preserve order and Nix's replacement rules. `lib.version_at_least(version, minimum)`
and `lib.version_older(version, other)` call the supplied library's comparisons.
`lib.get_dev(package)` and `lib.get_lib(package)` retain its output fallback and
explicit-output semantics. These operations preserve child origins and Nix string
dependency context rather than computing in Rust.

`value.has_attr(name)` and `value.attr_or(name, fallback)` use builtins with one
literal attribute name, including names computed by Nix. Dots are part of the
name, rather than a path. A present null value stays null; unselected values and
fallbacks remain lazy. `package.override_args(changes)` and
`package.override_attrs(update)` call that value's existing override functions.
Records and deferred callbacks retain nixpkgs' default, dependency-splicing and
recursive final-attribute behavior; Rust does not reconstruct the package.

For package-function adapters, `#[rusnix::args]` provides the same finite structural
navigation over a supplied deferred argument record. Bind it with
`args::from_value(arguments)` inside `PackageFunction::from_function_attrs`
(or the generic `NixValue::function_attrs`); accessors such as
`args.stdenv.host_platform.is_darwin()` construct symbolic selections. Scalar and
opaque leaf mappings, naming rules and explicit subtree access match
`#[rusnix::options]`. Rust declares expected shapes; it never reads argument values
or validates the external function's schema. Roots have no dynamic traversal or
whole-root accessor; retain the raw NixValue for advanced access. An argument
subtree marked `#[rusnix(value)]` also implements `NixExpression`, allowing typed
bindings and `as_attrs()` for deferred shallow record union. External aliases
and reusable views use explicit lower-level selection rather than source inspection.

Generated `args::argument_names()` returns the mapped names of the root's direct
fields in declaration order. The Git, curl, OpenSSL and MariaDB examples declare
complete public interfaces and pass these names to `from_function_attrs`, avoiding
duplicate argument-name lists. Partial views return only the fields they declare;
they do not discover undeclared arguments. Defaults and requiredness remain
explicit in the native function builder.

The [Git argument declaration](examples/git-nixpkg/inputs.rs) exercises this API without
changing defaults, `functionArgs`, `callPackage`, `.override` or `.overrideAttrs`.
The [argument-view tests](crates/rusnix-nix/tests/args.rs) verify literal path keys,
caller provenance, laziness and ordinary Nix callers reusing the same artifact.

Native argument selections lower directly to lexical bindings such as
`stdenv.hostPlatform.isDarwin`, including in dependent defaults. Whole-record uses
and outer arguments shadowed by nested named functions retain a lazy record
capture; references that escape their callback scope are rejected. Argument lookup
origins and source-map ancestry remain intact; ordinary lookup failures use
generated positions.

`NixValue::select_segments` preserves literal dots within keys, and
`NixValue::into_expr::<T>()` attaches a supported expected scalar type while
retaining the existing deferred expression and origin.

The [symbolic dependency tests](crates/rusnix-nix/tests/symbolic_options.rs) add
an ordinary Nix option-declaration fixture and an independent default-priority
port contribution of 5432. Each contribution retains its own `_file` identity.
Only contributions containing references become `{ config, ... }:` functions;
existing concrete modules retain their generated representation.
The generated dependency, omitting diagnostic comments and retained boundary
wrappers for readability, is:

```nix
systemd.services.example.serviceConfig.ExecStart =
  "example --port=" + builtins.toString config.services.example.port;
```

String addition plus `builtins.toString` expresses the dependency without raw
Nix-source interpolation. Attribute paths and concrete text are escaped data.
An ordinary Nix contributor can then define:

```nix
{ services.example.port = 6432; }
```

The test evaluates **the same compiled artifact** before and after replacing
only that ordinary Nix input: `"example --port=5432"` becomes
`"example --port=6432"`. No Rust conversion or compilation is repeated.
Another ordinary module's `lib.mkForce 7432` wins over both the normal definition
and Rusnix's default. NixOS alone selects the final value.

[Symbolic option tests](crates/rusnix-nix/tests/symbolic_options.rs) cover the base
value, unchanged-artifact override, priority selection,
the same symbolic text accepted by the real upstream OpenSSH `banner` option,
division-by-zero provenance and source-map fallback, missing-reference provenance,
lazy selection, bool/string/list uses, validation and escaped paths.
`tests/ui/symbolic-integer-as-boolean.rs` checks E0308 with a primary span and
`Expr<bool>`/`Expr<i64>` labels. An integer dependency cannot be an assertion
condition.
An unused command dependency leaves a throwing port unevaluated; selecting the
command demands it and maps the failure to the Rust reference. No `deepSeq` or
global forcing is generated.

The evaluation surface is still the existing pinned `lib.evalModules` harness:
`services.example` has fixture-declared types, while the systemd namespace is a
freeform placeholder. The separate OpenSSH test checks a genuine upstream option
type. This proves module fixed-point composition, not complete systemd validation
or service execution. Artifacts (generated Nix, values, Rust diagnostics and raw
Nix diagnostics) are saved under `target/symbolic-options/`.

The supported expression types are currently `i64`, `bool`, and `String`.
Only integer-to-text and concrete string-prefix operations were added; references
can also use existing integer operations, assertions and lists. Plain `compile`
rejects NixOS references as IR validation errors before producing unbound Nix.
Missing options, incompatible backend values and cycles remain evaluator concerns;
there is no whole-config handle, dynamic traversal, symbolic iteration or Rust
fixed-point execution. All evaluations use the unchanged disposable-store helper.


[Derive integration tests](crates/rusnix-nix/tests/derive.rs) cover nested
placement, literal renames, newtypes, flatten/skip, generics/lifetimes,
automatic unit-enum lowering, explicit structural enum mappings, record-list
laziness and provenance, independent NixOS merges/priorities and two-origin
conflicts, native handles, package resolution, symbolic overrides and naming
conventions. Proc-macro and UI tests cover naming rules and invalid annotations.
NixOS can inspect scalar heads while processing freeform namespaces; list
elements remain deferred.

[Inline-module tests](crates/rusnix-nix/tests/config_module.rs) cover local
structure, external reusable values, automatic unit enums/custom converters,
multiple roots, NixOS merges/priorities, two-origin conflicts, operation
provenance/laziness, symbolic/opaque handles and package resolution. The symbolic
example uses the module boundary; its override tests reuse the same generated
artifact. Typed-submodule retains fine-grained derives for reusable types and
uses the boundary for its local tree. Proc-macro tests reject out-of-line modules
without loading files.

The opaque boundary also supports scoped `NixValue::function` callbacks, lazy
`if_else` choices, equality and context-preserving text conversion. Existing Nix
functions still own collection traversal and schemas. Callback parameters cannot
escape their scope; generated binder names are deterministic and capture-safe.
`OptionRef<T>::into_value()` explicitly passes a finite known option dependency,
including collection values, to this boundary without reading it in Rust.
`Nixpkgs::from_module()` selects the NixOS-supplied package set, preserving its
configuration and overlays; these references require NixosModule lowering.


Structural lowering keeps optional values and optional definitions distinct:
`Option<T>` normally lowers `None` to Nix `null`; `#[rusnix(omit_none)]` on a field
or named struct omits absent direct Option fields instead. Struct settings are
local and are not inherited by nested structs. Empty collections are unchanged.

## Complete PostgreSQL module rewrite

The substantial [PostgreSQL NixOS module](examples/postgresql-nixos-module/README.md) separates its
user-defined Rust model (`model.rs`), public option schema (`schema.rs`), finite
symbolic dependencies (`options.rs`), and configuration implementation / lowering
(`lowering.rs`). Both sides of the pinned module are authored in Rust. The candidate
imports no original PostgreSQL module; the equivalence harness keeps it as a reference.
Ordinary NixOS modules use the same `services.postgresql.*` interface.

Typed owned-database provisioning and three-state role clauses coexist with
finite final-option dependencies and opaque package/build-helper calls.
The [equivalence suite](crates/rusnix-nix/tests/postgresql.rs) compares full NixOS
evaluation, generated-file/check derivation recipes and string dependency contexts,
including ordinary downstream overrides of the same generated artifact. It builds
and activates nothing. See [the example notes](examples/README.md#postgresql-compatibility-rewrite)
for coverage and limitations.

Public schemas use structural `OptionDecl` trees with `NixosModule::declare`.
`OptionType` selects and composes real `lib.types` values, including submodules,
freeform attrsets and coercions; NixOS performs validation and merging. Defaults,
defaultText and examples remain separate metadata. The PostgreSQL schema suite
compares documentation and all 32 ordinary, nested and migration options, plus
ordinary Nix consumers and invalid definitions. Declaration origins remain separate
from configuration definitions: a foreign bad value is not blamed on schema Rust,
while a bad schema default can identify its declaration.

`NixSession::evaluate_nixos_with_driver` lets backend/test infrastructure use a
custom semantic projection while retaining generated-module parsing and existing
Rusnix diagnostics. The adapter recognizes both staged nixpkgs library trees and
preserves precise NixOS option paths inside opaque records.

The [Git package example](examples/git-nixpkg/README.md) rewrites the pinned Git 2.47.0
package expression using the real nixpkgs `callPackage`/`mkDerivation` backend.
Its tests compare exact derivation recipes, feature/platform choices and ordinary
Nix overrides without fetching sources or building Git.
