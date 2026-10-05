# Rusnix: user models, existing Nix objects, and an escape hatch

```text
User-defined Rust domain model
        ↓
#[rusnix::config] / structural lowering
        ↓
semantic IR → Nix AST → Nix backend
        ↓
Nix/NixOS ecosystem
```

Three coexisting authoring choices feed this pipeline: user-defined typed models,
opaque handles for existing Nix objects, and an explicit generic escape hatch.
They are not a mandatory sequence for every value.

Rusnix uses Rust's existing type system rather than defining a new configuration
type system. Domain types belong to users and libraries. Core supplies composition,
lowering, Nix interoperability, diagnostics and generic validation, without an
opinionated catalogue of service, networking, account or TLS types.

**Rusnix does not try to generate or maintain Rust bindings for the whole Nix
ecosystem.** User-defined Rust models are optional higher-level models used where
they provide useful invariants. Existing Nix objects cross an explicit opaque
boundary; arbitrary NixOS options remain reachable through `Config::set`.

NixOS already validates enums and structured options and enforces assertions.
These examples do not claim otherwise, or claim that every option needs a Rust
wrapper. Rust adds a static, program-wide type system for the code constructing
configuration, while keeping Nix's ecosystem and final checks underneath.

| Example | What Rust adds |
| --- | --- |
| [Enum option](enum-option.rs) | First-class static alternatives |
| [Typed submodule](typed-submodule.rs) | Semantic field types |
| [Invalid states](invalid-states.rs) | Invalid combinations have no representation |
| [Function contracts](function-contracts.rs) | Machine-checked caller contracts |
| [Exhaustive match](exhaustive-match.rs) | Model changes expose affected consumers |
| [Typed values](typed-values.rs) | Same-shaped values remain semantically distinct |
| [Layered validation](layered-validation.rs) | Clear division of validation responsibility |
| [Nix interop](nix-interop.rs) | Existing ecosystem works without generated bindings |
| [Symbolic option](symbolic-option.rs) | Explicit typed dependencies follow ordinary Nix overrides |
| [PostgreSQL](postgresql.rs) | Real module implementation with typed provisioning and NixOS compatibility |

Each example is **one Rust file**: user models, structural placement and lightweight
source generation. Examples invoke no Nix evaluator and write no artifacts;
integration tests prove the behavioral claims. There is no separate Rust support/config/main tree and
no handwritten mechanical lowering in the nine small examples. PostgreSQL adds
one semantic IntoConfig adapter for optional inputs and ownership-derived roles.
The first six use a fictional `demo` schema, not bindings for actual NixOS
services. Each file defines its own small types directly, including its enums,
newtypes and policies. Small definitions repeat intentionally so an example is
self-contained; there is no example-types crate. The examples use direct tuple
construction and only the derives needed for lowering. Mode keeps Clone/Copy
where the same choice feeds several by-value consumers. A few types, fields and
model functions remain public so integration/UI tests can import the actual
example code; other declarations are private. Production validation and builders
are deliberately omitted. Each main unwraps only compilation of its known-valid
model, rather than introducing a separate error-handling API.

`Transport::Tls` requires both credentials; invalid-states additionally gives
Certificate and PrivateKey distinct Rust types.

## Authoring and composition

Use `#[rusnix::config]` for local configuration trees. Use fine-grained derives
for reusable or external types. Use explicit conversion implementations when the
mapping itself carries domain meaning.

For local configuration, one inline module boundary supplies structural lowering:

```rust
use rusnix_ir::{self as rusnix, nixos::NixosModule};

#[rusnix::config]
mod config {
    #[rusnix(root)]
    pub struct Machine { services: Services }

    struct Services { example: Example }

    struct Example { enable: bool, listen_port: u16 }

    pub fn model() -> Machine {
        Machine { services: Services {
            example: Example { enable: true, listen_port: 8080 },
        } }
    }
}
let module = NixosModule::empty().add(config::model()).add(packages);
// packages is an independent rooted contribution in nix-interop.rs.
```

