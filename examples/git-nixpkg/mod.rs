//! Reusable Rust-authored Git package and its normal package-set wiring.
mod inputs;

mod lowering;

pub mod model;

use rusnix_ir::interop::NixValue;

pub use lowering::factory;

pub fn arguments() -> NixValue {
    inputs::arguments(model::Git::defaults())
}
