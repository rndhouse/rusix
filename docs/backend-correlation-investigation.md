# Out-of-band backend diagnostic correlation

This investigation continues `e00fe587` in the existing `backend-provenance`
worktree. It uses the saved 35 real failures rather than repeating the context
investigation. Primary checkout remains at `fcbdf6b`.

## Evidence before metadata design

The 35 variants represent seven logical cases repeated with five wrapper modes.
Six cases (30 variants, 85.7%) are pinned stdenv dependency-type checks. The
remaining five variants are a coercion control with an existing precise origin.

Clue letters: A field; B index; C nested key; D package name; E drv path;
F output/store path; G offending value; H argument/function; I backend frame;
J no sufficient clue. Package names in traces identify the **owner**, not the
offending dependency. Source excerpts mentioning paths/values are not clues.

| Failure | Boundary / Rust source | Desired lost origin | Actual clues | Candidate / expected precision |
| --- | --- | --- | --- | --- |
| MariaDB integer curl | MariaDB buildInputs; supplied `1_i64.into()` | supplier + `i.curl()` | A B D I; no offending value/path | pinned validator + owner + field/index; exact child if list prefix is known |
| MariaDB record curl | same, `{notAPackage=true}` | supplied record | A B D I | same |
| Git integer OpenSSL | Git buildInputs, child 2 | supplied integer + `i.openssl()` | A B D I | same |
| curl integer OpenSSL | curl configureFlags | already maps consuming Rust operation | A D G H I; no index | field only; preserve stronger direct operation |
| curl pkg-config through MariaDB | curl nativeBuildInputs, child 1 | supplied pkg-config value | A B D I; outer MariaDB owner also in trace | owner chain + field/index |
| OpenSSL cryptodev through curl/MariaDB | OpenSSL buildInputs, child 1 | supplied cryptodev value | A B D I; outer curl/MariaDB fields and owners | owner chain + field/index |
| OpenSSL cryptodev through Git | same | same | A B D I; outer Git owner | owner chain + field/index |

All 35 have field/owner/frame clues. Thirty have indexes. None has a causal
offending .drv/output/store path or nested key. Nix's structured JSON has
`raw_msg`, `trace`, `file`, `line`, `column`, not structured field/index properties.
The index grammar comes from pinned `make-derivation.nix:284`, not an evaluator
heuristic. It is one-based; nested indexes are printed innermost first. Backend
trace positions can corroborate the message's provenance. Name alone is not a
unique call-instance ID.

Additional evaluation probes cover propagated/nested/check inputs, invalid
outputs, dependency output coercion, references, env keys and configureFlags.
They run through `NixSession` and save untouched diagnostics under
`target/backend-correlation/clues/`. Setup hooks/builders do not execute during
this evaluation-only task. A checkInputs error reports the merged buildInputs
index, so indexing only the original field beyond a known prefix is unsafe.

## Existing metadata audit

IR records retain named field values and their origins. Structural conversion
adds path purposes to fresh literals; already attributed expressions retain
their more precise origins. Lists retain ordered child Nodes. NixValue and typed
package wrappers preserve their introducing Node; callback parameters and named
arguments have binding identities. `try_into_nix_value` preserves this structure
while resolving flattened Rust records. Calls/merges/helpers retain syntax, not
evaluated values. Package construction expressions exist, but no realized
derivation/store identities are recorded by compilation. Successful explicitly
selected recipe projections could provide identities afterward, but a failed
evaluation has no completed result to join. The raw failures do not name the
malformed leaves' paths, and paths are not unique Rust-origin IDs.

Rendering persists byte spans, origins and semantic ancestry; it discards the
structural record/list/binding relations needed to interpret a backend index.
No identity can be learned from the malformed integer/record: it has no store
identity. Successful saved projections can identify paths after evaluation, but
path alone can have multiple Rust suppliers and no relevant failure uses it.
No pre-evaluation identity forcing is justified by this corpus.

Implementation results and final verification follow after the bounded prototype.

## Additional clue audit

These eleven offline evaluations complement the existing corpus; they are not
counted as another 35 real package failures. Full stderr is in each JSON's
`raw_nix`. All ran through the disposable isolated helper.

