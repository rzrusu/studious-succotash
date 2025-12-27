use chrono::Utc;
use once_cell::sync::Lazy;
use rusqlite::{params, Connection};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use uuid::Uuid;

static APP_DOCUMENTS_DIR: Lazy<RwLock<Option<PathBuf>>> = Lazy::new(|| RwLock::new(None));
static SAVE_MANAGER: Lazy<RwLock<Option<SaveManager>>> = Lazy::new(|| RwLock::new(None));

const APP_NAME: &str = "my_app";
const SAVES_SUBDIRECTORY: &str = "saves";
const METADATA_DB_FILE: &str = "metadata.db";

#[derive(Clone, Debug)]
pub struct SaveSlotMetadata {
    pub id: String,
    pub name: String,
    pub last_played: String,
    pub file_path: String,
}

impl SaveSlotMetadata {
    fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            name: row.get(1)?,
            last_played: row.get(2)?,
            file_path: row.get(3)?,
        })
    }
}

#[derive(Debug)]
pub struct SaveManager {
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
            saves_dir,
            metadata_db_path,
        })
    }

    fn metadata_connection(&self) -> Result<Connection, SaveManagerError> {
        open_configured_connection(&self.metadata_db_path)
    }

    fn create_slot(&self, display_name: String) -> Result<SaveSlotMetadata, SaveManagerError> {
        let slot_id = Uuid::new_v4().to_string();
        let slot_file_path = self.saves_dir.join(format!("{slot_id}.sav"));
        let slot_file_path_string = slot_file_path.to_string_lossy().into_owned();
        let last_played = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(); // matches SQLite DEFAULT CURRENT_TIMESTAMP format

        let connection = self.metadata_connection()?;
        connection.execute(
            "INSERT INTO save_slots (id, name, last_played, file_path)
             VALUES (?1, ?2, ?3, ?4)",
            params![slot_id, display_name, last_played, slot_file_path_string],
        )?;

        Ok(SaveSlotMetadata {
            id: slot_id,
            name: display_name,
            last_played,
            file_path: slot_file_path_string,
        })
    }

    fn all_slots(&self) -> Result<Vec<SaveSlotMetadata>, SaveManagerError> {
        let connection = self.metadata_connection()?;
        let mut statement = connection.prepare(
            "SELECT id, name, last_played, file_path
             FROM save_slots
             ORDER BY last_played DESC",
        )?;
        let rows = statement.query_map([], SaveSlotMetadata::from_row)?;

        let mut slots = Vec::new();
        for row in rows {
            slots.push(row?);
        }
        Ok(slots)
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

fn open_configured_connection(path: &Path) -> Result<Connection, SaveManagerError> {
    let connection = Connection::open(path)?;
    connection.pragma_update(None, "journal_mode", "WAL")?;
    Ok(connection)
}

fn initialize_metadata_db(path: &Path) -> Result<(), SaveManagerError> {
    let connection = open_configured_connection(path)?;
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

fn with_save_manager<T>(
    action: impl FnOnce(&SaveManager) -> Result<T, SaveManagerError>,
) -> Result<T, String> {
    let storage = SAVE_MANAGER
        .read()
        .map_err(|_| "SaveManager lock poisoned".to_string())?;
    let manager = storage
        .as_ref()
        .ok_or_else(|| "SaveManager has not been initialized".to_string())?;
    action(manager).map_err(|err| err.to_string())
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
pub fn create_new_slot(display_name: String) -> Result<SaveSlotMetadata, String> {
    with_save_manager(move |manager| manager.create_slot(display_name))
}

#[flutter_rust_bridge::frb(sync)]
pub fn get_all_slots() -> Result<Vec<SaveSlotMetadata>, String> {
    with_save_manager(|manager| manager.all_slots())
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
