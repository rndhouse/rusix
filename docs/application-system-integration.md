# Sharing application types with system configuration

## Overview

- The application and its installation code use the same Rust types.
- Rusix turns installation declarations into NixOS settings for packages, startup
  commands and configuration files.
- NixOS installs those settings as part of the system. The application reads its
  configuration using the same types that were used to create it.

## Installing application configuration

[AirlockOS](https://github.com/airlock-org/airlock) sends screenshots from a
desktop virtual machine (VM) to a personal VM. Its Rust type
[`CaptureSenderConfig`][contracts] identifies the destination. The type derives
Serde's `Serialize` and `Deserialize` traits, so installation code can write it
as JSON and the application can read it back.

The [service's installation code][capture-authoring] uses Rusix to declare where
that JSON belongs. This simplified example shows just the configuration file;
Airlock also declares the application package and its startup entry:

```rust
let config = CaptureSenderConfig {
    target: "usr-home".parse()?,
};

let application = rusix::installation::Application::new("airlock-sys-gui-service")
    .etc_text(
        "airlock/capture-sender.json",
        serde_json::to_string(&config)?,
    );
```

Here, `.etc_text()` declares a file under `/etc`. When this application is
[composed into a module][composition], Rusix generates the NixOS settings for
that file. NixOS combines them with the rest of the machine configuration, and
deployment makes `/etc/airlock/capture-sender.json` available to the application.
Constructing the Rust value alone does not write the file.

## Reading the installed configuration

The [running service][runtime] reads the installed file into the same type.
With error handling shortened, that looks like:

```rust
let bytes = std::fs::read("/etc/airlock/capture-sender.json")?;
let config: CaptureSenderConfig = serde_json::from_slice(&bytes)?;
```

The service uses `config.target` as its destination. Serde handles the JSON
conversion; Rusix supplies the NixOS installation definitions that put the file
where the service expects it.

Airlock shares its [command enum][commands] in the same way: installation code
uses `DesktopCommand::ScreenCapture` to construct a login command, and the
application parses that command using the same enum. The application crate can
therefore define both its behavior and its installation. The machine author
chooses where to run it and supplies the surrounding NixOS configuration and
communication permissions.

The snippets omit imports and surrounding functions. Source links are pinned to
Airlock revision `5ef2f0010e964a68beeb2f884a8b0c171ed40495`.

[contracts]: https://github.com/airlock-org/airlock/blob/5ef2f0010e964a68beeb2f884a8b0c171ed40495/crates/airlock-common/machine/src/services.rs
[capture-authoring]: https://github.com/airlock-org/airlock/blob/5ef2f0010e964a68beeb2f884a8b0c171ed40495/crates/airlock-sys-gui/service/src/authoring/capture.rs
[composition]: https://github.com/airlock-org/airlock/blob/5ef2f0010e964a68beeb2f884a8b0c171ed40495/crates/airlock-config/src/components/screen-capture.rs
[runtime]: https://github.com/airlock-org/airlock/blob/5ef2f0010e964a68beeb2f884a8b0c171ed40495/crates/airlock-sys-gui/service/src/captures.rs
[commands]: https://github.com/airlock-org/airlock/blob/5ef2f0010e964a68beeb2f884a8b0c171ed40495/crates/airlock-sys-gui/service/src/command.rs