| Probe | Clues / producer | Correlation assessment |
| --- | --- | --- |
| propagatedBuildInputs | A B D I; pinned dependency validator | exact child; demonstrated |
| nested buildInputs | A B D I; same validator, reversed nested index chain | exact nested child; demonstrated |
| checkInputs | A B D I, but names merged buildInputs | original-prefix children safe; appended check children unsupported |
| nativeCheckInputs | A B D I, but names merged nativeBuildInputs | unsupported appended tail; platform hooks can add more offsets |
| derivation with record outPath | A D I; evaluator string coercion | field visible, no offending dependency index/identity |
| duplicate outputs | A C/D I; evaluator names duplicate `out` | future exact key with multiple contributors; not implemented |
| invalid output name | A D F G I; evaluator path construction | path is an invalid recipe output, not a known supplier identity |
| outputs containing record | A D G I; evaluator expected-string check | field only, no list index |
| allowedReferences containing record | A D I; evaluator coercion | field only, no index or path identifying record |
| env.BAD containing record | A C D G I; pinned stdenv env check | a useful exact key, possible future adapter |
| configureFlags containing record | A D I; evaluator coercion | field only, no index |

No setup hook or generic builder executes during evaluation. Their build-time
messages therefore cannot substantiate an evaluation-only correlation policy.
The original 240-case matrix is retained as context-lifetime evidence; it does
not become evidence of package/store identities.

## Retained implementation and matching rules

`backend.rs` reads structured IR before rendering. It retains dependency fields,
known list prefixes (including bounded nested lists), supplier and consumer
origins, mkDerivation origins, demanded assignment scopes and owner edges.
Lexical bindings, direct-application named defaults, plain record merges,
literal conditions and pinned concatLists/optional/optionals shapes are followed
without executing Nix. `callPackage` can inject an automatic argument instead of
an authored default, so such defaults are unknown for shape inference. Unknown
feature-dependent names retain at most eight exact literal alternatives, matched
against the actual diagnostic owner; they never guess a flag's default. Pinned flatten preserves discovery of package edges, but its list
shape is deliberately unindexed. A projection-only pinned Perl withPackages
callback contains only upstream selections, so it cannot introduce a competing
Rust-authored boundary. Arbitrary callbacks and other helpers remain opaque.

Only unmodified standalone pinned package/library namespaces identify backend
functions. Overlays, custom stdenv/lib, opaque overrides and module-supplied
package sets do not inherit those assumptions. No rendered function-name rule
or runtime boundary was added. This is a bounded IR inspection for one pinned
backend adapter, not a generic symbolic evaluator or a full value mirror.

`Generated.backend_metadata` is an optional, hidden artifact JSON field with
private version 1 and the pinned nixpkgs revision. Old maps without it load;
unsupported versions/revisions or malformed tables are ignored. Existing spans
are identical. No public Rust authoring interface or backend-boundary enum was
introduced. Adding the artifact field requires explicit Rust `Generated`
struct literals to supply it or use `..Generated::default()`; serialized older
maps remain compatible.

Upstream parsing stays in `diagnostic.rs`. It requires:

1. The exact dependency-validator grammar, a positive one-based index chain,
   and a supported dependency field. Nested indexes are reversed and normalized
   to zero-based outermost-first paths.
2. The checked pin's actual make-derivation.nix throw frame at 284:14.
   An unrelated throw with the same English wording is insufficient. A changed
   pin/source location disables matching until the adapter is reviewed.
3. Corroborating field/full owner frames, their ordered parent chain, and a
   generated frame establishing the actively demanded assignment.
4. An indexed child in every plausible known boundary. Unknown list lengths,
   backend-appended check inputs and unexamined/opaque competing descendants
   cannot justify selecting one convenient candidate.

JSON `raw_msg`/`trace` is preferred; the text adapter reads trace headers and
locations, excluding source excerpts, and normalizes the opposite trace order.
Both recover the same six real delayed suppliers. The Nix reason and raw stderr
remain byte-for-byte unchanged. Stronger direct generated failure evidence
wins; validated correlation precedes the outer opaque call or demand fallback.
Assertion-condition operation IDs are also retained: if a generated trace shows
an active guard, correlation cannot blame its unforced body. A same-name
package failure inside an assertion guard demonstrated a false match during
review; the retained regression test verifies the conservative veto. This adds
no Nix contexts and leaves body failures after successful guards eligible.

