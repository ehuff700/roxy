#[macro_use]
extern crate log;

#[macro_use]
extern crate flutter_rust_bridge;

pub mod api;
// flutter_rust_bridge:ignore
mod core;
mod frb_generated;

pub use api::utils::error::BackendError;
