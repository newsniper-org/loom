//! RQ6 §6 M1 — ProductCard Island alone, real per-Island code splitting
//! (Task #27; see build.rs and `loom-compiler`'s
//! `codegen.rs::generate_single_island_client` doc comment for why this
//! crate exists separately from `crates/loom-widget-embed`). Deliberately
//! NOT wrapped in an extra `pub mod generated { }` the way
//! `loom-widget-embed` is — this crate compiles to exactly ONE Island, so
//! its generated module (`product_card_generated`) sits directly at the
//! crate root.

include!(concat!(env!("OUT_DIR"), "/generated.rs"));
