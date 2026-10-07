# Delayed backend provenance investigation

This records the context-only investigation at `e00fe587`. The later
[out-of-band correlation investigation](backend-correlation-investigation.md)
builds on this negative result without adding runtime wrappers. Historical
context experiments disable correlation so they continue measuring contexts alone.

Lazy `builtins.addErrorContext` does **not** fix the demonstrated delayed
dependency validation gap. It covers exceptions raised while evaluating the
wrapped expression. Once evaluation successfully produces an integer, list or
attrset, that context ends. Later validation and forcing of children do not
reactivate it, even when the backend retains the original thunk.

Element wrappers preserve deferred exceptions through stdenv, but existing
source maps and operation boundaries already recover the useful Rust operations
in the tested real-package cases where those exceptions occur. No production
instrumentation, parser change or new boundary category is adopted. Retained
changes are evidence tests, offline Nix fixtures, fixture registration and docs.

The answer is therefore conditional: the wrapper can survive until a delayed
evaluation exception **inside its own expression**, but not until a later
backend validation failure **after that expression has returned successfully**.

## Setup and reproduction

- Primary checkout: `/home/user/dev/rusnix`, clean at
  `fcbdf6b742e400327ed3dc7f02b280fb79929439` before work began.
- Branch: `backend-provenance`, created from that exact committed HEAD.
- Retained worktree: `/home/user/dev/worktrees/rusnix-backend-provenance`.
- nixpkgs: `8b27c1239e5c421a2bbc2c65d52e4a6fbf2ff296`, cloned with shared
  objects from the existing local checkout, without fetching.
- Nix 2.34.8, queried through `NixSession::version()`.
- Every Nix subprocess uses `isolated.rs` and a disposable local store. No
  builds, fetches, profiles, host configuration changes, activation, host GC or
  privileged operations occur. All evaluation is offline.

```bash
cargo test --locked -p rusnix-nix --test backend_provenance
```

Seven tests cover 240 minimal backend cases, 20 real-package input failures,
15 failures inside composed child packages, eight context-lifetime probes,
five complete recipe graph comparisons, ten partial/excluded graph evaluations
and ten default/field/validation/finalAttrs evaluations.

Full original diagnostics, including untouched `raw_nix`, are retained under
`target/backend-provenance/{matrix,real-failures,child-packages,lifetime,laziness}/`.
Real failures also save generated Nix and maps. Recipe comparisons are in
`target/composed-equivalence/backend-provenance-{none,field,child,call,combined}/`.
These generated artifacts remain in the worktree, outside Git. Tests inspect
structured trace `raw_msg` fields: labels in source excerpts do not count as
surviving contexts.

Standalone `.stderr` files also preserve the raw traces for all 35 real-package
and composed-child failures.

## Actual failures and diagnostic comparison

Investigation started with the existing composed test
`delayed_stdenv_dependency_validation_retains_reason_and_honest_mapping_limit`,
which supplies integer curl to MariaDB. Git's existing
`invalid_structured_dependency_maps_to_rust_lookup_or_call` also exercises a
non-package record with only a broad caller boundary.

Additional reproductions below compare no wrapper, whole field, individual child,
opaque call, and call + field + child at the actual mkDerivation handoff. **All
four strategies retain exactly the same primary Rust origin, reason and semantic
path as baseline.** Supplier locations and IDs are in saved generated maps.

| Rust supplier / desired child | Receiving generated boundary | Eventual Nix failure | Primary before and after |
| --- | --- | --- | --- |
| `1_i64.into()` supplied as MariaDB curl; consumed by `i.curl()` at `mariadb-nixpkg/lowering.rs:104` | MariaDB server mkDerivation, `buildInputs[6]` | stdenv invalid dependency type | outer `mariadb.drvPath` demand |
| record `{ notAPackage = true; }` supplied as MariaDB curl | same sixth buildInput | same type check | outer `mariadb.drvPath` demand |
| integer supplied as Git OpenSSL | Git mkDerivation, `buildInputs[2]` | same type check | outer `git.drvPath` demand |
| integer supplied as curl OpenSSL | curl mkDerivation, deferred configureFlags element | integer coercion, `lib/strings.nix:2139` | existing curl operation, `lowering.rs:324` |
| integer supplied as real Rust curl's `pkg-config`, with real Rust OpenSSL → curl → MariaDB | curl mkDerivation, `nativeBuildInputs[1]`; MariaDB consumes that actual Rust curl package | curl's delayed stdenv check | outer `mariadb.drvPath` demand |
| integer supplied as real Rust OpenSSL's enabled `cryptodev`, through real Rust curl → MariaDB | OpenSSL mkDerivation, `buildInputs[1]` | OpenSSL's delayed check during curl flag coercion | existing curl operation, `lowering.rs:324` |
| same bad Rust OpenSSL → Rust Git | OpenSSL mkDerivation, `buildInputs[1]` | OpenSSL's delayed check during Git dependency processing | outer `git.drvPath` demand |

