# Generated Nix layout

Rusnix formats its existing Nix AST with a document pretty-printer. The target is
100 characters, with two-space indentation. It changes whitespace only: AST
structure, expression tokens, parentheses, runtime contexts and NixOS metadata
remain unchanged. String contents, quoted paths and numeric spelling are preserved.
An indivisible literal may exceed the width target.

## Renderer audit and design

The previous renderer concatenated tokens directly. Literals, variables,
selections, applications, lambdas, argument patterns, conditionals, lets, binary
operators and builtins stayed inline unless a child introduced a newline.
Attribute sets always inserted newlines; inspection comments inserted additional
ones. There was no available-width calculation. Removing normal-mode origin
comments therefore exposed enormous physical lines.

The replacement has one layout pipeline:

```text
Nix AST → attributed documents → width-aware emission → Nix text + source spans
```

Documents contain text, soft breaks, mandatory breaks, concatenation, nesting,
groups and attribution. A group stays flat if its cached width and following
suffix fit the remaining line. Otherwise its soft breaks become newlines. Nested
groups independently decide whether they fit. Parentheses retain their existing
spelling and do not add indentation by themselves.

Each document caches its flat width once. Final emission uses an explicit command
stack; fit lookahead stops at the first break or exhausted width and does not
rescan large candidate subtrees. Width counts characters; persisted source-map
positions remain UTF-8 byte offsets.

The [document engine](../crates/rusnix-nix/src/render/layout.rs) retains origin and
diagnostic-site metadata. Attributed documents open a range before their first
output byte and close it after their contents, including nested layout. The
emitter records occurrence-specific spans and enclosing origins at that point.
No external formatter or post-render whitespace transformation is used.

## Construct rules

| Construct | Layout |
|---|---|
| Literal, variable, path | Unbroken text; preserve spelling and string escaping |
| List / attribute set | Inline if the entire group fits; otherwise one item/binding per line |
| Binding | Keep value inline when possible; otherwise indent its expression |
| Application / builtin / runtime context | Break arguments onto indented lines; preserve all existing parentheses |
| Lambda | Inline short bodies; indent long bodies |
| Native function argument set | Inline short patterns; otherwise one argument per line, including dependent defaults |
| Conditional | Inline short branches; otherwise indent branches and align `else` |
| Let | Inline tiny bindings; otherwise separate binding, `in` and body |
| Binary operation | Inline short operands; otherwise break before the operator |
| Selection | Preserve parenthesized base and exact attribute syntax; base expressions can break |
| Inspection comment | Mandatory newline before the expression; outside its own attributed range |

Normal and inspection modes use the same document engine. Inspection adds only
origin comments. Both retain all 2,337 spans and 619 mapped origins in the Git
example, with byte offsets calculated for their respective output.

## Git measurements

These figures include the example's final printed newline. Percentiles use the
nearest-rank convention; line lengths count characters.

| Metric | Normal before | Normal after | Inspection before | Inspection after |
|---|---:|---:|---:|---:|
| Lines | 200 | 2,992 | 2,537 | 5,983 |
| Bytes | 60,923 | 169,616 | 121,509 | 336,005 |
| Maximum line length | 6,220 | 920 | 908 | 920 |
| Median line length | 35 | 49 | 33 | 57 |
| 95th percentile | 1,447 | 94 | 154 | 75 |
| Lines over 100 | 58 | 89 | 168 | 89 |
| Lines over 200 | 38 | 27 | 24 | 27 |
| Origin comments | 0 | 0 | 2,337 | 2,337 |
| Runtime contexts | 301 | 301 | 301 | 301 |

Normal output's maximum falls 85.2%, and its 95th percentile falls 93.5%.
Separating previously crowded literals creates more individual over-width lines
in normal mode. Every remaining over-width line contains an indivisible quoted
literal; replacing quoted contents with empty quotes leaves no line over 100.
Inspection's maximum rises slightly because literals acquire indentation, while
its 95th percentile falls to 75. The layout uses more bytes for indentation and
line breaks; minimizing output size is not this formatter's goal.

A debug-build sample of 100 complete Git compilations took 759 ms before and
1,128 ms after (roughly 7.6 versus 11.3 ms per artifact). This measures validation,
AST lowering, rendering and source maps together. It is a rough observation, not
a microbenchmark or a timing assertion. The additional cost is about 3.7 ms per
substantial example artifact.