Structs directly inside the module get the existing conversion derives. Each
explicit root gets IntoConfig; multiple roots are supported. Normal Rust privacy
still applies, so the factory above constructs its private fields inside the module.
The generated paths include `services.example.listenPort`. No prefix strings are
needed, and each add still contributes an independent NixOS module.

The attribute is available as `rusnix_ir::config`; these examples alias the crate
as `rusnix` for `#[rusnix::config]`. It supports inline modules only. It does not
load `mod config;`, inspect imports, recurse into child modules/function-local
items, or expand types produced by other macros. Such types use the existing
fine-grained derives or explicit trait implementations. Local structs follow the
same named-record/single-field-newtype limits as those derives. A helper type
that needs no conversion can live outside the boundary.

Reusable/library types continue to use `#[derive(IntoRusnixValue)]` or
`#[derive(IntoConfig)]`, including types defined across files/crates. The
[typed-submodule example](typed-submodule.rs) imports its derived Endpoint into
a local config module. Explicit conversions using the canonical IntoConfig or
IntoRusnixValue names are respected. Detection is syntactic: aliased traits or
macro-generated implementations use the fine-grained path outside this boundary.
Local enums retain explicit exhaustive IntoRusnixValue mappings;
neither authoring style invents an automatic enum encoding. InputRef remains
lookup context; use field skip if retaining it as local state.

The types are defined by the user, without a global Rust model of NixOS.
Another root can contain only `environment.systemPackages`. Every add preserves
an independent module contribution: NixOS decides merge conflicts, list merging
and priorities. Flattening records inside ONE contribution is different and
still rejects conflicting leaf paths during IR validation.

IntoConfig derive also implements IntoRusnixValue. Reusable values such as
Endpoint and Port use IntoRusnixValue without acquiring global placement;
Transport uses an explicit conversion for its structural alternatives.
Single-field newtypes are transparent. Rust field names use `snake_case`; Rusnix
lowers them to Nix-style `lowerCamelCase` by default. Structs can select
`#[rusnix(rename_all = "PascalCase")]` for external schemas such as systemd.
Each nested struct chooses its own convention; a parent's setting is not inherited.
Explicit field renames are reserved for exceptions and override the struct's
convention. Raw identifiers such as `r#type` become `type`.

| Attribute | Meaning | Used by |
| --- | --- | --- |
| Struct `rename_all = "PascalCase"` | Mechanical naming for external schemas | systemd ServiceConfig |
| Struct `rename_all = "lowerCamelCase"` | Explicitly select the default convention | naming tests |
| Field `rename = "..."` | One exact attribute name, overriding the convention | Identity's `name` maps to `owner` |
| `flatten` | Place a record's fields into the containing record | typed-values and the test-only layered-validation collision |
| `skip` | Exclude local state from conversion and trait bounds | the tested generic derive fixture |

Leaf enums explicitly implement IntoRusnixValue with exhaustive matching;
structural alternatives match into small derived Plain/Tls records. No enum
serialization convention, tagged union scheme or option-schema generation is
implied. Custom value mappings return opaque RusnixValue through `into_value`,
`leaf` or `record`, without raw semantic nodes or Nix AST. Existing Expr,
OptionRef, PackageRef and NixValue leaves retain their symbolic/opaque semantics.
Imports and overlays remain explicit NixosModule/Nixpkgs boundary operations.

Derived bindings carry the conversion/add caller location and stable path-specific
IDs. Captured expression/reference origins survive; primitive initializer spans
are not captured separately. The [derive tests](../crates/rusnix-nix/tests/derive.rs)
check structure, values, provenance, real package resolution, NixOS merges,
priorities and two-origin conflicts. The
[module tests](../crates/rusnix-nix/tests/config_module.rs) also cover external
reusable values, explicit/manual conversions and multiple roots. The older
[authoring tests](../crates/rusnix-nix/tests/authoring.rs) still verify handwritten
adapter compatibility. Only nix-interop uses Config::set, for its intentionally
labelled dynamic escape hatch; it remains available for unsupported/dynamic paths.

