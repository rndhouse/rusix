# Rusnix

A small experiment in using ordinary Rust as a typed frontend for Nix. The
working loop is Rust type checking → semantic IR → validation → Nix AST →
generated Nix → isolated evaluation → a diagnostic pointing back to Rust.
Generated Nix is compiler output. Edit Rust configuration functions.

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

The [typed configuration showcase](examples/README.md) now demonstrates static
semantic types, unrepresentable field combinations, caller contracts, and
exhaustive model consumers. Nine single-file showcases and the multi-file
PostgreSQL example define their own domain types and distinguish user models,
opaque Nix objects and the generic escape hatch.
Seven tested Nix comparisons and forty-six UI fixtures
(including interop category safety and missing TLS keys) back their claims. The examples explain
which guarantees are static, which require IR checks, and which remain NixOS
checks; generic configuration/IR escape hatches remain explicit.

## Run

Prerequisites: Rust 1.88+ and `nix` plus `nix-instantiate` on PATH. Tested here
with Rust 1.97.1 and Nix 2.34.8 on Linux. The live structured-diagnostic tests
expect the `raw_msg`/`trace` fields provided by that Nix version. Older diagnostic
formats have adapter coverage, not a tested cross-version compatibility promise.

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
output directory replaces these compiler-owned artifacts and clears old results.

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
Only nix-interop deliberately calls Config::set; layered-validation failure assemblies live in tests.

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
| `rusnix-nix/render.rs` | Escaped Nix source, comments, contexts, byte source map |
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

Four mechanisms are exercised:

1. **Source spans:** byte ranges in generated Nix map to Rust origins. The
   smallest enclosing range provides a fallback for a generated Nix frame.
   External-file positions are never mapped as though they came from this file.
   Each span also stores enclosing configuration/assignment/value origins,
   allowing an option path to survive even when its runtime context has unwound.
2. **Comment markers:** generated comments contain stable `rn-...` origin IDs.
   These help manual inspection and survive emission, but comments alone do not
   survive evaluation as diagnostic metadata.
3. **Evaluation contexts:** only deferred division and range-constraint operations use
   `builtins.addErrorContext "rusnix-origin:rn-..." (...)`. The innermost known
   context identifies the Rust operation. Literals, lists, attribute sets, and
   assignments carry comments/spans but no runtime context. Static ancestry
   supplies the enclosing option path independently of evaluation demand.
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
Generated operation contexts now have this shape:

```nix
builtins.addErrorContext "rusnix-origin:rn-..." (builtins.div 44 0)
```

Division produces a scalar. Its context remains active when the demanded
operation fails, even when that operation was delayed inside a list. No outer
container needs forcing to keep that operation's context. Existing nested
diagnostics and their snapshots still pass, with the option path reconstructed
from the operation's static span ancestry.

Tests select both scalar and list-valued failing attributes while their good
siblings remain usable. A live AST fixture also omits every runtime context:
Nix's generated division frame still maps to Rust via source spans alone, with
the enclosing path retained. This proves a source-map fallback on a real Nix
failure, in addition to the adapter test which removes captured trace entries.

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

Nix's chroot local store still uses logical `/nix/store` names. The **physical**
store is `<root>/store/nix/store`, with its database inside that same root.
One regression test uses `builtins.toFile` through the isolated helper and
checks the physical file/database and subsequent cleanup. A separate command
construction test asserts store flags for parsing, evaluation, and version
construction, including selected evaluation, along with removal of host
store-selection variables. Literal-selection tests cover quotes, interpolation,
and attribute names resembling store-selection flags. This prevents accidental
default-store selection through this helper; it is not filesystem sandboxing.

## Agent iteration

```bash
cargo test --workspace --locked
bash scripts/check-fixtures.sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
```

The suite contains a Rust compile-fail doctest (`E0308`), a valid enum-bearing
configuration, a bad port constraint, a nested division failure, IR path
conflicts, generated syntax/static-binding failures, and an explicitly unmapped
error. Tests also cover lazy-context loss, structured/text/source-map adapters,
escaping/interpolation, integer boundaries, store isolation, and CLI artifacts.
Selective scalar/list evaluation, cloned-expression path recovery, and a live
source-map-only failure are covered too. Fourteen committed snapshots compare the stable Rust-facing diagnostic surface,
including actual source locations and underlying reasons. Missing Nix tooling
fails tests instead of silently skipping the central experiment.

