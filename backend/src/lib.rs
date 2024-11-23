#[macro_use]
extern crate log;

#[macro_use]
extern crate flutter_rust_bridge;

pub mod api;
// flutter_rust_bridge:ignore
mod core;
mod frb_generated;

pub use api::utils::error::BackendError;

#[frb(ignore)]
pub mod tls_extras {
    pub const CERT: &[u8] = include_bytes!("core/certs/cert.pem");
    pub const KEY: &[u8] = include_bytes!("core/certs/private.key");
}