OptionRef declares a typed dependency resolved after NixOS merging, with no Rust
read operation. [Symbolic option](symbolic-option.rs) shows the dependency in generated Nix;
integration tests prove an ordinary Nix override changes dependent output without
rerunning conversion or lowering. Expected Rust
types constrain expression operations; NixOS remains authoritative for schemas.

All ten showcase files use the module boundary for their local tree. The
remaining explicit IntoRusnixValue derives are intentional: reusable domain
values outside that module (credentials, endpoint fields and account identities),
and function-local Plain/Tls records inside semantic enum conversions. The latter
keep each mapping self-contained; the module macro does not inspect function
bodies. No showcase type explicitly derives IntoConfig; local roots use the module macro.

## Enum option

The example's Rust `Mode` enum has Server and Client variants.
`accepts_connections(Mode)` accepts that type and handles both variants. The
ConnectionPolicy stores Mode without turning it into a string until lowering.
The local config module lowers Root and ConnectionPolicy automatically. Root's
`demo` field supplies placement; default naming maps `accepts_connections`
to `acceptsConnections` without an annotation.

Nix comparison: `types.enum [ "server" "client" ]` checks values during module
evaluation. Rust additionally constrains every typed caller and consumer.
`Mode::Proxy` fails with E0599; passing `"server"` to `accepts_connections` fails with
E0308. Both are tested in [invalid-enum.rs](../tests/ui/invalid-enum.rs) and
[enum-string.rs](../tests/ui/enum-string.rs). Nix still checks the serialized enum
and option schema; [its comparison](../tests/comparisons/enum-option.nix) also
has a tested rejection of `"proxy"`.

## Typed submodule

```rust
struct Endpoint { host: Hostname, port: Port }
let endpoint = Endpoint { host: Hostname("service.internal".into()), port: Port(443) };
```

The example constructs an Endpoint directly and places it in
`Root { demo: Demo { endpoint } }`. The config module gives Root and Demo their
conversions automatically. Endpoint, Hostname and Port retain explicit derives
outside the module, demonstrating reusable values imported through normal Rust.
This tree creates `demo.endpoint.host` and `demo.endpoint.port`; Endpoint can be
reused at another placement without a global lowering method.
Assigning UserId to `port` or UserName to `host`
is a compiler error: E0308, expected Port/Hostname, found UserId/UserName.
[Both](../tests/ui/wrong-domain-type.rs) [misuses](../tests/ui/wrong-host-type.rs)
are real fixtures. Nix's corresponding [submodule](../tests/comparisons/typed-submodule.nix)
validates field values too; Rust retains their semantic identities across
fields, functions and composition before evaluation.

## Invalid states

```rust
enum Transport {
    Plain,
    Tls { certificate: Certificate, private_key: PrivateKey },
}
```

Plain has no credential fields. Tls cannot be constructed without both fields.
The real [plain-with-certificate fixture](../tests/ui/invalid-state.rs) fails with
E0559. The [missing-key fixture](../tests/ui/missing-private-key.rs) produces:

```text
error[E0063]: missing field `private_key` in initializer of `Transport`
```

The [Nix comparison](../tests/comparisons/invalid-states.nix) uses independent
boolean/nullable options and an assertion requiring TLS to agree with both
credential presences. Tests prove NixOS rejects both plain-with-credentials and
TLS-without-key. Rust prevents constructing those combinations through this sum
type; Nix rejects equivalent independent-field combinations during evaluation.
Neither version proves that the referenced files exist or contain valid keys.
Transport's explicit exhaustive value mapping selects a derived Plain or Tls
record. The local config module automatically lowers Root and ServiceConfig;
Root's `demo` field and ServiceConfig's `transport` field supply placement;
no handwritten configuration bindings are involved.

## Function contracts