The fixture script reruns snapshots and saves all CLI cases under
`target/diagnostic-fixtures/`. Failed fixtures are expected and their diagnostic
kinds are checked. Rust's type failure is covered by `cargo test` rather than
being passed to Nix. The original small warm suite took under one second; the interop suite
also expands the source archive and evaluates packages (about five seconds here).
Cargo's first dependency acquisition may need network access; once cached,
append `--offline` to Cargo commands. Nix expressions never fetch network data.

The selective-evaluation experiment proved local operation provenance without
eagerly forcing enclosing values; the module experiment extends it to NixOS
definitions, assertion messages, and a specific imported-module boundary. No full NixOS API,
broad package bindings, flake APIs, derivation building, deployment, or alternate backend is
implemented.

## NixOS module experiment

The new frontend lives in `rusnix-ir/nixos.rs`; lowering, pinned source staging,
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

**Evaluation surface:** pinned nixpkgs commit
`8b27c1239e5c421a2bbc2c65d52e4a6fbf2ff296` (24.11), with its complete library
and four modules vendored as unmodified upstream files. See `vendor/README.md`
and `vendor/nixpkgs/PIN.json`. Every listed file is SHA-256 checked, then copied
into the disposable session. The library is imported from there offline.

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

**Module provenance:** NixOS evaluates an option's type after a literal's context
would have unwound; unknown options can fail without a useful generated frame.
The compiler therefore emits **one inline imported module per assignment**:

```nix
{
  "_file" = "rusnix-definition:rn-cb0f3390b6cbe7a2";
  "config" = {
    "services"."openssh"."ports" = [ "twenty-two" ];
  };
}
```

NixOS carries `_file` into definition error messages. Rusnix resolves its marker
to the exact Rust `.set()` and option path. The actual raw type error includes:

```text
A definition for option `services.openssh.ports."[definition 1-entry 1]"' is not of type `16 bit unsigned integer; between 0 and 65535 (both inclusive)'. Definition values:
- In `rusnix-definition:rn-cb0f3390b6cbe7a2': "twenty-two"
```

The displayed reason replaces the marker with the Rust location; raw stderr is
retained unchanged. This avoids blaming the list container or imported SSH code.
Assertion messages carry `[rusnix-assertion:<id>]`; the driver adds one scalar
`addErrorContext "rusnix-stage:nixos-assertions"` around the failed-assertion throw.
The marker maps to `.assertion()` and `assertions.<name>`, and is removed only
from the displayed reason. Existing operation contexts remain **only** around
division/range checks; no generated `deepSeq` or broad forcing was added.

| Fixture | Observed reason | Rust mapping | Lost precision |
| --- | --- | --- | --- |
| `type` | ports entry is not a 16-bit unsigned integer | `.set`, `services.openssh.ports`, definition marker | individual list-entry Rust point |
| `unknown` | `services.openssh.rusnixMissing` does not exist | introducing `.set`, definition marker | no value-level runtime context |
| `assertion` | Failed assertions: SSH port policy rejected | `.assertion`, message marker | false boolean itself did not throw |
| `external` | attribute `version` missing inside `label.nix` | `.import(NixosLabel)`, external trace frame | no exact upstream-causal Rust expression |
| `lazy` | ports nested division by zero | `.divide`, runtime marker + static ancestry | none in the tested path |

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
small dependency (`sha2`) and assumes the vendored source is available relative
to the build workspace. The harness does not prove installed binary portability.
Generated module source is 486–979 bytes for these fixtures, plus a shared
1,635-byte driver; one-module-per-definition adds scaffolding but no eager
wrappers. The core design remains Rust IR → backend AST → source map/metadata.