The confidence distinction is explicit in the origin set: a unique supplier is
`Primary` with `BackendCorrelation`; indistinguishable suppliers are
`BackendCandidate` alternatives with **no primary**. Candidates are not falsely
labelled simultaneous causal contributors. The test with two same-name child
recipes reports both Rust origins. Identical parent/child names preserve owner
depth rather than collapsing distinct calls. An unselected same-name assignment
is excluded by scope. A foreign same-name dependency or unresolved automatic
default prevents a unique match; opaque helper/override arguments are not treated
as the resulting recipe.

## Before / after diagnostics

Each comparison reinterprets the **same** real Nix failure with metadata enabled
and disabled. This avoids changes in temporary-store paths or evaluator order.
Artifacts are `target/backend-correlation/real/*-{before,after}.json`, with
current generated maps beside them. `option_path` remains absent for these
standalone package configurations; `set result` ancestry and the backend
field/consumer origins remain in `related`. No invented NixOS option path is
added.

| Logical failure | Before primary / evidence | After primary / evidence |
| --- | --- | --- |
| MariaDB integer curl | outer mariadb.drvPath / SourceMap | supplied integer / BackendCorrelation |
| MariaDB record curl | outer mariadb.drvPath / SourceMap | supplied record / BackendCorrelation |
| Git integer OpenSSL | outer git.drvPath / SourceMap | supplied integer / BackendCorrelation |
| curl integer OpenSSL | curl lowering.rs:324 / ErrorContext | same operation; unchanged |
| bad curl pkg-config through MariaDB | outer mariadb.drvPath / SourceMap | supplied pkg-config integer / BackendCorrelation |
| bad OpenSSL cryptodev through curl/MariaDB | curl lowering.rs:324 / ErrorContext | supplied cryptodev integer / BackendCorrelation |
| bad OpenSSL cryptodev through Git | outer git.drvPath / SourceMap | supplied cryptodev integer / BackendCorrelation |

The controlled supplier is an attributed `Expr::int` (or NixValue record), so
its Rust construction point survives rather than collapsing to the enclosing
argument record. Related origins include curl's native dependency consumer and
OpenSSL's cryptodev consumer. Owner names are not misreported as offending
package identities. Multiple independent dependencies in the real Git/MariaDB
lists and nested/propagated controls distinguish child indices successfully.

Coverage accounting:

- Field/owner information exists in 35/35 saved variants (100%). A coarse field
  lookup could narrow 30 delayed variants; the other five already have a precise
  consuming operation. It does not prove the child.
- Field/index clues exist in 30/35 (85.7%), six logical delayed cases. The retained
  implementation demonstrates exact supplier recovery in **6/6** ordinary
  unwrapped logical cases, and keeps the seventh coercion control unchanged.
- Store/drv identity clues identify the offending child in **0/35**. No identity
  algorithm, pre-forcing, result cache or package-content inspection is added.
- No delayed logical case in the ordinary known graph remains unresolved in this
  corpus. The four historical handoff instrumentation variants per case replace
  stdenv with a foreign function; the compiler intentionally treats those as
  opaque, not as proof of the pinned backend contract. The old context tests
  disable correlation explicitly to continue comparing wrapper behaviour.

## Cost, identities and laziness

The compiler audit compares compilation with the pre-existing lower/render path,
including origin-comment mode, and asserts identical source and span JSON. The
before/after metrics below are identical in both columns; backend argument
wrappers added: **0** throughout. Graph output selects MariaDB drvPath, retaining
the complete connected authored graph in generated source.

| Output | Bytes before = after | Lines before = after | addErrorContext before = after | Compact metadata bytes |
| --- | ---: | ---: | ---: | ---: |
| Git | 63,303 | 1,078 | 150 | 17,593 |
| curl | 35,482 | 690 | 113 | 2,021 |
| OpenSSL | 38,177 | 699 | 113 | 1,026 |
| MariaDB | 37,570 | 697 | 100 | 4,577 |
| composed graph | 233,205 | 3,504 | 481 | 7,627 |

