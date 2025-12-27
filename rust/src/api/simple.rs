use chrono::Utc;
use once_cell::sync::Lazy;
use rusqlite::{params, Connection};
use rusqlite_migration::{Migrations, M};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, RwLock};
use uuid::Uuid;

static APP_DOCUMENTS_DIR: Lazy<RwLock<Option<PathBuf>>> = Lazy::new(|| RwLock::new(None));
static SAVE_MANAGER: Lazy<Mutex<Option<SaveManager>>> = Lazy::new(|| Mutex::new(None));

const APP_NAME: &str = "my_app";
const SAVES_SUBDIRECTORY: &str = "saves";
const METADATA_DB_FILE: &str = "metadata.db";

static SLOT_DB_MIGRATIONS: Lazy<Migrations<'static>> = Lazy::new(|| {
    Migrations::new(vec![M::up(
        "
        CREATE TABLE IF NOT EXISTS player_stats (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            health INTEGER NOT NULL,
            experience INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS inventory (
            position INTEGER PRIMARY KEY,
            item TEXT NOT NULL
        );
        ",
    )])
});

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

#[derive(Clone, Debug)]
pub struct PlayerData {
    pub health: i32,
    pub experience: i32,
    pub inventory: Vec<String>,
}

pub struct SaveManager {
    saves_dir: PathBuf,
    metadata_db_path: PathBuf,
    active_connection: Option<Connection>,
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
            active_connection: None,
        })
    }

    fn metadata_connection(&self) -> Result<Connection, SaveManagerError> {
        open_configured_connection(&self.metadata_db_path)
    }

    fn create_slot(&self, display_name: String) -> Result<SaveSlotMetadata, SaveManagerError> {
        let slot_id = Uuid::new_v4().to_string();
        let slot_file_path = self.saves_dir.join(format!("{slot_id}.db"));
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

    fn close_active_connection(&mut self) {
        if self.active_connection.is_some() {
            self.active_connection = None;
        }
    }

    fn load_slot(&mut self, slot_id: String) -> Result<(), SaveManagerError> {
        self.close_active_connection();
        let metadata_connection = self.metadata_connection()?;
        let slot_lookup_id = slot_id.clone();
        let slot_path_string: String = match metadata_connection.query_row(
            "SELECT file_path FROM save_slots WHERE id = ?1",
            params![&slot_lookup_id],
            |row| row.get(0),
        ) {
            Ok(path) => path,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                return Err(SaveManagerError::SlotNotFound(slot_lookup_id))
            }
            Err(err) => return Err(err.into()),
        };

        let slot_path = PathBuf::from(&slot_path_string);
        let mut slot_connection = open_configured_connection(&slot_path)?;
        SLOT_DB_MIGRATIONS.to_latest(&mut slot_connection)?;

        metadata_connection.execute(
            "UPDATE save_slots SET last_played = CURRENT_TIMESTAMP WHERE id = ?1",
            params![&slot_id],
        )?;

        self.active_connection = Some(slot_connection);
        Ok(())
    }

    fn save_player_data(&mut self, data: PlayerData) -> Result<(), SaveManagerError> {
        let connection = self
            .active_connection
            .as_mut()
            .ok_or(SaveManagerError::NoActiveSlot)?;

        let transaction = connection.transaction()?;
        transaction.execute(
            "INSERT INTO player_stats (id, health, experience)
             VALUES (1, ?1, ?2)
             ON CONFLICT(id) DO UPDATE SET
                health = excluded.health,
                experience = excluded.experience",
            params![data.health, data.experience],
        )?;

        transaction.execute("DELETE FROM inventory", [])?;
        for (position, item) in data.inventory.iter().enumerate() {
            transaction.execute(
                "INSERT INTO inventory (position, item) VALUES (?1, ?2)",
                params![position as i64, item],
            )?;
        }

        transaction.commit()?;
        Ok(())
    }
}

#[derive(Debug)]
enum SaveManagerError {
    Io(std::io::Error),
    Database(rusqlite::Error),
    SlotNotFound(String),
    NoActiveSlot,
    Migration(rusqlite_migration::Error),
}

impl fmt::Display for SaveManagerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SaveManagerError::Io(err) => write!(f, "I/O error: {err}"),
            SaveManagerError::Database(err) => write!(f, "database error: {err}"),
            SaveManagerError::SlotNotFound(id) => {
                write!(f, "save slot with id '{id}' does not exist")
            }
            SaveManagerError::NoActiveSlot => write!(f, "no save slot is currently loaded"),
            SaveManagerError::Migration(err) => write!(f, "migration error: {err}"),
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

impl From<rusqlite_migration::Error> for SaveManagerError {
    fn from(value: rusqlite_migration::Error) -> Self {
        SaveManagerError::Migration(value)
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
        .lock()
        .map_err(|_| "SaveManager lock poisoned".to_string())?;
    let manager = storage
        .as_ref()
        .ok_or_else(|| "SaveManager has not been initialized".to_string())?;
    action(manager).map_err(|err| err.to_string())
}

fn with_save_manager_mut<T>(
    action: impl FnOnce(&mut SaveManager) -> Result<T, SaveManagerError>,
) -> Result<T, String> {
    let mut storage = SAVE_MANAGER
        .lock()
        .map_err(|_| "SaveManager lock poisoned".to_string())?;
    let manager = storage
        .as_mut()
        .ok_or_else(|| "SaveManager has not been initialized".to_string())?;
    action(manager).map_err(|err| err.to_string())
}

#[flutter_rust_bridge::frb(sync)]
pub fn init_system(base_path: String) -> Result<(), String> {
    let manager =
        SaveManager::initialize(base_path).map_err(|err| format!("init_system failed: {err}"))?;
    let mut storage = SAVE_MANAGER
        .lock()
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
pub fn load_slot(slot_id: String) -> Result<(), String> {
    with_save_manager_mut(move |manager| manager.load_slot(slot_id))
}

#[flutter_rust_bridge::frb(sync)]
pub fn save_player_data(data: PlayerData) -> Result<(), String> {
    with_save_manager_mut(move |manager| manager.save_player_data(data))
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
