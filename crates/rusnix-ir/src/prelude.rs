//! Common interfaces for typed package and configuration authoring.
//!
//! Dynamic operations live in [`crate::interop::raw`]; IR construction lives in
//! the compiler backend. Neither is included in this prelude.
pub use crate::interop::{
    FinalAttrs, IntoNixExpression, NixAttrs, NixCallable, NixExpression, NixLibrary, NixList,
    NixNullable, NixOverridable, NixPath, Nixpkgs, Overridable, Package, PackageFunction,
    PackageRef, Platform, Stdenv, ToNixText,
};
pub use crate::nixos::{NixosModule, OptionRef};
pub use crate::{
    Config, ConfigValue, Expr, IntoConfig, IntoRusnixValue, ValidationError, args, config,
    nix_record, nix_text, options,
};
