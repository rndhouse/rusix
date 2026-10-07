# Runtime diagnostic boundaries

Byte spans and semantic ancestry are retained on every attributed expression.
Runtime `addErrorContext` markers supplement that map only at boundaries where
Nix can lose the useful Rust operation. Fine-grained origin comments are optional
human inspection aids; normal generated Nix omits them.

The [backend handoff investigation](backend-provenance-investigation.md) tests
whole-field and element contexts with real stdenv dependencies. Contexts survive
deferred evaluation exceptions but end when a value evaluates successfully;
they do not fix later backend type rejection or forcing of container children.
The runtime boundary policy remains unchanged.

The audit uses the existing isolated evaluator (Nix 2.34.8), pinned nixpkgs
`8b27c1239e5c421a2bbc2c65d52e4a6fbf2ff296`, and live evaluator locations.
No generated-source rewriting, fetching or building is involved.

| Expression category | Previous context | Can fail when demanded? | Source positions sufficient? | Current policy |
|---|---|---|---|---|
| Literal / lexical variable / function definition | No | A variable may force a deferred value | Definition positions do not identify a failed call | Spans only; calls carry their boundary |
| List / record / assignment | No | Children can fail later | Child operation spans retain container ancestry | Spans only; no forcing |
| Ordinary opaque attribute selection / literal dynamic keys | Yes | Missing field, wrong record type, deferred child | Yes: generated `getAttr` frame | Spans only |
| Lexical argument path / `args` navigation | Yes | Missing nested field, wrong record type | Yes: error position at selection, ahead of callback frames | Spans only |
| Final `config.*` option / `options` navigation | Yes | Foreign definition may throw | Missing fields map; a throwing final definition can omit the generated lookup frame | Retain final-option boundary |
| Boolean combinations / ordinary conditional | Yes (`If`) | Backend operand may not be boolean | Yes: generated branch-condition position | Spans only |
| Equality | Yes | Deferred operands may fail | Operand failure positions retain equality ancestry | Spans only |
| Division / generated range comparisons | Division only | Zero denominator / invalid operand | Yes: generated builtin frame | Spans only |
| `builtins.toString` | Yes | Unsupported value or failing conversion | Yes in tested scalar and child failures | Spans only |
| String prefix (`+`) | Yes | Backend expectation may disagree with value | No: Nix can point at the operand instead of the prefix operation | Retain coercion boundary |
| Text concatenation / interpolation | Calls and coercions | Deferred text or library function may fail | Literal/list/coercion children map | Retain opaque library application; remove child coercion markers |
| Function application (caller function, lib, builder, fetcher, override) | Yes | Function/schema/external implementation failure | Not reliably: positions can identify the function lookup instead of the call | Retain application boundary |
| Curried application with one Rust caller identity | Every partial application | Intermediate or final call may fail | One outer marker covers partial applications while demanded | One marker; keep all spans |
| Imported opaque value / nixpkgs lookup | Yes | Imported code can fail outside generated source | Not reliably for arbitrary external code | Retain interop boundary |
| Native attribute-set union (`//`) | New curl operation | An operand may not be an attribute set | No in tested cases: Nix reports only the enclosing field | Retain a narrow union boundary; child failures keep their own origins |
| Native expression assertion | New curl operation | False/wrong-type condition | Yes for the native assertion position; distinct from NixOS assertions | Spans only |
| Builtin function lookup | New curl operation | Missing builtin or later argument-type failure | A builtin definition position must not override its consuming call boundary | Lookup spans; runtime context only on application |
| Pinned source path construction | Yes | Validated literal path construction has no external computation | IR validation covers path data | Spans only |
| Explicit range validation | Yes | Constraint can reject | Positions work, but explicit validation is a useful diagnostic boundary | Retain validation marker |
| Imported-module category check | Yes | Handle can have wrong category | Explicit validation/import boundary | Retain marker |
| NixOS failed assertions | Stage marker and message metadata | False assertion / failing condition | Assertion metadata identifies the rule; child spans identify failing conditions | Keep existing stage/message mechanisms |
| NixOS type / merge / unknown-option errors | Definition metadata, not per-value contexts | Module processing rejects definitions | `_file` metadata retains independent origins | Unchanged; do not blame foreign definitions on schema Rust |

## Diagnostic evidence

[Expression comparisons](../crates/rusix-nix/src/context_audit.rs) render both
the previous wrapper policy and the current policy from the same AST. They
check equal primary Rust origins and reasons, retained `package.buildInputs`
ancestry and raw Nix errors. They also replay real messages through the textual
fallback. Cases cover missing direct/nested `args` fields, invalid conditions,
equality with a failing child, division, string coercion, prefix coercion,
wrong library arguments, `writeText`, deferred `mkDerivation` recipe errors,
explicit validation, nested callbacks, delayed external values, imported Nix,
and an actual `mkDerivation` `finalAttrs` callback failure.

[NixOS comparisons](../crates/rusix-nix/src/nixos_context_audit.rs) additionally
check explicit assertions and the complete PostgreSQL schema/implementation:
the same generated artifact accepts an ordinary downstream port override, then
maps a zero-port division to its consuming Rust operation. A negative port is
rejected through the public PostgreSQL schema, with no false Rust blame for the
foreign definition. A Rust-defined setting that fails while a separate
contribution reads it retains the original writer operation and path.

Ordinary failures now report `Provenance::SourceMap` instead of
`Provenance::ErrorContext`; the Rust operation and enclosing semantic path
remain the same. Two committed division snapshots change only that provenance
label. Raw Nix traces are naturally shorter.

