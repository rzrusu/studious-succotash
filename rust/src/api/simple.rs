use once_cell::sync::Lazy;
use rusqlite::Connection;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

static APP_DOCUMENTS_DIR: Lazy<RwLock<Option<PathBuf>>> = Lazy::new(|| RwLock::new(None));
static SAVE_MANAGER: Lazy<RwLock<Option<SaveManager>>> = Lazy::new(|| RwLock::new(None));

const APP_NAME: &str = "my_app";
const SAVES_SUBDIRECTORY: &str = "saves";
const METADATA_DB_FILE: &str = "metadata.db";

#[allow(dead_code)]
#[derive(Debug)]
pub struct SaveManager {
    base_path: PathBuf,
    app_dir: PathBuf,
    saves_dir: PathBuf,
    metadata_db_path: PathBuf,
}

impl SaveManager {
    fn initialize(base_path: impl Into<PathBuf>) -> Result<Self, SaveManagerError> {
        let base_path = base_path.into();
        let app_dir = base_path.join(APP_NAME);
        let saves_dir = app_dir.join(SAVES_SUBDIRECTORY);

        fs::create_dir_all(&saves_dir)?;
        let metadata_db_path = saves_dir.join(METADATA_DB_FILE);
        initialize_metadata_db(&metadata_db_path)?;

        Ok(Self {
            base_path,
            app_dir,
            saves_dir,
            metadata_db_path,
        })
    }
}

#[derive(Debug)]
enum SaveManagerError {
    Io(std::io::Error),
    Database(rusqlite::Error),
}

impl fmt::Display for SaveManagerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SaveManagerError::Io(err) => write!(f, "I/O error: {err}"),
            SaveManagerError::Database(err) => write!(f, "database error: {err}"),
        }
    }
}

impl std::error::Error for SaveManagerError {}

impl From<std::io::Error> for SaveManagerError {
    fn from(value: std::io::Error) -> Self {
        SaveManagerError::Io(value)
    }
}

impl From<rusqlite::Error> for SaveManagerError {
    fn from(value: rusqlite::Error) -> Self {
        SaveManagerError::Database(value)
    }
}

fn initialize_metadata_db(path: &Path) -> Result<(), SaveManagerError> {
    let connection = Connection::open(path)?;
    connection.execute(
        "CREATE TABLE IF NOT EXISTS save_slots (
            id UUID PRIMARY KEY,
            name TEXT NOT NULL,
            last_played TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            file_path TEXT NOT NULL
        )",
        [],
    )?;
    Ok(())
}

#[flutter_rust_bridge::frb(sync)]
pub fn init_system(base_path: String) -> Result<(), String> {
    let manager =
        SaveManager::initialize(base_path).map_err(|err| format!("init_system failed: {err}"))?;
    let mut storage = SAVE_MANAGER
        .write()
        .map_err(|_| "SaveManager lock poisoned".to_string())?;
    *storage = Some(manager);
    Ok(())
}

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
