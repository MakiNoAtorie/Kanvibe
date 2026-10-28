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
/// The data file and Board export version. 2 added `Column.collapsed`, `Card.due` and
/// `Card.files`, all defaulted when absent, so a v1 file loads with no migration.
pub(crate) const VERSION: u32 = 2;
/// `.bak1` older than this is refreshed before any save.
const STALE: Duration = Duration::from_secs(24 * 3600);

/// App settings saved beside the Boards. Missing fields take defaults. In the file (see
/// `SettingsFile`) the theme is `"theme": "light" | "dark"` and the window one object.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(from = "SettingsFile", into = "SettingsFile")]
pub struct Settings {
    pub last_board: Option<Id>,
    pub sidebar_collapsed: bool,
    /// The dark theme; light by default.
    pub dark: bool,
    /// The window's restored (neither minimised nor maximised) rectangle, in logical pixels.
    pub window: Option<Geometry>,
    pub maximized: bool,
}

/// `Settings` as written: `"window": { x, y, w, h, "maximized" }`.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct SettingsFile {
    #[serde(skip_serializing_if = "Option::is_none")]
    last_board: Option<Id>,
    sidebar_collapsed: bool,
    /// "dark", else light.
    theme: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    window: Option<WindowFile>,
}

#[derive(Default, Serialize, Deserialize)]
struct WindowFile {
    /// Missing while no restored rectangle is known.
    #[serde(flatten)]
    rect: Option<Rect>,
    #[serde(default)]
    maximized: bool,
}

#[derive(Serialize, Deserialize)]
struct Rect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl From<SettingsFile> for Settings {
    fn from(f: SettingsFile) -> Self {
        let window = f.window.unwrap_or_default();
        Settings {
            last_board: f.last_board,
            sidebar_collapsed: f.sidebar_collapsed,
            dark: f.theme == "dark",
            window: window.rect.map(|r| Geometry {
                x: r.x,
                y: r.y,
                width: r.w,
                height: r.h,
            }),
            maximized: window.maximized,
        }
    }
}

impl From<Settings> for SettingsFile {
    fn from(s: Settings) -> Self {
        let rect = s.window.map(|g| Rect {
            x: g.x,
            y: g.y,
            w: g.width,
            h: g.height,
        });
        SettingsFile {
            last_board: s.last_board,
            sidebar_collapsed: s.sidebar_collapsed,
            theme: if s.dark { "dark" } else { "light" }.into(),
            window: (rect.is_some() || s.maximized).then_some(WindowFile {
                rect,
                maximized: s.maximized,
            }),
        }
    }
}

/// A window or monitor rectangle in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Geometry {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
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

/// Where the session is in backup rotation. Held by whoever saves; see `rotate`.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum Rotation {
    /// Nothing rotated yet this session.
    #[default]
    Due,
    /// Rotated (or found nothing to back up) this session; only the 24 h rule applies.
    Done,
    /// The file didn't load cleanly, or was restored or started empty: rotating would push a
    /// good backup out, so nothing rotates this session.
    Off,
}