The translator prefers the error's own generated position over function trace
frames. A precise unwrapped operation at the error position or before the innermost
runtime marker can outrank that boundary. This also works across independently
generated NixOS definitions; static ancestry supplies the failing contribution
and path rather than pretending those definitions are physically nested. Literals and
function definitions cannot displace a consuming call. The source map records
this distinction as `diagnostic_site`, defaulting to false for older maps.
External-file positions are never mapped as generated Rust code.

The JSON adapter uses structured positions and trace messages; the legacy text
adapter remains a tested heuristic. Nix versions with different trace metadata
may require an adapter update. ID-only recovery still cannot distinguish cloned
occurrences if generated positions are absent. A lazy derivation may fail only
when a result attribute is selected, after the constructor's runtime context
has unwound; the selection is then the useful boundary, with raw Nix details
retained. No eager forcing is added to keep constructor contexts alive.

## Origin identifiers

Rusix identifies source origins with IDs such as `rn-e160b9de21c72674`.
Generated Nix includes the ID at selected failure boundaries in `addErrorContext`.
Inspection rendering also includes comments (`# rn-e160b9de21c72674`). Source-map
and diagnostic metadata use that ID unchanged. NixOS definition files and assertion messages
also carry the same ID; their role is recorded separately in the artifact.
Readers recognize only complete IDs with 16 lowercase hexadecimal digits.

## Optional inspection rendering

The provenance mechanisms have separate roles:

- Source maps carry fine-grained machine provenance.
- `addErrorContext` carries selected runtime provenance.
- NixOS definition/schema/assertion metadata carries module provenance.
- Origin comments show the same IDs to a human inspecting generated Nix.

No diagnostic path consumes origin comments. Rendering calculates spans as it
emits expressions, independently of whether a preceding comment is requested.
Normal and inspection maps contain the same origins, ancestry and diagnostic-site
flags, but different byte offsets. Always save a map with its own rendered source;
removing comments afterward would invalidate its offsets.

The comment dependency audit found:

| Site / role | Classification | Contract |
|---|---|---|
| Runtime and NixOS diagnostic readers | A: none required | Consume positions, contexts and module metadata; never origin comments |
| Renderer offsets and saved source text | B: layout-dependent | Recompute offsets for each rendering; do not edit rendered text afterward |
| Expression comments and annotated documentation | C: human/debug aid | Available only when explicitly requested |
| Round-trip comment presence/alignment tests | D: inspection convention | Check debug comments; normal output asserts their absence |
| Persisted source/map tests | D: machine consistency | Both modes retain identical origins and valid per-output spans |
| Parser rejection of IDs in source excerpts | D: diagnostic safety test | Comments cannot masquerade as runtime frames |
| Git comment/context regression | D: output policy | Normal has no origin comments; debug has one per attributed expression |
| Dead comment-reading logic | E: none found | No obsolete parser or migration code is needed |

Normal compilation, CLI artifacts and examples use comment-free expression output.
There is still one compiler-owned header comment. Inspection is explicit:

```rust
use rusix_ir::IntoConfig;
use rusix_nix::{RenderOptions, compile_with_options};

#[derive(IntoConfig)]
struct Output {
    enabled: bool,
}

let annotated = compile_with_options(
    Output { enabled: true },
    RenderOptions { origin_comments: true },
).unwrap();
```

The same option is available through `nixos::compile_module_with_options` and,
for backend AST authors, `render_with_options`. One renderer handles both modes.
There is no global mode, duplicate default artifact, or CLI inspection flag.

[Rendering comparisons](../crates/rusix-nix/src/render_audit.rs) check persisted
maps, nested structures, source-map-only failures, cloned expression occurrences,
lazy siblings/guards, discarded priorities and eight NixOS failure cases. The
expression matrix additionally compares both renderings for twenty demanded
failures, including calls, lookups, interpolation and nested callbacks. Comparisons
require identical diagnostic kind, primary/related/causal origins, option path,
provenance and underlying reason. Raw traces retain their own generated positions
and source excerpts, which naturally differ between renderings.

## Git output before width-aware layout

| Metric | Before boundary audit | After boundary audit | Compact IDs / inspection | Normal output |
|---|---:|---:|---:|---:|
| Lines | 2,537 | 2,537 | 2,537 | 200 |
| UTF-8 bytes | 218,566 | 158,441 | 121,509 | 60,923 |
| `addErrorContext` calls | 1,226 | 301 | 301 | 301 |
| Origin comments | 2,337 | 2,337 | 2,337 | 0 |

The boundary audit removed 925 runtime wrappers and 60,125 bytes. Compact IDs
remove another 36,932 bytes while keeping every ID, comment and runtime boundary.
Making inspection comments optional saves another 60,586 bytes (49.9% of the
annotated output). Both renderings retain 2,337 source-map spans and 619 distinct
mapped origin IDs. Normal Nix text contains 150 distinct IDs at runtime boundaries;
inspection text shows all 619. The machine mapping loses no origins.

These figures measure the printed `git-nixpkg` example, including its final newline.
Removing comments eliminates line breaks, leaving longer expressions; this change
does not redesign the generated-source formatter.

The [source-map-aware pretty-printer](generated-nix-layout.md) subsequently changes
layout in both modes. Its measurements distinguish structural lines from
indivisible literals; origin, source-span and runtime-context counts are unchanged.
