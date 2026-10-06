# Runtime diagnostic boundaries

Generated comments, byte spans and semantic ancestry are retained on every
attributed expression. Runtime `addErrorContext` markers supplement that map
only at boundaries where Nix can lose the useful Rust operation.

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
| Pinned source path construction | Yes | Validated literal path construction has no external computation | IR validation covers path data | Spans only |
| Explicit range validation | Yes | Constraint can reject | Positions work, but explicit validation is a useful diagnostic boundary | Retain validation marker |
| Imported-module category check | Yes | Handle can have wrong category | Explicit validation/import boundary | Retain marker |
| NixOS failed assertions | Stage marker and message metadata | False assertion / failing condition | Assertion metadata identifies the rule; child spans identify failing conditions | Keep existing stage/message mechanisms |
| NixOS type / merge / unknown-option errors | Definition metadata, not per-value contexts | Module processing rejects definitions | `_file` metadata retains independent origins | Unchanged; do not blame foreign definitions on schema Rust |

## Diagnostic evidence

[Expression comparisons](../crates/rusnix-nix/src/context_audit.rs) render both
the previous wrapper policy and the current policy from the same AST. They
check equal primary Rust origins and reasons, retained `package.buildInputs`
ancestry and raw Nix errors. They also replay real messages through the textual
fallback. Cases cover missing direct/nested `args` fields, invalid conditions,
equality with a failing child, division, string coercion, prefix coercion,
wrong library arguments, `writeText`, deferred `mkDerivation` recipe errors,
explicit validation, nested callbacks, delayed external values, imported Nix,
and an actual `mkDerivation` `finalAttrs` callback failure.

[NixOS comparisons](../crates/rusnix-nix/src/nixos_context_audit.rs) additionally
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

## Git output

| Metric | Before | After |
|---|---:|---:|
| Lines | 2,537 | 2,537 |
| UTF-8 bytes | 218,566 | 158,441 |
| `addErrorContext` calls | 1,226 | 301 |
| Origin comments | 2,337 | 2,337 |

Origin comments are deliberately unchanged. The reduction is 925 runtime
wrappers and 60,125 bytes; it does not depend on an arbitrary size target.
