# Nix expression rendering

The backend renders the existing AST with Nix's precedence and grammar rules,
then lays out the attributed document at a 100-character target width. It does
not reassociate operators, rewrite expressions, or post-process source text.
The [layout engine](generated-nix-layout.md) emits source spans directly from
the resulting text in both normal and inspection modes.

## Audit and precedence policy

The earlier renderer grouped every application, its function and its argument,
every selection base, and every binary/conditional/function expression. This
was safe but obscured ordinary Nix calls and paths. The new
[policy](../crates/rusix/src/compiler/render/precedence.rs) uses increasing binding
strength, based on the [Nix operator table](https://nix.dev/manual/nix/2.24/language/operators.html)
and the [Nix parser grammar](https://github.com/NixOS/nix/blob/master/src/libexpr/parser.y).

This table covers the currently represented syntax, from weakest to strongest.
“Group left/right” concerns a child of equal binding strength; weaker children
are always grouped in operator/application/selection positions.

| AST category | Association | Group left | Group right | Application function | Application argument / list item | Selection base |
|---|---|---|---|---|---|---|
| Lambda, argument-set function, `let`, `assert`, `if` | Full-expression forms | n/a | n/a | Group | Group | Group |
| `&&` | Left | No | Yes | Group | Group | Group |
| `==` | Non-associative | Yes | Yes | Group | Group | Group |
| `>=`, `<=` | Non-associative | Yes | Yes | Group | Group | Group |
| `//` | Right | Yes | No | Group | Group | Group |
| `+` (numeric, string or path) | Left | No | Yes | Group | Group | Group |
| Negative numeric literal | Unary negation | n/a | n/a | Group | Group | Group |
| Application, builtin call, runtime error-context wrapper | Left | No | Yes | Direct | Group | Group |
| Selection, lexical argument selection | Attribute path | Direct chain | n/a | Direct | Direct | Direct |
| Bool, null, nonnegative number, string, variable | Atomic | n/a | n/a | Direct | Direct | Direct |
| List / attribute-set literal | Atomic | n/a | n/a | Direct | Direct | Direct |
| Path literal | Atomic, lexical exception | n/a | n/a | Direct | Direct | Group |
| Explicit AST `Group` | Atomic | n/a | n/a | Direct | Direct | Direct |

Selection is stronger than application. Nix list items and application arguments
accept selections or simple expressions, rather than arbitrary operator or
application expressions. Consequently `[ (f x) y ]` contains two elements;
`[ f x y ]` would contain three. Attribute-set arguments need no extra group:
`f { key = value; }` is valid Nix syntax.

Paths need a lexical exception: `./source.path.field` is one path token, not a
selection. The AST's path selection therefore renders `(./source.path).field`.
This also applies to a path reached through an empty selection/view root.
Negative application arguments remain grouped (`f (-42)`). The minimum signed
integer retains its existing grouped subtraction spelling, since its positive
magnitude cannot be expressed as a signed Nix integer literal.

No unary-operator, subtraction, infix division/multiplication, `||`, `++`, `?`,
implication or pipe node exists in the current AST. Integer division is a call
to `builtins.div`; boolean negation is already expressed by existing conditionals.
The renderer introduces no syntax or precedence entries for absent operators.

Full-expression positions—attribute values, defaults, lambda bodies, let values
and bodies, and conditional branches/conditions—do not need blanket parentheses.
Explicit `Group` nodes retain their documented parentheses. Those nodes also
carry assignment/reference attribution; they are not removed or merged here.

## Applications, selections and attribution

Left-nested application nodes render as a visible application spine. Their
documents and spans still nest separately: the first application covers `f a`,
and the next covers `f a b`. The AST itself is unchanged. Each argument gets a
break opportunity with two-space indentation, so long calls remain width-aware.

Selection paths use identifiers when safe, otherwise escaped quoted attributes.
For example, `perlPackages.perl.libPrefix` is a normal path, while
`value."literal.dot"."quoted\"name"` retains literal attribute data. Reserved
words are quoted. Hyphenated identifiers such as `pkg-config` remain valid.

A runtime `addErrorContext` wrapper has application precedence regardless of
the child's kind. The child is grouped only when the argument grammar requires
it; the surrounding parent context then decides whether to group the wrapper.
Runtime contexts, origin IDs and ancestry entries are preserved. Empty view paths
remain attributed but are classified as identity expressions rather than failing
lookups; their rendered text is just the child variable. This avoids blaming a
view root instead of the attribute selection that consumes it in either mode.

Two independently attributed lookup operations retain a boundary group, such as
`(value.present).missing`. Without it Nix folds both steps into one path and
reports the same start for either missing attribute, making their Rust origins
indistinguishable. The boundary parenthesis belongs to the parent syntax, outside
the base lookup's own span. This exception is tested for failures at both steps
in both modes. A single structural accessor with several path segments still
renders directly, as do nested selections without independent attribution.

Removing `(input)` from `(input).missing` exposes an attribution ambiguity:
Nix reports the start of the lookup, which now overlaps the smaller variable
span. In inspection mode comments may separate the two starts. For actual error
positions and builtin/condition failure frames, diagnostic lookup uses the
nearest enclosing diagnostic-site span when the innermost span is non-fallible.
General source-position lookup still returns the smallest enclosing span, and
function-definition/call-site frames retain that behavior. This preserves blame
on the lookup rather than its parameter or an outer opaque-call boundary, without
using unnecessary parentheses as source-position markers. All spans and ancestry
remain available, including the variable's span.

Recovery cannot promote a position outside its innermost known runtime context.
For example, a missing final option remains attributed to its `OptionRef`, even
when wrapped in a string conversion. The structured and text diagnostic readers
use the same boundary limit; runtime-marker precedence is unchanged.

## Representative rendering changes

These fragments abbreviate surrounding source; literal contents and AST children
are unchanged. Selected runtime boundaries are still present in actual packages.

```nix
# Before
((((lib).optional) (perlSupport)) ((perlPackages).perl))
# After
lib.optional perlSupport perlPackages.perl
```

```nix
# Before
((((lib).concatLists) ([ ... ])))
# After
lib.concatLists [ ... ]
```

```nix
# Before: a package default
perlSupport ? (((stdenv).buildPlatform == (stdenv).hostPlatform))
# After
perlSupport ? stdenv.buildPlatform == stdenv.hostPlatform
```

```nix
# Before
(builtins.addErrorContext "rn-e160b9de21c72674" (((lib).optionalString) (enabled) ("text")))
# After
builtins.addErrorContext "rn-e160b9de21c72674" (lib.optionalString enabled "text")
```

Required groups remain visible: `f (g x)`, `(f x).out`,
`map (x: x.field) xs`, `f (if enabled then a else b)`,
`a + (b + c)`, `(a // b) // c`, and `(a == b) == c`.

## Git measurements

These figures include the example's final printed newline. Line lengths count
characters; the 95th percentile uses nearest rank. The AST, attribution density,
runtime-context policy and literal strings are unchanged.

| Metric | Normal before | Normal after | Inspection before | Inspection after |
|---|---:|---:|---:|---:|
| Lines | 2,992 | 2,136 | 5,983 | 4,967 |
| Bytes | 169,616 | 133,197 | 336,005 | 293,239 |
| Maximum line length | 920 | 920 | 920 | 920 |
| Median line length | 49 | 58 | 57 | 59 |
| 95th percentile | 94 | 97 | 75 | 75 |
| Lines over 100 | 89 | 90 | 89 | 90 |
| Lines over 200 | 27 | 27 | 27 | 27 |
| Origin comments | 0 | 0 | 2,337 | 2,337 |
| Runtime contexts | 301 | 301 | 301 | 301 |

Normal output is 21.5% smaller; inspection output is 12.7% smaller. The width
target remains 100. Indivisible escaped shell/text literals still account for
long lines; the additional over-width line is a literal that now fits beside
shorter surrounding syntax. This change optimizes grouping, not string layout.

## Verification

The focused renderer tests exercise every AST category in 1,520 parent contexts,
parsed by isolated Nix in both modes. Thirty-four concrete evaluations compare
minimal rendering with an oracle that explicitly groups every node of the same
AST. They include floating addition whose grouping changes the result, curried
applications, non-associative equality, set unions, defaults, special names,
negative numbers, and lazy discarded branches.

Eight added tests cover precedence, grammar, evaluation and attribution. Tests
also retain source spans for every application prefix and ensure missing
selections identify the consuming operation with no runtime context. Existing
determinism, Unicode byte-position, cloned-origin, selective-evaluation and
normal/inspection diagnostic tests remain enabled.

An additional temporary capture compared 28 existing diagnostic cases before and
after: 20 expression/interop cases and eight NixOS cases. Every Rust-facing field
matched, including primary/related origins, causal origin roles, provenance,
option path, kind and reason. Original raw Nix traces remain retained; their
generated locations and excerpts naturally differ. Temporary capture changes
were reverted; there is no legacy renderer or production compatibility branch.

The verified batch passes `cargo test --workspace --locked` (436 tests), format
checking, Clippy across all targets with warnings denied, ordinary and strict
rustdoc, and the structural spacing check (137 Rust files, zero missing gaps).
The fixture workflow passes all 13 examples, 60 compile-fail fixtures, 14 existing
diagnostic snapshots and persisted CLI checks. No snapshot expectations changed.
Git's 41 tests, Curl's 22, PostgreSQL's 57 implementation tests and four schema
tests pass. Git's default derivation remains
`/nix/store/qk30rjnnsz9lxdp9b6k5hbjj7p8w17pg-git-2.47.0.drv`.
All Nix evaluation uses the existing disposable isolated-store helper, without
network fetching or package builds.