```rust
fn configure_service(endpoint: Endpoint, transport: Transport) -> ServiceConfig
```

The caller supplies semantic values and receives a reusable service model.
`Root { demo: service }` determines placement through the local config module. Passing `true` instead of Transport is a [tested E0308](../tests/ui/function-contract.rs).
The [Nix function comparison](../tests/comparisons/function-contracts.nix) accepts
`{ host, port, enableTLS ? true }` and has output module types. Nix can add runtime
argument checks; Rust checks this caller contract statically throughout the
program. Nix still checks final option values and external configuration.

## Exhaustive match

The same Mode drives two meaningful consumers: `firewall_policy` chooses inbound
ports and `service_policy` chooses whether to accept connections. Both explicitly
match Server and Client, without a wildcard.

The [model-evolution fixture](../tests/ui/non-exhaustive-match.rs) adds Peer and
produces **two** E0004 errors, one at each consumer's `match mode`:

```text
error[E0004]: non-exhaustive patterns: `Mode::Peer` not covered
```

The compiler identifies affected exhaustive consumers when the model changes.
Nix still validates the [policy output options](../tests/comparisons/exhaustive-match.nix);
it has no general static exhaustiveness checker for arbitrary functions. A Rust
wildcard would deliberately give up this maintenance property.

## Typed values

Listener contains a Port and Hostname; Identity contains a UserId and UserName.
The example deliberately uses 1000 for both numeric domains and `"admin"` for
both text domains. Those compatible representations remain different Rust types.
The [fixture](../tests/ui/wrong-domain-type.rs) checks both Endpoint field misuse
and `listen(UserId(1000))`:

```text
expected `Port`, found `UserId`
```

Listener and Identity derive value conversion; `flatten` on Listener's owner
places Identity's `userId` and `owner` fields beside `port` and `host`. The default
converts `user_id`; the explicit `name` → `owner` rename is semantic, not casing.
Root's demo field determines the configuration path. This is semantic identity,
not a range-checking comparison. The illustrative
UserId is bounded to u16, not a complete platform UID model. These examples use
transparent newtypes without constructors or local validation: they distinguish
meanings, but do not prove DNS, account, path or file validity. The [Nix comparison](../tests/comparisons/typed-values.nix)
continues checking its primitive option types.

## Layered validation

| Authority | Tested failure | Why this layer |
| --- | --- | --- |
| rustc | UserId supplied where Port is required, E0308 | Static semantic domain mismatch |
| Rusnix IR | Two derived records flatten to the same option path in one contribution | Requires inspecting the assembled IR |
| NixOS | `services.openssh.exampleUnsupported` does not exist | Upstream option declarations are authoritative |

The runnable file authors a valid upstream SSH contribution using a local config
module for SshContribution<T>, Services<T> and options, plus a reusable Port value,
then prints generated Nix.
[typed_examples.rs](../crates/rusnix-nix/tests/typed_examples.rs) assembles two port
records that collide inside one flattened contribution and verifies the IR error.
It separately uses Config::set to introduce an unsupported option and verifies
NixOS rejects it. Those deliberate invalid assemblies and diagnostic assertions
belong in tests. Independent add contributions retain normal NixOS merge semantics.
The [valid comparison](../tests/comparisons/layered-validation.nix) imports the
real upstream SSH module and is compared mechanically with the example output.

## Nix interop

```rust
let pkgs = Nixpkgs::new();
let hello: PackageRef = pkgs.get("hello");
let requests = pkgs.get("python312Packages.requests");
let ssh: ModuleRef = pkgs.module("services/networking/ssh/sshd.nix");
let uppercase: NixFunction = pkgs.function("toUpper");
```

The executable shows three authoring choices: its own Transport model, opaque
package/module/function/overlay/input handles, and a runtime-computed NixOS option
path through the labelled Config::set escape hatch. PackageContribution maps
`Vec<PackageRef>` structurally to `environment.systemPackages`; no package-specific
bindings are needed. One config module lowers the independent rooted
contributions automatically. The program prints generated Nix, without resolving packages
or performing NixOS evaluation.