Git retains two exact name alternatives because its SVN default is unknown
to the collector; the diagnostic picks the matching name. The graph has three
boundaries (MariaDB, curl, OpenSSL). Metadata has a compile/artifact storage cost; zero generated/runtime
instrumentation cost is not a claim of zero compiler work. Inspection is bounded
by recursion/visit limits, a 4,096-child prefix cap and a 1,024-byte name cap,
and stops conservatively when exhausted. A compact lexical list representing
65,536 elements verifies bounded retention with identical generated Nix. No identities
are obtained early: malformed integers/records have no store identities, paths
can have several suppliers, and this corpus has no causal path clue to join.

Existing exact-recipe tests compare Git/curl/OpenSSL/MariaDB outputs and drv/output
paths against pinned nixpkgs, including all four MariaDB families and composed
OpenSSL → curl → MariaDB / OpenSSL → Git edges. Generated Nix is identical, so
context metadata cannot affect hashes or recipes. Existing and focused tests
also cover unused named defaults, excluded dependencies, unused fields, finalAttrs
recursion, earlier validation failure and partial graph branches. Focused
successful evaluation matches with metadata enabled/disabled. No evaluation
semantic change or new forcing occurs.

## Remaining gaps and policy

Keep source maps for ordinary expressions and runtime contexts for meaningful
fallible operations. Add **optional structured backend argument metadata plus
conservative pinned adapters** where a real backend reports a usable index/key.
The smallest demonstrated useful granularity is a dependency field's known list
prefix and supplier/consumer origins, tied to a demand scope and owner chain.
A field-only table cannot recover the demonstrated bad dependency child.

Do not generalize to store-path joins or arbitrary English matching yet. Raw
path/name occurrence alone is not causal identity, and duplicate package names
are real ambiguity. Fields/indices lost through unknown transformations,
post-success outPath coercions, anonymous records, unindexed configure/reference
validation and foreign or overridden package scopes remain on the existing
fallback. Merged check-input normalization and env/output-key adapters are
tractable follow-up work, not fundamental impossibilities. Truly unindexed,
identity-free errors need better structured Nix/backend diagnostics; structured
owner/field/index events would remove the pin-specific message adapter. When
backend processing destroys even those identifiers, evaluator-level value
provenance remains the promising broader mechanism. No Nix fork is implemented.

## Verification and retained state

Final commands and results are recorded in
`target/backend-correlation/complete.json`; complete logs are
`target/backend-correlation/complete-*.log`. Earlier review runs remain under
`final-*` and `verified-*` as intermediate evidence. The fixture workflow includes the new
correlation suite and preserves all diagnostic snapshots and unmapped cases.
All final checks passed on the retained source:

- `cargo fmt --all --check`; structural spacing (172 Rust files, zero gaps);
  `git diff --check`.
- `cargo test --workspace --locked`: **543 passing tests in 40 groups**, including
  67 compile-fail fixtures checked by the UI test, 15 correlation tests and two
  metadata compiler audits.
- Git 44, curl 23, OpenSSL 8, MariaDB 8, composed 16, PostgreSQL 57 and PostgreSQL
  schema 4 passed; all old provenance/evaluator/parser tests passed too.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`.
- `cargo doc --workspace --no-deps --locked`, also with
  `RUSTDOCFLAGS='-D warnings'`.
- `bash scripts/check-fixtures.sh`: diagnostic snapshots/CLI artifacts,
  UI checks, the new correlation suite, all 15 examples and all registered
  package/PostgreSQL/composed suites passed. No snapshots changed.

The implementation/report are one coherent commit on `backend-provenance`, above
`e00fe587`; the worktree remains at
`/home/user/dev/worktrees/rusnix-backend-provenance`. Primary checkout is still
clean at `fcbdf6b742e400327ed3dc7f02b280fb79929439`. Only configured human identity
`rndhouse <rndhouse@protonmail.com>` is used. No Nix fetch/build/profile/host-store
or privileged operation occurred. Ignored evidence and verification artifacts
remain available in the retained worktree.
