//! The data file: `kanvibe.json` in the data folder, loaded on startup and saved after every change.
use std::fs;
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use crate::domain::{Board, Data, Id};

const FILE: &str = "kanvibe.json";
const TMP: &str = "kanvibe.json.tmp";
const LOCK: &str = "kanvibe.lock";
const BACKUPS: [&str; 2] = ["kanvibe.json.bak1", "kanvibe.json.bak2"];
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

/// The single-copy lock: creates the data folder and locks `kanvibe.lock` in it.
/// `Ok(None)` when another copy holds it. Keep the file for the life of the process;
/// the OS releases the lock on exit or crash, and the file itself is never deleted.
pub fn lock(dir: &Path) -> io::Result<Option<fs::File>> {
    fs::create_dir_all(dir)?;
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join(LOCK))?;
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(fs::TryLockError::WouldBlock) => Ok(None),
        Err(fs::TryLockError::Error(e)) => Err(e),
    }
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

/// "Save a copy as…": the data file's contents written to `path`, which the user picked.
pub fn save_copy(path: &Path, data: &Data, settings: &Settings) -> io::Result<()> {
    let file = FileOut {
        version: VERSION,
        next_id: data.next_id,
        settings,
        boards: &data.boards,
    };
    fs::write(path, serde_json::to_vec_pretty(&file)?)
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

/// Why the data file didn't load. Each has its own startup screen.
#[derive(Debug)]
pub enum LoadError {
    /// `version` is higher than this build reads. Touch nothing; Quit only.
    Newer,
    /// The file couldn't be read (locked, permission), even after retrying for 1 s.
    Io(io::Error),
    /// The unreadable flow: the file doesn't parse or has the wrong shape, or it is missing
    /// while a backup exists. The file is never overwritten.
    Unreadable(String),
}

/// The data file's path in the data folder.
pub fn file_path(dir: &Path) -> PathBuf {
    dir.join(FILE)
}

/// Reads the data file, first deleting any leftover `.tmp` from an interrupted save.
/// `Ok(None)` when there is no data file and no backup: first run.
/// I/O errors are retried 5 times over 1 s.
pub fn load(dir: &Path) -> Result<Option<(Data, Settings)>, LoadError> {
    let mut attempt = 0;
    let bytes = loop {
        match read(dir) {
            Err(_) if attempt < 5 => {
                attempt += 1;
                std::thread::sleep(Duration::from_millis(200));
            }
            result => break result.map_err(LoadError::Io)?,
        }
    };
    match bytes {
        Some(bytes) => parse(&bytes).map(Some),
        None if BACKUPS.iter().any(|b| dir.join(b).exists()) => Err(LoadError::Unreadable(
            "The data file is missing, but a backup exists.".into(),
        )),
        None => Ok(None),
    }
}

/// The newest backup that loads, and when it was written. `.bak1` is newer than `.bak2`.
pub fn backup(dir: &Path) -> Option<(PathBuf, SystemTime)> {
    BACKUPS.iter().map(|b| dir.join(b)).find_map(|path| {
        parse(&fs::read(&path).ok()?).ok()?;
        let modified = fs::metadata(&path).ok()?.modified().ok()?;
        Some((path, modified))
    })
}

/// "Restore from backup": moves the bad data file aside, then copies the newest readable
/// backup over it.
pub fn restore(dir: &Path) -> io::Result<()> {
    let (backup, _) = backup(dir).ok_or_else(|| io::Error::other("No readable backup"))?;
    move_aside(dir)?;
    let tmp = dir.join(TMP);
    fs::copy(backup, &tmp)?;
    rename_with_retry(&tmp, &dir.join(FILE))
}

/// "Start empty": moves the bad data file aside, then writes an empty one.
pub fn start_empty(dir: &Path) -> io::Result<()> {
    move_aside(dir)?;
    save(dir, &Data::default(), &Settings::default())
}

/// Renames the data file, if there is one, to `kanvibe.json.corrupt-<UTC timestamp>`.
/// Old `.corrupt` files are never deleted or replaced.
fn move_aside(dir: &Path) -> io::Result<()> {
    let file = dir.join(FILE);
    if !file.try_exists()? {
        return Ok(());
    }
    let stamp = jiff::Timestamp::now().strftime("%Y%m%dT%H%M%SZ");
    let base = format!("{FILE}.corrupt-{stamp}");
    let mut target = dir.join(&base);
    for n in 2.. {
        if !target.try_exists()? {
            break;
        }
        target = dir.join(format!("{base}-{n}"));
    }
    rename_with_retry(&file, &target)
}

/// The data file's bytes; `None` when it doesn't exist.
fn read(dir: &Path) -> io::Result<Option<Vec<u8>>> {
    match fs::remove_file(dir.join(TMP)) {
        Err(e) if e.kind() != ErrorKind::NotFound => return Err(e),
        _ => {}
    }
    match fs::read(dir.join(FILE)) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

fn parse(bytes: &[u8]) -> Result<(Data, Settings), LoadError> {
    #[derive(Deserialize)]
    struct Version {
        version: u32,
    }
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let unreadable = |e: serde_json::Error| LoadError::Unreadable(e.to_string());
    let Version { version } = serde_json::from_slice(bytes).map_err(unreadable)?;
    if version > VERSION {
        return Err(LoadError::Newer);
    }
    let file: FileIn = serde_json::from_slice(bytes).map_err(unreadable)?;
    let data = Data {
        next_id: file.next_id,
        boards: file.boards,
    };
    Ok((data, file.settings))
}
