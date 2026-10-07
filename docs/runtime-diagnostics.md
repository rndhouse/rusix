# Runtime diagnostics

Rusix connects Nix evaluation failures to the Rust code that produced the
expression. It retains the original Nix reason and stderr alongside the recovered
Rust location. Some upstream failures have only a broad boundary or no Rust
origin; diagnostics preserve that limitation.

## Mapping failures to Rust

Source maps record generated UTF-8 byte spans, Rust origins and enclosing semantic
origins. Generated evaluator positions can identify a failing operation and its
configuration path, including when evaluation is delayed inside a container.
External-file positions are never treated as positions in generated Rust code.

Selected `builtins.addErrorContext` markers supplement source spans when Nix may
lose the useful generated operation. NixOS definition, schema and assertion
metadata retain module origins independently.

| Expression or boundary | Mapping policy |
| --- | --- |
| Literal, variable, function definition, list, record or assignment | Source spans and ancestry; no container forcing |
| Ordinary attribute or argument lookup, condition, equality, division or `toString` | Generated positions identify the consuming operation |
| Opaque application, imported value or nixpkgs lookup | Retain a runtime boundary |
| Final NixOS option dependency, string-prefix coercion or shallow record union | Retain a narrow runtime boundary |
| Explicit validation or imported-module category check | Retain a validation boundary |
| NixOS assertion, option type, merge or unknown-option error | Use assertion or module metadata |

The translator prefers a concrete generated failure position to function trace
frames. A precise child operation can outrank an enclosing runtime marker.
Literals and function definitions cannot displace a consuming operation.
Structured JSON trace fields are preferred; the text fallback uses narrower
heuristics and may need updates for different Nix diagnostic formats.

## Delayed backend validation

A runtime context ends when its wrapped expression evaluates successfully.
A returned list or record can have children forced later, and stdenv can reject
a successfully evaluated dependency afterward. Extra wrappers cannot recover
those supplier origins. Rusix preserves laziness without forcing containers
to keep contexts alive.

For the pinned stdenv dependency-type validator, optional compiler-owned backend
metadata can correlate an owner, dependency field and list index with a Rust
supplier. Direct generated failure evidence takes precedence. A unique validated
supplier uses `Provenance::BackendCorrelation`; indistinguishable suppliers are
reported as candidates without a primary origin.

Unknown overrides or custom backends, unknown list shapes, backend-appended input
tails and failures without corroborating owner frames retain their existing
fallback. Unsupported metadata is ignored, and older artifacts without it still
load. Correlation adds no runtime contexts or early evaluation.

## Origin identifiers

Origins use IDs such as `rn-e160b9de21c72674`, shared by source maps, diagnostic
JSON, selected runtime contexts and NixOS metadata. IDs describe source locations
and semantic purposes, rather than unique runtime instances. Moving Rust source
or changing its line positions changes IDs.

Concrete generated positions distinguish cloned occurrences. Without positions,
ID-only recovery may choose the first matching span and yield an ambiguous path.
Captured Rust locations are points, so diagnostics underline a single caret.

## Inspection rendering

Normal compilation omits fine-grained origin comments. Request them explicitly
when inspecting generated Nix:

```rust
use rusix::{IntoConfig, RenderOptions, compile_with_options};

#[derive(IntoConfig)]
struct Output {
    /// Whether the generated configuration is enabled.
    enabled: bool,
}

let annotated = compile_with_options(
    Output { enabled: true },
    RenderOptions { origin_comments: true },
).unwrap();
```

The option is also available through `nixos::compile_module_with_options` and
`render_with_options`. Origin comments are human inspection aids; diagnostics
consume positions, runtime contexts and module metadata.

Normal and inspection maps retain the same origins, ancestry and diagnostic-site
flags, but have different offsets. Always keep each map with its own rendered
source. See [generated Nix layout](generated-nix-layout.md) for formatting rules.