The saved maps explicitly contain the supplier at `backend_provenance.rs:216:23`
for the integer replacements, `:335:81` for curl's pkg-config and `:326:41`
for OpenSSL's cryptodev (purpose: `opaque call argument`). The corresponding
outer demand origins are `:231:68` and `:350:68`. Desired consuming child
locations include Git `lowering.rs:202`, curl `lowering.rs:193` and OpenSSL
`lowering.rs:237`. These specific child origins exist in the maps; wrappers
do not make them appear in the delayed-validation diagnostics.

`Diagnostic::option_path` is absent in every reproduction, before and after.
There is an enclosing `result` assignment and semantic ancestry in maps/related
origins, but no recovered Rust dependency child path. Nix's reported dependency
index belongs to its validation traversal; it is not invented as a Rust path.

The MariaDB integer's original trace begins with:

```text
Dependency is not of a valid type: element 6 of buildInputs for mariadb-server
while calling 'throw'                 make-derivation.nix:284:14
while calling anonymous lambda       make-derivation.nix:281:15
from call site                        lib/lists.nix:334:32
from call site                        make-derivation.nix:312:25
... getDev / getOutput, dependency processing, derivationStrict ...
while evaluating result               generated.nix
```

None of the strategies inserts the offending curl child's marker in this trace.
Complete unmodified traces, including excerpts and positions, are in
`target/backend-provenance/real-failures/mariadb-curl-int-*.json`.

Positive controls already work: failing OpenSSL fetchurl through the graph maps
to OpenSSL `lowering.rs:130`; failing curl fetchurl demanded by MariaDB maps to
curl `lowering.rs:155`; malformed OpenSSL consumed by curl maps to curl
`lowering.rs:324`. For the last case a configureFlags child wrapper survives,
but adds no useful origin. The same parent wrapper can survive while demanding
a broken child OpenSSL package; it still identifies curl's consuming operation,
not OpenSSL's offending dependency.

Existing Git, curl, OpenSSL, MariaDB, composed and PostgreSQL tests were inspected
and verified. PostgreSQL covers module lookup/type/assertion boundaries rather
than this mkDerivation dependency gap. Existing positive diagnostics and raw
traces are preserved; no snapshot expectations are weakened or updated.

## IR, lowering, and loss mechanism

1. Literals, package handles, list elements and argument selections already have
   `Node::origin`. This includes the supplier and the consuming
   `i.curl()`/`i.cryptodev()`/`i.pkg_config()` expression.
2. Typed mkDerivation/builders produce `ValueKind::Apply`. Lowering classifies
   applications as `DiagnosticBoundary::OpaqueCall`. Ordinary literals,
   containers and selections retain source spans and semantic ancestry.
3. Generated lists pass lazy parameter/child expressions to stdenv. MariaDB
   concatenates common inputs before its server/client builder calls.
4. `make-derivation.nix:279–285` uses `imap1`. Its callback forces `dep` enough
   for `isDerivation`, equality and type predicates. A wrapped integer becomes
   `1`; a wrapped non-package record becomes an attrset. This successful
   evaluation ends the context.
5. The callback **subsequently** throws its invalid-type error, outside that
   context. The trace names stdenv's callback, not the generated supplier. No
   generated child position remains for source-map ancestry to recover.
6. Accepted dependencies go through splicing, `getDev`/`getOutput` and eventual
   string coercion. Context on their attrset does not cover the separate lazy
   `outPath` field. No copying is required for this second loss either.

Thus thunk retention is insufficient. Context is an evaluation stack boundary,
not persistent metadata on a successful value.

## Minimal Nix experiments

The matrix runs 12 scenarios × four payloads × five placements: none, A whole
field, B individual child, C opaque call, D call + field + child. Payloads are an
integer, a non-package attrset, a deferred throw, and a derivation-shaped attrset
with deferred failing `outPath`.

