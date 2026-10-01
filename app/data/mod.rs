//! Durable records and file storage. Active-plan lifecycle is intentionally deferred.
use crate::{customization::Package, model::*};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs::{self, File, OpenOptions},
    io::{BufReader, BufWriter, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static COUNTER: AtomicU64 = AtomicU64::new(0);
pub type ToolResult = Result<Value, Value>;
pub(crate) fn error(code: &str, message: impl ToString) -> Value {
    json!({"code":code,"message":message.to_string()})
}
pub(crate) fn id() -> String {
    format!(
        "a{:x}-{:x}-{:x}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}
#[derive(Serialize, Deserialize)]
pub(crate) struct Scenario {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub customization_package: Option<Package>,
    pub revision: u64,
    pub problem: Problem,
    pub parent: Option<String>,
}
#[derive(Serialize, Deserialize)]
pub(crate) struct SavedSchedule {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub customization_package: Option<Package>,
    pub scenario_id: String,
    pub revision: u64,
    pub problem: Problem,
    pub schedule: Schedule,
}
#[derive(Serialize, Deserialize)]
pub(crate) struct Import {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub customization_package: Option<Package>,
    pub problem: Problem,
    pub expected_tasks: usize,
    pub chunks: std::collections::BTreeMap<String, String>,
    pub finalized: Option<String>,
}

pub(crate) struct FileStore<'a> {
    pub root: &'a Path,
    pub workspace: &'a Path,
}
impl FileStore<'_> {
    pub fn path(&self, key: &str) -> Result<PathBuf, Value> {
        if key.is_empty()
            || key.len() > 120
            || !key
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err(error("ID", "Invalid artifact ID"));
        }
        Ok(self.root.join(format!("{key}.json")))
    }
    pub fn read<T: serde::de::DeserializeOwned>(&self, key: &str) -> Result<T, Value> {
        let bytes = fs::read(self.path(key)?).map_err(|e| error("NOT_FOUND", e))?;
        serde_json::from_slice(&bytes).map_err(|e| error("ARTIFACT", e))
    }
    pub fn write<T: Serialize>(&self, key: &str, value: &T) -> Result<(), Value> {
        let target = self.path(key)?;
        let temp = self.root.join(format!("{}.tmp", id()));
        let mut f =
            BufWriter::with_capacity(65536, File::create(&temp).map_err(|e| error("STORE", e))?);
        serde_json::to_writer(&mut f, value).map_err(|e| error("STORE", e))?;
        f.flush().map_err(|e| error("STORE", e))?;
        f.get_ref().sync_all().map_err(|e| error("STORE", e))?;
        fs::rename(temp, target).map_err(|e| error("STORE", e))
    }
    pub fn input_file(&self, path: &str) -> Result<Value, Value> {
        let path = self
            .workspace
            .join(path)
            .canonicalize()
            .map_err(|e| error("FILE", e))?;
        if !path.starts_with(self.workspace) {
            return Err(error(
                "PATH",
                "Input must be inside the configured workspace",
            ));
        }
        let f = File::open(path).map_err(|e| error("FILE", e))?;
        if f.metadata().map_err(|e| error("FILE", e))?.len() > 256 * 1024 * 1024 {
            return Err(error("SIZE", "Use chunk import above 256 MiB"));
        }
        serde_json::from_reader(BufReader::with_capacity(65536, f)).map_err(|e| error("JSON", e))
    }
    pub fn lock(&self) -> Result<File, Value> {
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(self.root.join("store.lock"))
            .map_err(|e| error("STORE", e))?;
        fs2::FileExt::lock_exclusive(&lock).map_err(|e| error("STORE", e))?;
        Ok(lock)
    }
}