`InputRef::local("example", "input.nix")` names a user-supplied local object; the
file is not needed for source generation. The integration tests instead supply
[tests/fixtures/nix-interop-input.nix](../tests/fixtures/nix-interop-input.nix),
which contains actual overlay, package and module outputs plus failure controls.
It is local test data, not a flake fetch.

[interop.rs](../crates/rusnix-nix/tests/interop.rs) reuses the example's package
contribution and combined module. Tests verify hello, nested requests, the
Nix overlay's hello alias, an external package/module, upstream SSH ports `[22]`,
and `toUpper("rusnix") = "RUSNIX"`. Test-only schema imports and priority selection
exclude unrelated full-system defaults; these are not normal authoring requirements.
The same suite retains missing-package/nested/function/module/input and boundary
provenance checks. [Category misuse](../tests/ui/module-as-package.rs) fails with
E0308; deliberately mislabelling an actual Nix module as a package is rejected by
NixOS's package type at evaluation.

Imported internals map to the Rust import boundary. Immediate lookup/call failures
retain their operation origin; a delayed child of a returned container maps to
explicit selection rather than the original call. Original traces remain available.
Tests use offline pinned nixpkgs in the isolated store, evaluate package metadata
without building, and retain the current fixed x86_64-linux root, handle-scoped
overlays and partial NixOS harness. InputRef is not a general flake resolver.

The example also calls `pkgs.package_function("writeTextFile")` with a structured
`NixValue::record`: literals and a real package handle share one opaque argument.
`function("toUpper")` refers to lib; `package_function` refers to the package set.
Chained `.call` operations support curried functions such as writeText.
[structured_interop.rs](../crates/rusnix-nix/tests/structured_interop.rs) verifies
writeText/writeTextFile/runCommand without building, nested lists/maps and native
references, and symbolic text following an ordinary Nix override on the same
artifact. It also checks call/operation provenance and unforced sibling values.
Function results remain NixValue; Rusnix does not infer their package category.

## Symbolic option

Concrete Rust values are computed before lowering. `OptionRef<T>` instead declares
a dependency on a final merged NixOS option; Rust cannot read its resolved value.
The component is defined locally in [symbolic-option.rs](symbolic-option.rs):