| Scenario | Child throw under B/D | Integer/attrset rejected under A/B/C/D | Nested outPath failure under A/B/C/D |
| --- | --- | --- | --- |
| dependency list | CHILD survives | no marker | no marker |
| nested dependency list | CHILD survives | no marker; Nix preserves nested indexes | no marker |
| propagatedBuildInputs | CHILD survives | no marker | no marker |
| copying through two attrsets | CHILD survives | no marker | no marker |
| library identity map | CHILD survives | no marker | no marker |
| library flatten | CHILD survives | no marker | no marker |
| reconstruction using `x + 0` after type inspection | CHILD survives if source evaluation throws first | no marker | no marker |
| makeOverridable / override | CHILD survives | no marker | no marker |
| overrideAttrs | CHILD survives | no marker | no marker |
| recursive finalAttrs | CHILD survives | no marker | no marker |
| three mkDerivation layers, including propagation | CHILD survives | no marker | no marker |
| plain derivation environment field | CHILD and whole FIELD survive scalar throw | invalid record loses context; integer is valid | no marker |

There are 235 expected failures and five successful scalar-integer derivations.
Whole-field contexts on list heads do not cover elements; opaque-call contexts
on returned derivation attrsets do not cover later recipe demands. Field context
plus ancestry cannot identify an element with no generated position or marker.

The test-only stdenv adapter selects five list-shaped fields: buildInputs,
nativeBuildInputs, propagatedBuildInputs, configureFlags and cmakeFlags. It uses
lazy map/mapAttrs to attach experiment labels and forwards function-valued
finalAttrs arguments. It is scoped to these tested shapes, not a generic recipe
schema or proposed public API; it does not reproduce backend type validation.

Eight separate lifetime probes establish the same facts without nixpkgs:
wrapping a throw or a complete rejecting helper operation retains context;
wrapping `[ throw ... ]`, `{ child = throw ...; }`, a successfully forced scalar
or a successfully forced container does not cover a later child/helper failure.
Wrapping the nested failing field itself works. That requires knowing the actual
fallible computation and still cannot cover a validator rejecting an already
evaluated primitive. Recursive package/output instrumentation was not attempted.

The smallest useful granularity for **evaluation exceptions** is the deferred
scalar/element operation. No tested value-handoff granularity solves validation
of successfully evaluated values. Human labels aid experiment inspection but
cannot change this lifetime; existing `rn-...` IDs/maps remain authoritative.

## Laziness and identity

All placements preserve unused defaults, excluded optional dependencies, unused
selected configureFlags/passthru fields, later dependency validation after an
earlier unsupported-hardening failure, finalAttrs recursion, an unused recursive
passthru field and a live version override. Git with a throwing curl/MariaDB
branch and curl with an excluded throwing OpenSSL branch also remain evaluable.

All five placements match pinned upstream complete ATerm recipe bytes, `.drv`
paths and every output path for the connected default graph:

| Package | Identical derivation path |
| --- | --- |
| OpenSSL 3.3.2 | `/nix/store/mqf1h79k4p5y723yvn0869ni24ifpnpn-openssl-3.3.2.drv` |
| curl 8.11.0 | `/nix/store/m9lkswh8vqpcpvj43mlp243jqgd3ka7q-curl-8.11.0.drv` |
| Git 2.47.0 | `/nix/store/qk30rjnnsz9lxdp9b6k5hbjj7p8w17pg-git-2.47.0.drv` |
| MariaDB server 10.11.10 | `/nix/store/ph4f1jlld7627mn1xqb05d357gw0l663-mariadb-server-10.11.10.drv` |
| MariaDB client 10.11.10 | `/nix/store/gd3f8yw68jnsh5m09rkylxa093pqa7s2-mariadb-client-10.11.10.drv` |

Recipes and outputs are not normalized. These are logical identities evaluated
inside disposable stores. Five adapter graph comparisons give 25 exact package
comparisons. Four additional temporary AST renderings give 20 more, including
the same OpenSSL → curl → MariaDB and OpenSSL → Git edges. Representative
defaults are tested; arbitrary recipes/backends are not claimed. Existing full
package matrices remain part of full verification.

## Instrumentation cost

A temporary pre-render AST probe marked selected dependency/flag fields or their
literal list children using existing origins and renderer context flags. It
did not match rendered function names, rewrite generated source or add forcing.
It was removed from compiler source; code/results remain under
`target/backend-provenance/cost/`. This is a cost experiment, **not a proposed
semantic policy**. Its traversal can count helper-list children and shared
records that do not correspond one-to-one to final dependency positions.

Each row represents one instantiated package, or the graph, without additional
exported factories/families. Bytes omit println's final newline. Existing
operation contexts are reused, so most whole fields need no added wrapper.

