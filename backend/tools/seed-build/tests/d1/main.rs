//! D1-level tests (`make test-d1`): real SQL on isolated local D1 state through wrangler, using
//! the `d1-local` harness crate. Every test is `#[ignore]`d, so plain `cargo test` needs no Node.js.

mod harness;
mod schema;