The original 9 NixOS tests and all earlier snapshots still pass. The current
full verification result and multi-origin findings follow below.

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
  "_file" = "rusnix-definition:rn-85c0e94174be752e";
  "config" = {
    "services"."openssh"."authorizedKeysCommandUser" = # rusnix-origin:rn-8163424b47eca2b2
    "root";
  };
}
{
  "_file" = "rusnix-definition:rn-4da1cd4d48cf31f8";
  "config" = {
    "services"."openssh"."authorizedKeysCommandUser" = # rusnix-origin:rn-cadcdcb6a3ef14f8
    "nobody";
  };
}
```

Both definitions evaluate successfully alone. Combined, the real upstream
string option's `mergeEqualOption` rejects them. Its raw reason is:

```text
The option `services.openssh.authorizedKeysCommandUser' has conflicting definition values:
- In `rusnix-definition:rn-4da1cd4d48cf31f8': "nobody"
- In `rusnix-definition:rn-85c0e94174be752e': "root"
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
- In `rusnix-definition:rn-5c963c20bc78fbd8': "rusnix-label"
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

The evaluator/driver and store-selection helper were not expanded for this
experiment. No broad forcing, metadata probe, extra fetch, build, activation,
or host-store operation was added. An unselected conflicting option leaves
selected ports usable. All prior lazy/context/source-map/compiler/store tests
pass. Generated fixture modules are 781–1,322 bytes; nested imports and small
priority payloads are the added scaffolding.

Run the new cases with `check-nixos <fixture> --out <directory>`, or run
`scripts/check-fixtures.sh` for all old/new cases and reviewable artifacts.
The multi-origin phase added 9 merge/causal-set integration tests, 2 adapter/
rendering unit tests, and 1 CLI test, bringing that phase to 49 passing tests.
All four new rendering snapshots remain passing. The subsequent typed showcase
brought the typed-example phase to **67 passing tests**, zero failures or ignored
tests (58 ordinary tests and 9 compile-fail doctests). Its additions are 10 live
example/comparison tests, 1 UI diagnostic-code test, and 7 compile-fail doctests.
Formatting, Clippy with warnings denied, and the full fixture script pass.

The provenance experiment is complete at this scope. The following typed showcase phase demonstrates frontend invariants that reject
important invalid configurations before Nix evaluation. No diagnostic infrastructure
was expanded for the showcase. See `examples/README.md`.

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

`rusnix_ir::interop` owns distinct PackageRef/ModuleRef/OverlayRef/NixFunction
handles, InputRef, and an explicit NixValue escape hatch. A reference carries its
source identity, structured attribute segments, and Rust lookup origin. It
lowers to AST imports, escaped getAttr operations, and function applications.
Nix owns existence, object schemas, overlay semantics, and actual package type
checking. `NixosModule::system_packages` accepts only PackageRef; `import_ref`
accepts only ModuleRef. Typed user-defined models can coexist with these handles.

Interop uses the full **already cached and now vendored** archive at the existing
revision, verified by SHA-256 and expanded into disposable ordinary source files.
The original minimal tests retain their small source subset. Full package roots
use explicit `system = "x86_64-linux"`, `config = {}`, and handle-scoped overlays;
this is not yet a platform or nixpkgs-configuration API. The evaluator imports
upstream `config/system-path.nix` to get the real systemPackages schema, forces
only that option's definition priority to exclude unrelated full-system defaults,
and projects package name/version/type metadata. It neither selects system.path
nor serializes whole derivations. Evaluation may materialize `.drv` records in
the isolated store; packages are never built. The helper additionally disables
import-from-derivation, and every Nix process retains explicit disposable store
and matching eval-store selection.

Contexts on lookup/call/selection operations preserve immediate opaque-boundary
failures. Imported module functions retain file boundaries through existing trace
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
let file = pkgs.package_function("writeTextFile").call(args);
let curried = pkgs.package_function("writeText")
    .apply(["postgresql.conf".into(), "workers = 4\n".into()]);