| Artifact | Baseline contexts / bytes / lines | Whole field contexts / bytes / lines / added | Child contexts / bytes / lines / added | Combined contexts / bytes / lines / added |
| --- | --- | --- | --- | --- |
| Git | 150 / 63,303 / 1,078 | 150 / 63,303 / 1,078 / 0 | 181 / 66,064 / 1,110 / 31 | 181 / 66,064 / 1,110 / 31 |
| curl | 113 / 35,482 / 690 | 114 / 35,551 / 691 / 1 | 129 / 37,522 / 727 / 16 | 130 / 37,623 / 729 / 17 |
| OpenSSL | 113 / 38,177 / 699 | 113 / 38,177 / 699 / 0 | 122 / 39,098 / 713 / 9 | 122 / 39,098 / 713 / 9 |
| MariaDB | 100 / 37,570 / 697 | 100 / 37,570 / 697 / 0 | 206 / 49,444 / 875 / 106 | 206 / 49,444 / 875 / 106 |
| Graph | 481 / 216,150 / 3,433 | 482 / 216,227 / 3,434 / 1 | 643 / 241,746 / 3,798 / 162 | 644 / 241,863 / 3,800 / 163 |

Combined graph cost is 163 new static markers, 25,713 bytes (11.9%) and 367
lines, without evidence it repairs delayed invalid-type attribution. The final
production delta is **zero wrappers, zero bytes and zero lines**.

## Classification and next direction

| Boundary | Supported classification |
| --- | --- |
| mkDerivation field/element whose own evaluation throws | A: useful lifetime; in tested Rust packages D: no additional origin because operation already maps |
| successfully evaluated dependency later rejected | E: context ends before validator throws, even without reconstruction |
| dependency attrset whose child is later forced | E: outer context does not cover child thunk |
| fetcher calls | B: existing boundary works in OpenSSL/curl/MariaDB controls; arbitrary delayed fetcher arguments untested |
| callPackage override arguments yielding later-invalid dependencies | E by demonstrated value lifetime; application failures still have their boundary |
| package-function applications | B for immediate failures; E for later failures after returning a successful container |
| override / overrideAttrs / finalAttrs | A for still-failing child thunks; E for later invalid-type rejection, directly tested |
| raw library helpers | B/C for existing operation/position failures; copies retain deferred exceptions; rejection after successful value evaluation is E |
| module imports | existing metadata/import policy; no new module-wrapper experiment or general A/E claim |

Keep ordinary structure on source maps/ancestry, and meaningful fallible/opaque
operations on runtime contexts. Do not add a general `BackendArgument` boundary
from these results. The existing private `DiagnosticBoundary` enum is unchanged.

The next promising bounded experiment is **structured backend-boundary metadata
and per-field dependency provenance tables**, correlated with Nix's reported
field/index. nixpkgs remains authoritative; Rusnix records suppliers and edges,
not duplicate validation. Concatenation, optional/check inputs, splicing,
overrides, repeated call instances and transformed indexes need explicit handling
and honest correlation limits. No such mechanism is implemented here.

Evaluator-level provenance could retain origins through successful evaluation
more generally, but requires a larger scope and evaluator changes. Additional
`addErrorContext` wrappers cannot provide persistent value metadata.

## Final verification

All checks run from the requested worktree. `RUSTDOCFLAGS='-D warnings'` is set
for workspace tests, documentation and the fixture workflow.

| Check | Result / log |
| --- | --- |
| `cargo fmt --check` and `cargo fmt --all --check` | Passed; `target/final-fmt-default.log`, `target/final-fmt.log` |
| structural spacing checker | 169 Rust files, zero gaps; `target/final-spacing.log` |
| `cargo test --workspace --locked` | 526 passed in 39 groups, zero failed/ignored; `target/final-workspace.log` |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed; `target/final-clippy.log` |
| strict `cargo doc --workspace --no-deps --locked` | Passed; `target/final-rustdoc.log` |
| `cargo build --workspace --examples --locked` | All 15 built; `target/final-examples.log` |
| UI / compile-fail | All 67 UI fixtures plus workspace doctests passed |
| PostgreSQL / schema | 57 + 4 passed |
| Git / curl / OpenSSL / MariaDB / composed | 44 / 23 / 8 / 8 / 16 passed |
| focused provenance suite | Seven tests passed, including all matrix rows |
| `bash scripts/check-fixtures.sh` | Passed, including all 15 example runs and CLI snapshots/artifacts; `target/final-fixtures.log` |

The machine-readable check record is `target/final-verification.json`. The
single investigation commit retains only documentation, evidence tests and
fixture workflow registration. The final response records its hash and final
worktree status.

The primary checkout remains clean at the starting HEAD, and the worktree's
nixpkgs checkout remains clean at the pin. The worktree is retained. Generated
evidence stays in ignored `target/`; no production source changes or failed
implementation spike remain in the committed tree.
