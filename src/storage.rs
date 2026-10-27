//! The data file: `kanvibe.json` in the data folder, loaded on startup and saved after every change.
use std::fs;
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::domain::{Board, Data, Id};

const FILE: &str = "kanvibe.json";
const TMP: &str = "kanvibe.json.tmp";
const VERSION: u32 = 1;

/// App settings saved beside the Boards. Missing fields take defaults.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_board: Option<Id>,
}

#[derive(Serialize)]
struct FileOut<'a> {
    version: u32,
    next_id: Id,
    settings: &'a Settings,
    boards: &'a [Board],
}

#[derive(Deserialize)]
struct FileIn {
    next_id: Id,
    #[serde(default)]
    settings: Settings,
    #[serde(default)]
    boards: Vec<Board>,
}

/// The data folder: `--data-dir <path>` if given, else `%LOCALAPPDATA%\Kanvibe`.
pub fn data_dir(args: impl IntoIterator<Item = String>) -> Option<PathBuf> {
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        if arg == "--data-dir" {
            return args.next().map(PathBuf::from);
        }
    }
    Some(dirs::data_local_dir()?.join("Kanvibe"))
}

/// Writes the data file atomically: `kanvibe.json.tmp`, `sync_all`, rename over `kanvibe.json`.
pub fn save(dir: &Path, data: &Data, settings: &Settings) -> io::Result<()> {
    let file = FileOut {
        version: VERSION,
        next_id: data.next_id,
        settings,
        boards: &data.boards,
    };
    let tmp = dir.join(TMP);
    let mut out = fs::File::create(&tmp)?;
    out.write_all(&serde_json::to_vec_pretty(&file)?)?;
    out.sync_all()?;
    drop(out);
    rename_with_retry(&tmp, &dir.join(FILE))
}

/// Antivirus, indexers and sync tools briefly hold files open; retry the rename up to 10 times
/// with 50–200 ms backoff on sharing or access errors.
fn rename_with_retry(from: &Path, to: &Path) -> io::Result<()> {
    for attempt in 1.. {
        match fs::rename(from, to) {
            Err(e) if attempt <= 10 && is_sharing_or_access(&e) => {
                std::thread::sleep(Duration::from_millis((50 * attempt).min(200)));
            }
            result => return result,
        }
    }
    unreachable!()
}

fn is_sharing_or_access(e: &io::Error) -> bool {
    // 32 / 33: Windows ERROR_SHARING_VIOLATION / ERROR_LOCK_VIOLATION, which std leaves uncategorised.
    e.kind() == ErrorKind::PermissionDenied
        || cfg!(windows) && matches!(e.raw_os_error(), Some(32 | 33))
}

/// Reads the data file, first deleting any leftover `.tmp` from an interrupted save.
/// `Ok(None)` when there is no data file: first run.
/// A content error comes back as `ErrorKind::InvalidData`.
pub fn load(dir: &Path) -> io::Result<Option<(Data, Settings)>> {
    match fs::remove_file(dir.join(TMP)) {
        Err(e) if e.kind() != ErrorKind::NotFound => return Err(e),
        _ => {}
    }
    let bytes = match fs::read(dir.join(FILE)) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes);
    let file: FileIn = serde_json::from_slice(bytes)?;
    let data = Data {
        next_id: file.next_id,
        boards: file.boards,
    };
    Ok(Some((data, file.settings)))
}
