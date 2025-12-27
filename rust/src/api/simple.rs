use once_cell::sync::Lazy;
use std::path::PathBuf;
use std::sync::RwLock;

static APP_DOCUMENTS_DIR: Lazy<RwLock<Option<PathBuf>>> = Lazy::new(|| RwLock::new(None));

#[flutter_rust_bridge::frb(sync)]
pub fn set_application_documents_directory(dir: String) {
    let mut storage = APP_DOCUMENTS_DIR
        .write()
        .expect("application documents directory lock poisoned");
    *storage = Some(PathBuf::from(dir));
}

#[flutter_rust_bridge::frb(sync)]
pub fn debug_application_documents_directory() -> Option<String> {
    let storage = APP_DOCUMENTS_DIR
        .read()
        .expect("application documents directory lock poisoned");
    storage
        .as_ref()
        .map(|path| path.to_string_lossy().into_owned())
}

#[flutter_rust_bridge::frb(init)]
pub fn init_app() {
    // Default utilities - feel free to customize
    flutter_rust_bridge::setup_default_user_utils();
}