## Representative fragments

These excerpts omit surrounding expressions and later arguments/items. Ellipses
below indicate omitted source, not new Nix syntax or modified string contents.

### Factory arguments

Before:

```nix
"factory" = (({ fetchurl, fetchpatch, lib, stdenv, buildPackages, curl, ... }:
```

After:

```nix
"factory" =
  (({
    fetchurl,
    fetchpatch,
    lib,
    stdenv,
    buildPackages,
    curl,
    ...
    perlSupport ? (((stdenv).buildPlatform == (stdenv).hostPlatform)),
    osxkeychainSupport ? ((stdenv).hostPlatform.isDarwin),
    ...
  }:
```

### Dependency lists

Before:

```nix
"buildInputs" = (builtins.addErrorContext "rn-3b2aa87a28144747" ((((lib).concatLists) ([ [ curl openssl zlib expat cpio ... ] ... ]))));
```

After:

```nix
"buildInputs" =
  (builtins.addErrorContext
    "rn-3b2aa87a28144747"
    ((((lib).concatLists)
      ([
        [
          curl
          openssl
          zlib
          expat
          cpio
          (if (stdenv).hostPlatform.isFreeBSD then
            libiconvReal
          else
            libiconv
          )
          bash
        ]
        ...
```

### Nested applications

Before:

```nix
(builtins.addErrorContext "rn-bb77e2c7da7ff041" ((builtins.getAttr ("concatStringsSep") ((builtins.import (./nixpkgs/lib))))))
```

After, within `postPatch`:

```nix
(builtins.addErrorContext
  "rn-bb77e2c7da7ff041"
  ((builtins.getAttr
    ("concatStringsSep")
    ((builtins.import (./nixpkgs/lib)))
  ))
)
```

### Complex text fields

Before, a fragment in `postPatch`:

```nix
([ "# Fix references ..." (builtins.toString (gettext)) "\n\n# ensure ..." ])
```

After:

```nix
([
  "# Fix references ..."
  (builtins.toString (gettext))
  "\n\n# ensure ..."
])
```

The complete literal bytes remain unchanged. The actual generated file keeps
those full strings, even when an individual literal exceeds 100 characters.
`postInstall` uses the same list layout. The factory guards, `mkDerivation`,
`nativeBuildInputs`, `buildInputs`, `configureFlags`, `makeFlags`, `postPatch`,
`postInstall`, `meta` and `passthru` were inspected in the complete output.

## Diagnostic and semantic evidence

A temporary baseline experiment captured twenty expression failures and eight
NixOS failures before replacing the renderer. Afterward, every structured field
matched: kind, reason, primary/related/causal origins, option path, provenance and
external file. Original raw stderr remains available, with different generated
positions and source excerpts. The temporary test edits were reverted.

The existing paired-rendering matrix checks normal versus inspection diagnostics,
source-map-only failures, reused expressions, lazy siblings/guards and discarded
priorities. Twelve focused [renderer tests](../crates/rusnix-nix/src/render/tests.rs)
cover short/long layouts, argument defaults, nesting, conditionals, lets,
applications, literals, all AST variants, deterministic text/maps, and a real
failure after inline Unicode text. Token comparison across flat, formatted and
inspection documents confirms that layout retains parentheses and literal data.

The Git equivalence suite compares exact derivation recipes and full projections;
PostgreSQL compares its complete implementation and public option declarations.
Runtime instrumentation and its placement are unchanged. No legacy renderer or
format-compatibility mode is retained. Parenthesis/application simplification is
explicitly reserved for a separate task.

Verification passed: 428 workspace tests; 41 Git cases with the unchanged default
`/nix/store/qk30rjnnsz9lxdp9b6k5hbjj7p8w17pg-git-2.47.0.drv`; 57 PostgreSQL
implementation and four schema cases; and 22 Curl cases. The fixture workflow,
UI tests, diagnostic snapshots and all 13 examples passed. Formatting, Clippy
with warnings denied, ordinary/strict rustdoc and the structural spacing check
(135 Rust files, zero missing gaps) passed. Evaluation used only disposable
isolated stores; no packages were fetched or built.
