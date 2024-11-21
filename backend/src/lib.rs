#[macro_use]
extern crate log;

pub mod api;
mod frb_generated;
pub use api::utils::error::BackendError;
