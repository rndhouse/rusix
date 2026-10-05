# Pinned nixpkgs evaluation sources

`nixpkgs/` contains unmodified upstream files from NixOS/nixpkgs release 24.11,
commit `8b27c1239e5c421a2bbc2c65d52e4a6fbf2ff296` (the dereferenced release tag).
The original upstream license is in `nixpkgs/COPYING`.

Source archive:
`https://codeload.github.com/NixOS/nixpkgs/tar.gz/8b27c1239e5c421a2bbc2c65d52e4a6fbf2ff296`

Archive SHA-256:
`b4e794d1b935c1960e95526db7ed394f886064b4d974684dbc4a59d4012e1028`

`nixpkgs/PIN.json` records the revision, archive digest, and individual SHA-256
digests for all 232 upstream files (about 1 MB of content). Rusnix verifies every
listed file once per process before copying it into each disposable session.
The manifest itself is reviewed repository data, not an upstream signature.

Selection: all of upstream `lib/`, root `.version`, `.version-suffix`, `COPYING`,
and these four modules:

- `nixos/modules/services/networking/ssh/sshd.nix`
- `nixos/modules/misc/assertions.nix`
- `nixos/modules/misc/label.nix`
- `nixos/modules/system/activation/top-level.nix`

The last file is a reference for assertion enforcement, never an evaluated
import. The original minimal tests do not load a package set or system builder.
Preparation downloaded the archive using HTTPS and extracted these files as
ordinary filesystem data; it ran no Nix fetching command and used no Nix store.
The original module tests use this checked-in subset offline. To reproduce
the subset, extract exactly the selection above from that archive and compare
every file with `PIN.json`; upstream files must remain byte-for-byte unchanged.


The interoperability experiment also vendors the **same full source archive** as
`nixpkgs-source.tar.gz` (47,237,858 bytes compressed (about 45 MiB), 178 MB of file content). It was
copied from the already cached archive above; this phase made no network fetch.
The backend checks its SHA-256 before unpacking it into an ordinary temporary
source directory. Sessions share that source tree while alive, but never share
stores. Each session links it as `nixpkgs-full`; the last owner removes the tree.
This retains upstream package discovery and evaluation without maintaining a
hand-selected dependency closure or Rust package metadata. Both source copies
have the same pin. No archive-refresh or automatic downloading step is provided.

Interop imports upstream `default.nix` with explicit system/config/overlays and
uses the actual `config/system-path.nix` option declarations. Metadata evaluation
can create `.drv` records and source paths, exclusively in the disposable store.
It does not build outputs. Import-from-derivation is disabled, substitutions and
builders are empty, and all evaluation commands use the private offline helper.
The minimal evaluator does not import the system-building top-level module.
