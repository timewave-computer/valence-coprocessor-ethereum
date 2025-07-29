use std::env;

pub fn version() -> String {
    env::var("CARGO_PKG_VERSION").unwrap()
}
