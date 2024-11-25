#[flutter_rust_bridge::frb(init)]
pub fn init_app() {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    // Default utilities - feel free to customize
    flutter_rust_bridge::setup_default_user_utils();
}