```rust
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

One config module supplies conversions for the root and nested structs. Default naming converts
`service_config` to `serviceConfig`; ServiceConfig uses struct-level PascalCase
for `exec_start` → `ExecStart`. No field renames are needed to place the expression
in `systemd.services.example.serviceConfig.ExecStart`:

```rust
// Inside the #[rusnix::config] module:
#[rusnix(rename_all = "PascalCase")]
struct ServiceConfig {
    exec_start: Expr<String>,
}
``` The generated dependency is
equivalent to:

```nix
{ config, ... }: {
  systemd.services.example.serviceConfig.ExecStart =
    "example --port=" + toString config.services.example.port;
}
```

The example prints the concrete Rust command and generated symbolic module.
[symbolic_options.rs](../crates/rusnix-nix/tests/symbolic_options.rs) reuses its
derived ExampleService and supplies the option declarations and ordinary Nix
contributors from [the Nix fixture](../tests/fixtures/symbolic-options.nix).
The integration tests verify that the same generated Rusnix artifact evaluates
with port `5432`, then produces `6432` after an ordinary Nix module overrides the
referenced option. Rust conversion and lowering are not rerun. A separate
`lib.mkForce` test resolves to `7432`; NixOS alone selects the final value.

The same tests retain laziness, operation/reference provenance and generated
source-map fallback checks. Nix already supports this fixed-point behavior;
Rusnix preserves it through an explicit typed dependency. `T` is the author's
expectation, not proof of the NixOS schema. The
[UI test](../tests/ui/symbolic-integer-as-boolean.rs) rejects integer expressions
as boolean assertion conditions. There is no whole-config read or dynamic
traversal. The harness declares types for example options while systemd is a
freeform placeholder; a separate upstream OpenSSH banner test checks a real
option type. No service is built or run.

## PostgreSQL compatibility rewrite

[postgresql.rs](postgresql.rs) replaces the configuration-generation side of the
679-line PostgreSQL module at `8b27c1239e5c421a2bbc2c65d52e4a6fbf2ff296` (about
210 implementation lines). It reuses upstream public option declarations and
migration imports; it does not recreate the NixOS option schema.

`Postgresql` supplies ordinary Rust inputs. `Database::Owned { name, clauses }`
creates the matching database and role, so their names cannot disagree.
`Clause::{Preserve, Enable, Disable}` distinguishes leaving an existing role
attribute alone from granting or revoking it. The new
[compile-fail fixture](../tests/ui/postgresql-owned-mismatch.rs) proves that the
owned form has no separate owner-name field. Unowned databases and additional
roles remain available, and PostgreSQL setting names remain open-ended.

```rust
let postgres = Postgresql {
    enable: true,
    databases: vec![Database::Owned {
        name: "app".into(),
        clauses: BTreeMap::from([("login".into(), Clause::Enable)]),
    }],
    ..Postgresql::default()
};
let module = implementation().add(postgres);
```

The two local roots automatically lower input and implementation trees.
The input adapter's dynamic record omits unset options and derives matching roles;
that is semantic conversion, not repetitive field lowering. The implementation
uses finite OptionRef dependencies for enabled state, package/JIT/extensions,
settings, files, databases/users, state version and numeric Unix identities.
`NixValue::function` callbacks give existing Nix library functions bounded work
on those final collections. Rust never reads final config or traverses a global
fixed point. `Nixpkgs::from_module()` uses NixOS's package set and overlays.
Package variants/withPackages, writeText, writeTextDir and runCommand remain opaque
Nix operations. Text templates contain shell/SQL/configuration data, not Nix syntax.

[The integration suite](../crates/rusnix-nix/tests/postgresql.rs) evaluates the full
pinned NixOS module set twice: once with upstream PostgreSQL, once replacing that
import slot with upstream declarations plus the generated Rust implementation.
It compares settings, authentication/ident files, generated-file derivation
recipes, service scripts and unit text, environment, hardening, directories,
Unix accounts, package identities and check recipes. Store paths and Nix string
contexts are compared exactly. Only the set-like pathsToLink list is sorted;
SQL, package, argument and executable-string ordering remain significant.

The matrix covers disabled/default/custom packages, JIT, extension functions and
list coercion, directories, all setting primitives and preload-list coercion,
authentication addition/replacement, ident maps, initdb/initial/recovery scripts,
databases/users/ownership, all seven clauses in all three states, priorities,
TCP/IP, hardening overrides, checks, state-version defaults, renamed/removed
options, invalid values, laziness and cross compilation. A legacy-metadata fixture
uses a real derivation with explicitly synthetic version metadata for the old
service-type/directory-permission branches; it does not claim to build old PostgreSQL.

[An ordinary Nix contributor](../tests/fixtures/postgresql-downstream.nix) changes
port to 6432, data directory, settings, authentication, ensured databases/users
and PostgreSQL package to version 15. The same generated artifact follows every
change without rerunning Rust lowering, and remains equivalent to upstream.
Conflict tests retain both Rust origins; a failing symbolic setting retains its
original Rust division operation. Foreign ownership assertions retain NixOS's
reason but currently lack a per-user Rust origin after assertion aggregation.

Evaluation constructs derivations but builds no files or packages and runs no
SQL/service/check commands. Thus this proves configuration-generation equivalence,
not database migration or runtime correctness. Upstream remains authoritative for
its schema: null is allowed for declared nullable settings, not arbitrary freeform
keys. Raw input strings retain upstream quoting behavior. This example must replace
upstream's implementation, not be imported alongside it; the schema-only adapter
and comparison machinery belong to tests. There is no PostgreSQL type in core.

## Verification and limits

```bash
cargo run --locked -p rusnix-nix --example invalid-states
cargo run --locked -p rusnix-nix --example nix-interop
cargo run --locked -p rusnix-nix --example symbolic-option
cargo test --workspace --locked
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
bash scripts/check-fixtures.sh
```

All ten examples compile and run without invoking Nix: they print generated
source. The fixture script runs every example and the legacy generic SSH compiler
example, integration checks and existing CLI fixtures. Only tests and CLI checks
perform evaluation, always through the isolated-store helper.

| Example | AUTHORING kept in source | VERIFICATION in tests |
| --- | --- | --- |
| enum-option | Mode, typed consumers, automatic local placement | typed_examples.rs, ui.rs |
| typed-submodule | Semantic fields and reusable Endpoint | typed_examples.rs, ui.rs |
| invalid-states | Transport alternatives and structural projection | typed_examples.rs, ui.rs |
| function-contracts | Typed caller contract | typed_examples.rs, ui.rs |
| exhaustive-match | Two exhaustive policy consumers | typed_examples.rs, ui.rs |
| typed-values | Nominal types and flattened structure | typed_examples.rs, ui.rs |
| layered-validation | Valid upstream contribution | typed_examples.rs; invalid assemblies and schema errors |
| nix-interop | Opaque handles, composition, explicit escape | interop.rs; real lookups, boundaries and assertions |
| symbolic-option | Typed dependency and automatic local unit structure | symbolic_options.rs; artifact reuse, override, priority and provenance |
| postgresql | Domain provisioning, symbolic derivation and real nixpkgs builders | postgresql.rs; full NixOS equivalence, overrides and failures |

DOCUMENTATION stays here: comparisons, expected outcomes, test links and limitations.
Tests retain reviewable generated Nix/results under `target/typed-examples/` and
`target/symbolic-options/`; compiler JSON goes under `target/typed-examples/ui/`.

Seven important Nix comparisons live in `tests/comparisons/` as executable test
data. Backend tests import the actual single-file Rust examples and compare
their evaluated outputs with these modules, including native enum/assertion
rejections. Twenty-six UI fixtures back the documented invalid Rust cases through the
code/span/type-label checker. They import actual example-local types and functions
or deliberately evolve a test-local enum; the moved fixture-only SSH helper's
contract is also checked. Two fixtures verify two independent errors each.
The symbolic-reference fixture rejects an integer expression as a boolean
assertion condition with E0308. A derived PackageRef field rejects ModuleRef
with E0308. Derive errors reject container prefixes, automatic enum conversion
and conflicting attributes. Additional cases reject invalid/duplicate rename_all,
rename_all on transparent newtypes, and misplaced field rename_all. The twenty-six
fixtures check twenty-eight errors using codes where available, useful primary spans
and relevant messages/type labels.
The module API also rejects missing/duplicate roots, enum/tuple roots, macro
arguments and category mistakes. Core has three compile-fail doctests for generic
Expr, opaque handle categories and unmapped module enums, plus a runnable
module-authoring doctest. No complete compiler-wording snapshots are required.

The guarantees apply to the typed API. Explicit primitive extraction, opaque
`as_value()`, generic Config and raw IR can bypass them. Later independent NixOS
overrides can change a serialized model; final Nix checks remain valuable. Rust
types do not prove termination, absence of panics, operational service correctness
or arbitrary opaque Nix schemas. The strongest demonstrations are the Transport
sum type, model-evolution matches, and their coexistence with opaque Nix objects.

The derive deliberately supports named structs and single-field value newtypes,
not arbitrary Serde features. Flattening a non-record is an IR validation error.
Option/null and dynamic maps are supported explicitly at the NixValue boundary;
this does not add general Option/map derive semantics. Foreign Nix may still
force scalar heads while processing freeform option namespaces; list elements
remain deferred. Derive
adds no Nix forcing, and the existing selective-evaluation tests remain intact.