```

`package_function` looks up functions in the real package set, retaining its
overlays; the existing `function` looks in nixpkgs/lib. Each `.call` is ordinary
Nix application; `.apply([args...])` handles currying on either NixFunction or
NixValue and preserves the Rust application call site. Results stay `NixValue`,
without automatic PackageRef inference. Existing package functions/overrides can also be
selected through `as_value().select(...)` and called. Function schemas and errors
belong to Nix. No builder or package-specific Rust code is involved.

Use derived Rust structs for meaningful fixed schemas and `NixValue::record`
arrays/iterators for open records. `into_value().into_nix_value()` converts a
structural value to an atomic opaque value without flattening it into bindings.
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


Current verification: `cargo test --workspace --locked` passes **238 tests**
(233 ordinary tests and 5 doctests, including 3 compile-fail cases; zero
failed/ignored). The interop target has 24 tests; structured_interop has 23
and PostgreSQL has 56.
Formatting and all-target Clippy with warnings denied pass.
The fixture script passes 203 integration checks, ten runnable
showcase examples plus the legacy generic SSH compiler example, and 20 original
CLI fixture invocations. All 14 diagnostic snapshots remain passing.
Examples print generated source without Nix evaluation, fixture construction or
artifact writes. Tests reuse the actual authoring models for comparison,
combined interop behavior and symbolic artifact reuse.


The original examples cleanup kept eight single Rust files under `examples/`,
with models and typed functions before lowering. It used manual conversion for
complete components; the nine small examples now use automatic structural lowering and
explicit value mappings.
Reusable Endpoint values have no global conversion: the typed-submodule example
now owns its placement through a derived Root → Demo → Endpoint tree. TLS requires
both Certificate and PrivateKey. The model-evolution fixture checks two incomplete
policy consumers; the domain fixture checks both a field and a function argument.
The native Nix assertion test also rejects TLS without its key. IR/AST separation,
diagnostics, lazy evaluation and store isolation are preserved. Executable
Nix comparisons now live in `tests/comparisons/`; the local opaque Nix input is
`tests/fixtures/nix-interop-input.nix`. That cleanup added no showcase examples; symbolic-option was added subsequently.

The first authoring abstraction is `IntoConfig::into_config(self) -> Config`
(tracked caller, unsealed) plus `NixosModule::empty().add(component)`.
Each add stores a separate child NixosModule; bindings are never flattened
across components. Config remains a contribution with duplicate-path validation,
and Config::set remains the supported generic escape hatch. Config itself also
implements IntoConfig. Explicit per-contribution priority continues to use
`module(NixosModule::new(component.into_config()).priority(...))`.
The six authoring tests exercise two different typed components, individually
valid definitions that conflict together with both Rust origins, list merging,
priority filtering, caller forwarding and previously captured expression origins.
This authoring step reused the existing backend, diagnostics, opaque handles and store-isolation machinery.
Conversion-generated bindings identify the into_config/add authoring call;
untracked adapter helpers stop caller forwarding, so tracked lowering helpers
remain important. During priority filtering Nix can inspect the head of an
unwrapped definition; an override wrapper keeps its discarded content lazy.
This is upstream module behavior, not additional Rusnix forcing.

Domain-neutral cleanup removed `rusnix_ir::typed`, LogLevel, OpenSsh and the
ExistingModule catalogue from core. All examples define their own small
models directly. Fixture-only SSH/module helpers live in `tests/support/nixos.rs`.
Pinned imports now carry a validated relative path as data, rendered through the
Nix AST with escaped string addition; opaque references are unchanged.
The domain UI fixtures compile actual example-owned types (or a deliberately evolved
test-local enum) and check codes/spans/type labels. They preserve the former nine
domain/example compile-fail doctest guarantees, including the moved SSH setter
contract; the two remaining core compile-fail doctests cover generic Expr and
opaque PackageRef/ModuleRef category safety. No domain aliases are retained.

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
`bool`, `String`, and `i64` return their existing Expr types. NixValue, Option<T>,
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

The [symbolic dependency tests](crates/rusnix-nix/tests/symbolic_options.rs) add
an ordinary Nix option-declaration fixture and an independent default-priority
port contribution of 5432. Each contribution retains its own `_file` identity.
Only contributions containing references become `{ config, ... }:` functions;
existing concrete modules retain their generated representation.
The actual generated dependent value is:

```nix
"systemd"."services"."example"."serviceConfig"."ExecStart" = # rusnix-origin:rn-3011865f26657c91
(builtins.addErrorContext "rusnix-origin:rn-3011865f26657c91" (("example --port=" + # rusnix-origin:rn-f3e74398cc7c2441
(builtins.addErrorContext "rusnix-origin:rn-f3e74398cc7c2441" ((builtins.toString (# rusnix-origin:rn-5f19170779a2dd8f
(builtins.addErrorContext "rusnix-origin:rn-5f19170779a2dd8f" ((config)."services"."example"."port")))))))));
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