/// Rotates the two backups beside the data file, if due. Call before every save, with
/// `data_changed` when the save changes Board data. Due on the session's first data-changing
/// save, and before any save once `.bak1` is older than 24 h. Steps: data file to `.bak1.tmp`, delete `.bak2` and move `.bak1` to `.bak2` (only when `.bak1` reads), `.bak1.tmp` to
/// `.bak1`. A failed rotation stays due, so the next save retries it.
pub fn rotate(dir: &Path, rotation: &mut Rotation, data_changed: bool) -> io::Result<()> {
    let [bak1, bak2] = BACKUPS.map(|b| dir.join(b));
    let stale = || {
        let modified = fs::metadata(&bak1).and_then(|m| m.modified()).ok();
        modified.is_some_and(|m| m.elapsed().unwrap_or_default() > STALE)
    };
    if *rotation == Rotation::Off || !(*rotation == Rotation::Due && data_changed || stale()) {
        return Ok(());
    }
    let tmp = dir.join(format!("{}.tmp", BACKUPS[0]));
    let bytes = match fs::read(dir.join(FILE)) {
        Err(e) if e.kind() == ErrorKind::NotFound => {
            *rotation = Rotation::Done; // first run: nothing to back up
            return Ok(());
        }
        result => result?,
    };
    if fs::read(&bak1).is_ok_and(|b| b == bytes) {
        *rotation = Rotation::Done; // `.bak1` already holds it; rotating would only lose `.bak2`
        return Ok(());
    }
    let mut out = fs::File::create(&tmp)?;
    out.write_all(&bytes)?;
    out.sync_all()?;
    drop(out);
    // An unreadable `.bak1` is only replaced: shifting it would push a readable `.bak2` out.
    if fs::read(&bak1).is_ok_and(|b| parse(&b).is_ok()) {
        remove_if_exists(&bak2)?;
        rename_with_retry(&bak1, &bak2)?;
    }
    rename_with_retry(&tmp, &bak1)?;
    *rotation = Rotation::Done;
    Ok(())
}

fn remove_if_exists(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(e) if e.kind() != ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
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

/// The data file as loaded: repaired (see `repair::repair`) and migrated to this version.
#[derive(Debug, Default)]
pub struct Loaded {
    pub data: Data,
    pub settings: Settings,
    /// Something was repaired: the file didn't load cleanly, so backups must not rotate.
    pub repaired: bool,
}

/// `open` without the repaired flag.
pub fn load(dir: &Path) -> Result<Option<(Data, Settings)>, LoadError> {
    Ok(open(dir)?.map(|l| (l.data, l.settings)))
}

/// Reads the data file. `Ok(None)` when there is no data file and no backup: first run.
/// I/O errors are retried 5 times over 1 s. A leftover `.tmp` from an interrupted save is
/// deleted only once the file loads (or on first run); a newer or bad file touches nothing.
pub fn open(dir: &Path) -> Result<Option<Loaded>, LoadError> {
    let loaded = load_file(dir)?;
    remove_if_exists(&dir.join(TMP)).map_err(LoadError::Io)?;
    Ok(loaded)
}

fn load_file(dir: &Path) -> Result<Option<Loaded>, LoadError> {
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
        Some(bytes) => {
            let (loaded, version) = parse(&bytes)?;
            if version < VERSION {
                keep_original(dir, version, &bytes).map_err(LoadError::Io)?;
            }
            Ok(Some(loaded))
        }
        None if BACKUPS.iter().any(|b| dir.join(b).exists()) => Err(LoadError::Unreadable(
            "The data file is missing, but a backup exists.".into(),
        )),
        None => Ok(None),
    }
}

/// Copies a pre-migration file to `kanvibe.v<old>.json`, which is never replaced or deleted.
/// Done at load, so it is always in place before the first save after a migration.
fn keep_original(dir: &Path, version: u32, bytes: &[u8]) -> io::Result<()> {
    let path = dir.join(format!("kanvibe.v{version}.json"));
    let mut out = match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Err(e) if e.kind() == ErrorKind::AlreadyExists => return Ok(()),
        result => result?,
    };
    out.write_all(bytes)?;
    out.sync_all()
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
    match fs::read(dir.join(FILE)) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// The file, repaired and migrated, and the `version` it was written with.
fn parse(bytes: &[u8]) -> Result<(Loaded, u32), LoadError> {
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
    // Migrations from older versions go here, oldest first, rewriting the JSON before `FileIn`
    // reads it. None yet: versions 0 and 1 read as version 2.
    let file: FileIn = serde_json::from_slice(bytes).map_err(unreadable)?;
    // A `last_board` that isn't an active Board needs no repair: `App::open` falls back.
    let (data, repaired) = crate::repair::repair(Data {
        next_id: file.next_id,
        boards: file.boards,
    });
    let loaded = Loaded {
        data,
        settings: file.settings,
        repaired,
    };
    Ok((loaded, version))
}
