# Sharing application types with system configuration

An application's Rust codebase can also describe how to install and connect it
to the rest of the system. Rusix lets that configuration reuse the application's
types and functions, then participate in NixOS module composition.

[AirlockOS](https://github.com/airlock-org/airlock) uses this approach for its
desktop service. Airlock separates applications into virtual machines (VMs);
one service sends screenshots and recordings from the desktop VM to a personal
VM. Its Rust definitions connect application behavior, desktop integration and
the permissions needed to communicate between VMs.

The snippets below are excerpts, with surrounding definitions omitted. Source
links are pinned to Airlock revision `f15b61d7afec96c60cd320d810c0181c7dd0e7c1`.

## Command parsing and invocation generation

The GUI service defines a Rust enum, [`DesktopCommand`][commands], describing
operations it accepts. The running application's command-line parser uses that
enum and dispatches capture operations like this:

```rust
Action::Desktop(DesktopCommand::ScreenCapture { config, action }) => {
    captures::execute(action, config.as_deref())
}
```

In the same crate, the [installation code][capture-authoring] constructs a
command to retry pending captures when the user logs in:

```rust
self.desktop_command(&DesktopCommand::ScreenCapture {
    config: Some(CONFIG_PATH.into()),
    action: CaptureAction::Retry { target: None },
})
```

The helper combines the packaged executable's path with the command's arguments
and escapes them for a desktop entry. Rusix emits the NixOS configuration that
installs that entry and the service's configuration file.

The runtime parser and installation code use the same command enum. Rust checks
its variants and fields, while a [focused test][command-test] checks that
constructed arguments round-trip through the production parser. The crate's
separate `runtime` and `authoring` [features][features] let configuration authors use this interface
without compiling the service's runtime I/O and X11 dependencies.

## Deriving service configuration and Qubes permissions

The machine author decides which VMs should communicate. Airlock's
[machine configuration][machine] connects a desktop VM to a personal VM:

```rust
crate::components::screen_capture::connect(&gui, &home);
```

This is an Airlock API built on Rusix. Its [typed signature][connection] accepts
a GUI VM as the source and a personal VM as the destination. When the machine
description is finalized, Airlock derives the following from the connection:

- The sender's configuration identifying the destination VM.
- The receiver's configuration listing its permitted callers.
- Qubes permission rules allowing the capture operations between those VMs.

The [shared configuration types][contracts] are serialized into installed files
and read by the running services. The sender checks that an explicitly requested
destination agrees with its installed configuration; the receiver checks the
authenticated caller against its configured sources. Qubes also enforces the
communication permissions.

## Explicit installation contracts

The service also describes its executable and capture files with Rusix's
[`installation::Application`][installation] API. The machine composes the
participating applications for each target VM with an explicit policy:

```rust
let behavior = installation::compose(
    &scope,
    installation::CollisionChecks::Enabled,
    [service.capture_sender(&config)],
)
.unwrap_or_else(|error| panic!("{error}"));
```

With checks enabled, Rusix rejects conflicting declared executable names,
subcommand paths and `/etc` file destinations during Rust configuration
construction, before Nix evaluation. For example, `screen-capture` belongs
beneath `airlock-sys-gui-service`; another executable can use the same subcommand
name. Applications in separate VMs are composed separately. The package list and
text-file definitions are emitted from the same declarations that were checked.

The service collects shared desktop command paths and aliases from its CLI
schema, alongside the names of its other root commands. GNOME callbacks use
those same command-name definitions. Runtime-only nested commands are outside
this registration.

This API is opt-in. `CollisionChecks::Disabled` preserves duplicate contributions
for normal NixOS merging, which may still reject incompatible values; malformed
names and paths remain errors. Each composition checks only its declared resources,
not other compositions, undeclared package exports, opaque modules or downstream
overrides. These are ordinary Rust checks, not Verus proofs or `rustc` type errors.

## NixOS module composition

The application owns its commands, package recipe and installation helpers.
The machine author owns VM placement and the connections that grant access.
Airlock's [module composition][connection] supplies the caller's package set to
the service and combines its installation settings with other NixOS modules.
It also adds NixOS assertions to detect changes to the installed capture
configuration that would disagree with the declared connection.

The Rust functions construct deferred Nix expressions. NixOS evaluates the
expressions and merges their definitions with the other modules; deployment
installs the resulting packages and configuration files. Qubes checks the
installed permission rules when services communicate between VMs.

[commands]: https://github.com/airlock-org/airlock/blob/f15b61d7afec96c60cd320d810c0181c7dd0e7c1/crates/airlock-sys-gui/service/src/command.rs
[capture-authoring]: https://github.com/airlock-org/airlock/blob/f15b61d7afec96c60cd320d810c0181c7dd0e7c1/crates/airlock-sys-gui/service/src/authoring/capture.rs
[command-test]: https://github.com/airlock-org/airlock/blob/f15b61d7afec96c60cd320d810c0181c7dd0e7c1/crates/airlock-sys-gui/service/src/main.rs#L153
[features]: https://github.com/airlock-org/airlock/blob/f15b61d7afec96c60cd320d810c0181c7dd0e7c1/crates/airlock-sys-gui/service/Cargo.toml
[machine]: https://github.com/airlock-org/airlock/blob/f15b61d7afec96c60cd320d810c0181c7dd0e7c1/crates/airlock-config/src/profiles/usb_lab.rs
[connection]: https://github.com/airlock-org/airlock/blob/f15b61d7afec96c60cd320d810c0181c7dd0e7c1/crates/airlock-config/src/components/screen-capture.rs
[contracts]: https://github.com/airlock-org/airlock/blob/f15b61d7afec96c60cd320d810c0181c7dd0e7c1/crates/airlock-common/machine/src/services.rs

[installation]: ../crates/rusix/src/installation.rs
