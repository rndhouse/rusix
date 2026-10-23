# Sharing application types with system configuration

## TL;DR

An application and its NixOS installation code can import the same Rust types.
Installation code uses them to generate startup commands and configuration files;
the running application parses those inputs back into the same types. Traits and
conversion functions provide that translation, while Rusix generates the NixOS
definitions that install them.

## Airlock screen-capture integration

[AirlockOS](https://github.com/airlock-org/airlock) uses this approach to deliver
screenshots from a desktop virtual machine (VM) to a personal VM. The application
defines its capture commands and configuration types; the machine author decides
which VMs should communicate.

The following excerpts omit surrounding definitions.

## Reusing application commands in installation code

The desktop service's command-line parser uses a Rust enum,
[`DesktopCommand`][commands]. In the same crate, the
[installation code][capture-authoring] uses that enum to construct a command
that retries pending screenshot deliveries at login:

```rust
self.desktop_command(&DesktopCommand::ScreenCapture {
    config: Some(CONFIG_PATH.into()),
    action: CaptureAction::Retry { target: None },
})
```

Rusix generates the NixOS configuration that installs this command in a desktop
startup entry. Rust checks the command construction against the same enum used
by the running application. The application and its installation code can evolve
together using shared definitions.

## Deriving configuration from application connections

The [machine configuration][machine] connects the desktop VM to a personal VM:

```rust
crate::components::screen_capture::connect(&gui, &home);
```

This Airlock function accepts typed VM handles. From the connection, Airlock
[derives][connection] the sender's destination, the receiver's permitted callers,
and the Qubes rules authorizing communication between those VMs. The running
services consume the resulting configuration through [shared Rust types][contracts].

The application supplies its commands, package recipe and installation helpers.
The machine author chooses where it runs and which connections to permit. Rusix
expresses that integration as NixOS definitions, which compose with existing Nix
modules and the caller's package set. NixOS evaluates and merges the definitions;
deployment installs the result, and Qubes enforces the communication permissions.

Source links are pinned to Airlock revision
`f15b61d7afec96c60cd320d810c0181c7dd0e7c1`.

[commands]: https://github.com/airlock-org/airlock/blob/f15b61d7afec96c60cd320d810c0181c7dd0e7c1/crates/airlock-sys-gui/service/src/command.rs
[capture-authoring]: https://github.com/airlock-org/airlock/blob/f15b61d7afec96c60cd320d810c0181c7dd0e7c1/crates/airlock-sys-gui/service/src/authoring/capture.rs
[machine]: https://github.com/airlock-org/airlock/blob/f15b61d7afec96c60cd320d810c0181c7dd0e7c1/crates/airlock-config/src/profiles/usb_lab.rs
[connection]: https://github.com/airlock-org/airlock/blob/f15b61d7afec96c60cd320d810c0181c7dd0e7c1/crates/airlock-config/src/components/screen-capture.rs
[contracts]: https://github.com/airlock-org/airlock/blob/f15b61d7afec96c60cd320d810c0181c7dd0e7c1/crates/airlock-common/machine/src/services.rs