Nine tests cover the base value, unchanged-artifact override, priority selection,
the same symbolic text accepted by the real upstream OpenSSH `banner` option,
division-by-zero provenance and source-map fallback, missing-reference provenance,
lazy selection, bool/string/list uses, validation and escaped paths.
`tests/ui/symbolic-integer-as-boolean.rs` checks E0308 with a primary span and
`Expr<bool>`/`Expr<i64>` labels. An integer dependency cannot be an assertion
condition. The UI suite has thirty cases checking thirty-two errors (including fifteen
uncoded macro errors).
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


The derive integration target has fifteen evaluator tests for nested placement, literal renames,
newtypes, flatten/skip, generics/lifetimes, automatic unit-enum lowering, explicit
structural enum mappings, record-list laziness and provenance (including source-map fallback), independent NixOS
merges/priorities and two-origin conflicts, native handle preservation, real
package resolution, symbolic override compatibility and local naming conventions
with explicit rename precedence. Three proc-macro naming tests cover mechanical
conversion rules. Thirty-eight UI fixtures check forty errors (nineteen coded
rustc errors and twenty-one macro errors),
with primary spans and relevant tokens instead of full compiler snapshots.
Dependencies reuse the already cached syn/quote/proc-macro2 versions; no fetch
was needed. Nix invocation/store protections are unchanged. NixOS can still
inspect scalar heads while processing freeform namespaces; this is upstream
module behavior, not new derive forcing.


The inline-module authoring target has nineteen integration tests for local structure,
external reusable values, automatic unit enums/custom converters, multiple roots, real
NixOS merges/priorities and two-origin conflicts, operation provenance/laziness,
and native symbolic/opaque handles and actual package resolution. The symbolic
example uses the new boundary; its existing same-artifact override tests still
reuse its actual component. Typed-submodule retains fine-grained derives for its
reusable types and uses the boundary for its local tree. Proc-macro tests reject
out-of-line modules without loading files. The existing trait APIs, backend and
store-isolation helper remain authoritative.


The opaque boundary also supports scoped `NixValue::function` callbacks, lazy
`if_else` choices, equality and context-preserving text conversion. Existing Nix
functions still own collection traversal and schemas. Callback parameters cannot
escape their scope; generated binder names are deterministic and capture-safe.
`OptionRef<T>::into_value()` explicitly passes a finite known option dependency,
including collection values, to this boundary without reading it in Rust.
`Nixpkgs::from_module()` selects the NixOS-supplied package set, preserving its
configuration and overlays; these references require NixosModule lowering.


## PostgreSQL implementation rewrite

The substantial [PostgreSQL example](examples/postgresql/main.rs) separates its
user-defined Rust model (`model.rs`), finite symbolic dependencies (`options.rs`),
and configuration implementation / lowering (`lowering.rs`). It rewrites the pinned
module's configuration implementation while reusing upstream option declarations /
public schema.
Typed owned-database provisioning and three-state role clauses coexist with
finite final-option dependencies and opaque package/build-helper calls.
The [equivalence suite](crates/rusnix-nix/tests/postgresql.rs) compares full NixOS
evaluation, generated-file/check derivation recipes and string dependency contexts,
including ordinary downstream overrides of the same generated artifact. It builds
and activates nothing. See [the example notes](examples/README.md#postgresql-compatibility-rewrite)
for coverage and limitations.

`NixSession::evaluate_nixos_with_driver` lets backend/test infrastructure use a
custom semantic projection while retaining generated-module parsing and existing
Rusnix diagnostics. The adapter recognizes both staged nixpkgs library trees and
preserves precise NixOS option paths inside opaque records.
