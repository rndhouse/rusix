# Defining system integration in an application crate

## Overview

- An application crate can provide Rust functions that describe how to install
  and start the application, and how it connects to other software.
- Machine configuration calls those functions with its own packages and settings.
- Rusix turns the returned declarations into NixOS settings that combine with
  existing Nix modules.

## The application's installation API

[AirlockOS](https://github.com/airlock-org/airlock) has a desktop service that
sends screenshots to another virtual machine (VM). The `airlock-sys-gui-service`
crate contains both the running service and its [installation API][authoring].
A caller requests capture support through:

```rust
let application = service.capture_sender(&config);
```

This is an Airlock function that returns a Rusix `Application`. Its
[implementation][capture-authoring] declares:

- The service package and its executable.
- The configuration file naming the destination VM.
- A login command that retries pending deliveries.
- Changes to GNOME that call the service when a screenshot or recording is ready.

The installation code uses the application's own `DesktopCommand` enum to
construct the login command and its `CaptureSenderConfig` type for the settings.
These are the [same commands][commands] and [configuration types][contracts] used
by the running service. The crate therefore keeps the application and its
installation definitions together, with Rust checking their use of shared types.

## Using the API in machine configuration

The machine supplies its package set and source paths when creating the service
object. In the [machine's NixOS module][composition], `inputs.pkgs` is the package
set supplied by NixOS:

```rust
let service = airlock_sys_gui_service::authoring::deployment(
    &inputs.pkgs,
    &machine_source::service_sources(&inputs),
);
```

The machine then passes the application's declarations to Rusix. This shortened
snippet uses `?` in a function returning `Result`:

```rust
let behavior = installation::compose(
    &scope,
    installation::CollisionChecks::Enabled,
    [service.capture_sender(&config)],
)?;
```

Here, `installation` is Rusix's installation API. It checks the declared resources
for conflicts and produces settings to include in the NixOS module. The machine
combines those settings with its other modules and supplies the permissions for
communication between VMs.

The application defines its installation through a reusable Rust API. The
machine chooses where and how to use it. Rust constructs the definitions; NixOS
evaluates and merges them, and deployment installs the result.

The snippets omit surrounding code. Source links are pinned to Airlock revision
`5ef2f0010e964a68beeb2f884a8b0c171ed40495`.

[authoring]: https://github.com/airlock-org/airlock/blob/5ef2f0010e964a68beeb2f884a8b0c171ed40495/crates/airlock-sys-gui/service/src/authoring.rs
[capture-authoring]: https://github.com/airlock-org/airlock/blob/5ef2f0010e964a68beeb2f884a8b0c171ed40495/crates/airlock-sys-gui/service/src/authoring/capture.rs
[composition]: https://github.com/airlock-org/airlock/blob/5ef2f0010e964a68beeb2f884a8b0c171ed40495/crates/airlock-config/src/components/screen-capture.rs
[commands]: https://github.com/airlock-org/airlock/blob/5ef2f0010e964a68beeb2f884a8b0c171ed40495/crates/airlock-sys-gui/service/src/command.rs
[contracts]: https://github.com/airlock-org/airlock/blob/5ef2f0010e964a68beeb2f884a8b0c171ed40495/crates/airlock-common/machine/src/services.rs
