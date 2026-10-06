//! Emit the complete family and the default package without building or fetching.
mod inputs;

mod lowering;

mod model;

mod scripts;

use rusnix_ir::{
    Config,
    interop::{NixValue, Nixpkgs},
};

fn main() {
    let factory = lowering::factory(model::Release::Preview);
    let openssl =
        Nixpkgs::new().call_package(&factory, NixValue::record([] as [(&str, NixValue); 0]));
    let generated = rusnix_nix::compile(
        &Config::new()
            .set("factory", factory)
            .set("family", lowering::family_factory())
            .set("openssl", openssl),
    )
    .unwrap();
    println!("{}", generated.source);
}
