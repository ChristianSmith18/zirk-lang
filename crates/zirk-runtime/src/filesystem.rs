//! Synchronous, bounded filesystem host operations for the compiler-owned
//! `std.fs` and `std.path` declarations. Every effect is rooted at an opened
//! capability directory, uses a canonical exact-resource permission, and
//! validates an opened file handle before reading or changing its contents.

use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
#[cfg(test)]
use std::io::Seek;
use std::io::{self, Read, Write};
use std::path::{Component, Path as HostPath, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::UNIX_EPOCH;

use cap_fs_ext::DirExt as _;
use cap_std::ambient_authority;
use cap_std::fs::{Dir, OpenOptions, ReadDir};
use notify::Watcher as _;
use zirk_runtime_core::permission_grant::FilesystemScopeExtent;

pub const MAX_WHOLE_FILE_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_CHUNK_BYTES: u64 = 4 * 1024 * 1024;
pub const MAX_LINE_BYTES: usize = 1024 * 1024;
pub const MAX_OPEN_FILES: usize = 256;
pub const MAX_OPEN_DIRECTORIES: usize = 128;
pub const MAX_TEMPORARY_RESOURCES: usize = 128;
pub const MAX_OPEN_LOCKS: usize = 128;
pub const MAX_OPEN_WATCHERS: usize = 32;
pub const MAX_DIRECTORY_DEPTH: u32 = 64;
pub const MAX_DIRECTORY_ENTRIES: u64 = 1_000_000;
pub const MAX_DIRECTORY_BYTES: u64 = 1_u64 << 40;
pub const MAX_WATCHER_QUEUE_CAPACITY: u32 = 256;
pub const MAX_WATCHED_DIRECTORIES: u64 = 1_024;
const MAX_WATCH_PATHS_PER_EVENT: usize = 128;
const MAX_WATCH_EVENT_PATH_BYTES: usize = 4_096;

const FLAG_READ: u32 = 1;
const FLAG_WRITE: u32 = 2;
const FLAG_APPEND: u32 = 4;
const FLAG_CREATE: u32 = 8;
const FLAG_CREATE_NEW: u32 = 16;
const FLAG_TRUNCATE: u32 = 32;

// Numeric operation ids are mirrored in zirk-codegen-llvm's closed binding
// table. Keeping one C ABI entry point makes the source boundary narrow while
// every operation remains a compiler-verified enum case.
pub const PATH_JOIN: i32 = 1;
pub const PATH_COMPONENTS: i32 = 2;
pub const PATH_PARENT: i32 = 3;
pub const PATH_NAME: i32 = 4;
pub const PATH_NORMALIZE: i32 = 9;
pub const PATH_ABSOLUTE: i32 = 10;
pub const PATH_CANONICALIZE: i32 = 11;

pub const FILE_READ_BYTES: i32 = 100;
pub const FILE_READ_TEXT: i32 = 101;
pub const FILE_WRITE_BYTES: i32 = 102;
pub const FILE_WRITE_TEXT: i32 = 103;
pub const FILE_OPEN: i32 = 104;
pub const FILE_CLOSE: i32 = 105;
pub const FILE_IS_CLOSED: i32 = 106;
pub const FILE_READ_HANDLE: i32 = 107;
pub const FILE_READ_EXACT: i32 = 108;
pub const FILE_READ_LINE: i32 = 109;
pub const FILE_APPEND_BYTES: i32 = 110;
pub const FILE_APPEND_TEXT: i32 = 111;
pub const FILE_APPEND_HANDLE: i32 = 112;
pub const FILE_METADATA_HANDLE: i32 = 113;
pub const FILE_EXISTS: i32 = 114;
pub const FILE_WRITE_BYTES_ATOMIC: i32 = 115;
pub const FILE_WRITE_TEXT_ATOMIC: i32 = 116;
pub const FILE_METADATA_PATH: i32 = 117;

pub const DIRECTORY_CREATE: i32 = 200;
pub const DIRECTORY_CREATE_ALL: i32 = 201;
pub const DIRECTORY_ENTRIES_OPEN: i32 = 202;
pub const DIRECTORY_ENTRIES_NEXT: i32 = 203;
pub const DIRECTORY_ENTRIES_CLOSE: i32 = 204;
pub const DIRECTORY_WALK_OPEN: i32 = 205;
pub const DIRECTORY_WALK_NEXT: i32 = 206;
pub const DIRECTORY_WALK_CLOSE: i32 = 207;
pub const DIRECTORY_IS_EMPTY: i32 = 208;
pub const DIRECTORY_SIZE: i32 = 209;
pub const DIRECTORY_COPY: i32 = 210;
pub const DIRECTORY_MOVE: i32 = 211;
pub const DIRECTORY_REMOVE: i32 = 212;
pub const DIRECTORY_REMOVE_ALL: i32 = 213;
pub const DIRECTORY_WATCH: i32 = 214;
pub const WATCHER_NEXT: i32 = 215;
pub const WATCHER_NEXT_ASYNC: i32 = 216;
pub const WATCHER_CLOSE: i32 = 217;

pub const LINK_IS_LINK: i32 = 220;
pub const LINK_READ_TARGET: i32 = 221;
pub const LINK_CREATE: i32 = 222;
pub const FILE_SET_READONLY: i32 = 230;
pub const TEMP_CREATE_FILE: i32 = 240;
pub const TEMP_CREATE_DIRECTORY: i32 = 241;
pub const TEMP_PATH: i32 = 242;
pub const TEMP_CLEANUP: i32 = 243;
pub const TEMP_RELEASE: i32 = 244;
pub const FILE_LOCK: i32 = 250;
pub const FILE_UNLOCK: i32 = 251;
pub const FILE_LOCK_ASYNC: i32 = 252;

pub const FILE_READ_BYTES_ASYNC: i32 = 300;
pub const FILE_READ_TEXT_ASYNC: i32 = 301;
pub const FILE_WRITE_BYTES_ASYNC: i32 = 302;
pub const FILE_WRITE_TEXT_ASYNC: i32 = 303;
pub const FILE_OPEN_ASYNC: i32 = 304;
pub const FILE_READ_HANDLE_ASYNC: i32 = 305;
pub const FILE_WRITE_HANDLE_ASYNC: i32 = 306;
pub const FILE_READ_EXACT_ASYNC: i32 = 307;
pub const FILE_APPEND_BYTES_ASYNC: i32 = 308;
pub const FILE_APPEND_TEXT_ASYNC: i32 = 309;
pub const FILE_APPEND_HANDLE_ASYNC: i32 = 310;
pub const DIRECTORY_ENTRIES_OPEN_ASYNC: i32 = 320;
pub const DIRECTORY_ENTRIES_NEXT_ASYNC: i32 = 321;
pub const DIRECTORY_WALK_OPEN_ASYNC: i32 = 322;
pub const DIRECTORY_WALK_NEXT_ASYNC: i32 = 323;

#[repr(C)]
pub struct FilesystemOut {
    /// `0 = value`, `1 = error`, `2 = normal EOF`.
    pub kind: i32,
    /// Discriminant from FileError or PathError, depending on the operation.
    pub error: i32,
    /// Scalar result: handles, booleans, or metadata flags.
    pub scalar: u64,
    /// Managed Zirk String, Array, or List result; null when unused.
    pub payload: *mut c_void,
}

impl FilesystemOut {
    fn value() -> Self {
        Self {
            kind: 0,
            error: 0,
            scalar: 0,
            payload: std::ptr::null_mut(),
        }
    }

    fn error(error: FsError, path_error: bool) -> Self {
        Self {
            kind: 1,
            error: if path_error {
                error.path_discriminant()
            } else {
                error.file_discriminant()
            },
            scalar: 0,
            payload: std::ptr::null_mut(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FsError {
    InvalidPath,
    InvalidOptions,
    NotFound,
    PermissionDenied,
    AlreadyExists,
    WrongType,
    Closed,
    Decoding,
    Limit,
    EndOfStream,
    Unsupported,
    Io,
    WouldBlock,
    CrossDevice,
    NotEmpty,
    Cancelled,
}

impl FsError {
    // FileError declaration order in stdlib/std/fs/fs.zrk.
    fn file_discriminant(self) -> i32 {
        match self {
            Self::InvalidPath => 0,
            Self::InvalidOptions => 1,
            Self::NotFound => 2,
            Self::PermissionDenied => 3,
            Self::AlreadyExists => 4,
            Self::WrongType => 5,
            Self::Closed => 6,
            Self::Decoding => 7,
            Self::Limit => 8,
            Self::EndOfStream => 9,
            Self::Unsupported => 10,
            Self::Io => 11,
            Self::WouldBlock => 12,
            Self::CrossDevice => 13,
            Self::NotEmpty => 14,
            Self::Cancelled => 15,
        }
    }

    // PathError declaration order in stdlib/std/path/path.zrk.
    fn path_discriminant(self) -> i32 {
        match self {
            Self::InvalidPath => 0,
            Self::PermissionDenied => 1,
            Self::NotFound => 2,
            Self::Unsupported => 3,
            _ => 4,
        }
    }
}

fn map_io(error: io::Error) -> FsError {
    #[cfg(unix)]
    if error.raw_os_error() == Some(libc::EXDEV) {
        return FsError::CrossDevice;
    }
    #[cfg(windows)]
    if matches!(error.raw_os_error(), Some(17)) {
        return FsError::CrossDevice;
    }
    #[cfg(unix)]
    if matches!(error.raw_os_error(), Some(39 | 66)) {
        return FsError::NotEmpty;
    }
    #[cfg(windows)]
    if error.raw_os_error() == Some(145) {
        return FsError::NotEmpty;
    }
    match error.kind() {
        io::ErrorKind::NotFound => FsError::NotFound,
        io::ErrorKind::PermissionDenied => FsError::PermissionDenied,
        io::ErrorKind::AlreadyExists => FsError::AlreadyExists,
        io::ErrorKind::InvalidInput | io::ErrorKind::InvalidData => FsError::InvalidPath,
        io::ErrorKind::IsADirectory | io::ErrorKind::NotADirectory => FsError::WrongType,
        io::ErrorKind::WouldBlock => FsError::WouldBlock,
        io::ErrorKind::Unsupported => FsError::Unsupported,
        _ => FsError::Io,
    }
}

struct OpenFile {
    file: cap_std::fs::File,
    path: String,
    read: bool,
    write: bool,
    append: bool,
    io_lock: Arc<Mutex<()>>,
}

enum AsyncFileValue {
    Bytes(Vec<u8>),
    Text(String),
}

static FILES: OnceLock<Mutex<HashMap<u64, OpenFile>>> = OnceLock::new();
static DIRECTORY_READERS: OnceLock<Mutex<HashMap<u64, DirectoryReader>>> = OnceLock::new();
static DIRECTORY_WALKERS: OnceLock<Mutex<HashMap<u64, DirectoryWalker>>> = OnceLock::new();
static NEXT_HANDLE: AtomicU64 = AtomicU64::new(1);
static NEXT_TEMP: AtomicU64 = AtomicU64::new(1);
static OPEN_FILE_COUNT: AtomicUsize = AtomicUsize::new(0);
static TEMPORARY_COUNT: AtomicUsize = AtomicUsize::new(0);
static TEMPORARIES: OnceLock<Mutex<HashMap<u64, TemporaryResource>>> = OnceLock::new();
static WATCHER_COUNT: AtomicUsize = AtomicUsize::new(0);
static FILE_WATCHERS: OnceLock<Mutex<HashMap<u64, FileWatcherResource>>> = OnceLock::new();
static LOCK_COUNT: AtomicUsize = AtomicUsize::new(0);
static FILE_LOCKS: OnceLock<Mutex<HashMap<u64, FileLockResource>>> = OnceLock::new();
static LOCKED_FILE_HANDLES: OnceLock<Mutex<HashSet<u64>>> = OnceLock::new();

struct FileLockResource {
    file: cap_std::fs::File,
    file_handle: u64,
}

struct FileWatcherResource {
    root: PathBuf,
    recursive: bool,
    watcher: Arc<Mutex<notify::RecommendedWatcher>>,
    receiver: Arc<Mutex<Receiver<WatcherRecord>>>,
    overflow: Arc<AtomicBool>,
    async_waiters: Arc<Mutex<HashSet<(u64, crate::task::TaskId)>>>,
    watched_paths: HashSet<PathBuf>,
    closed_reported: bool,
}

enum WatcherRecord {
    Created(String),
    Modified(String),
    Removed(String),
    Renamed(String),
    Closed,
    Error(FsError),
}

struct TemporaryReservation {
    committed: bool,
}

struct LockReservation {
    committed: bool,
}

struct AcquiredFileLock {
    file: Option<cap_std::fs::File>,
    file_handle: u64,
    reservation: Option<LockReservation>,
    registered_handle: bool,
    native_locked: bool,
}

struct WatcherReservation {
    committed: bool,
}

#[derive(Clone, Copy)]
struct TemporaryIdentity {
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
}

enum TemporaryObject {
    File(cap_std::fs::File),
    Directory(Dir),
}

struct TemporaryResource {
    path: String,
    parent_scope: String,
    operation: &'static str,
    parent: Dir,
    name: std::ffi::OsString,
    identity: TemporaryIdentity,
    _object: TemporaryObject,
}

fn reserve_file_slot(count: &AtomicUsize) -> Result<(), FsError> {
    count
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
            (current < MAX_OPEN_FILES).then_some(current + 1)
        })
        .map(|_| ())
        .map_err(|_| FsError::Limit)
}

fn release_file_slot(count: &AtomicUsize) {
    let previous = count.fetch_sub(1, Ordering::AcqRel);
    debug_assert!(previous > 0, "filesystem handle count underflow");
}

struct OpenFileReservation {
    committed: bool,
}

struct DirectoryReader {
    path: String,
    operation: &'static str,
    entries: ReadDir,
}

struct WalkFrame {
    directory: Dir,
    path: String,
    depth: u32,
    entries: ReadDir,
}

struct DirectoryWalker {
    root: String,
    max_depth: u32,
    max_entries: u64,
    visited: u64,
    frames: Vec<WalkFrame>,
}

fn directory_readers() -> &'static Mutex<HashMap<u64, DirectoryReader>> {
    DIRECTORY_READERS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn directory_walkers() -> &'static Mutex<HashMap<u64, DirectoryWalker>> {
    DIRECTORY_WALKERS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn register_directory_reader(
    path: String,
    operation: &'static str,
    entries: ReadDir,
) -> Result<u64, FsError> {
    register_directory_reader_resource(DirectoryReader {
        path,
        operation,
        entries,
    })
}

fn register_directory_reader_resource(reader: DirectoryReader) -> Result<u64, FsError> {
    let mut readers = directory_readers()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if readers.len() >= MAX_OPEN_DIRECTORIES {
        return Err(FsError::Limit);
    }
    let handle = loop {
        let candidate = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed).max(1);
        if !readers.contains_key(&candidate) {
            break candidate;
        }
    };
    readers.insert(handle, reader);
    Ok(handle)
}

fn directory_entry(operation: i32, args: &[i64]) -> Result<FilesystemOut, FsError> {
    let handle = argument_u64(args, 0)?;
    if operation == DIRECTORY_ENTRIES_CLOSE {
        directory_readers()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&handle)
            .ok_or(FsError::Closed)?;
        return Ok(FilesystemOut::value());
    }
    Ok(output_string(&directory_entry_value(handle)?))
}

fn directory_entry_value(handle: u64) -> Result<String, FsError> {
    let (path, permission_operation) = {
        let readers = directory_readers()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let reader = readers.get(&handle).ok_or(FsError::Closed)?;
        (reader.path.clone(), reader.operation)
    };
    authorize_operation(
        "filesystem.read",
        &path,
        permission_operation,
        FilesystemScopeExtent::Directory,
    )?;
    let mut readers = directory_readers()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let reader = readers.get_mut(&handle).ok_or(FsError::Closed)?;
    let Some(entry) = reader.entries.next() else {
        return Ok(String::new());
    };
    let entry = entry.map_err(map_io)?;
    let path = HostPath::new(&reader.path).join(entry.file_name());
    let path = path_text(&path)?;
    let kind = entry.file_type().map_err(map_io)?;
    let marker = if kind.is_file() {
        'f'
    } else if kind.is_dir() {
        'd'
    } else if kind.is_symlink() {
        'l'
    } else {
        'o'
    };
    Ok(format!("{marker}{path}"))
}

fn register_directory_walker(
    root: String,
    directory: Dir,
    max_depth: u32,
    max_entries: u64,
) -> Result<u64, FsError> {
    let walker = open_directory_walker(root, directory, max_depth, max_entries)?;
    register_directory_walker_resource(walker)
}

fn open_directory_walker(
    root: String,
    directory: Dir,
    max_depth: u32,
    max_entries: u64,
) -> Result<DirectoryWalker, FsError> {
    if max_depth > MAX_DIRECTORY_DEPTH || max_entries == 0 || max_entries > MAX_DIRECTORY_ENTRIES {
        return Err(FsError::Limit);
    }
    let entries = directory.entries().map_err(map_io)?;
    Ok(DirectoryWalker {
        root: root.clone(),
        max_depth,
        max_entries,
        visited: 0,
        frames: vec![WalkFrame {
            directory,
            path: root,
            depth: 0,
            entries,
        }],
    })
}

fn register_directory_walker_resource(walker: DirectoryWalker) -> Result<u64, FsError> {
    let mut walkers = directory_walkers()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if walkers.len() >= MAX_OPEN_DIRECTORIES {
        return Err(FsError::Limit);
    }
    let handle = loop {
        let candidate = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed).max(1);
        if !walkers.contains_key(&candidate) {
            break candidate;
        }
    };
    walkers.insert(handle, walker);
    Ok(handle)
}

fn directory_walk_entry(operation: i32, args: &[i64]) -> Result<FilesystemOut, FsError> {
    let handle = argument_u64(args, 0)?;
    if operation == DIRECTORY_WALK_CLOSE {
        directory_walkers()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&handle)
            .ok_or(FsError::Closed)?;
        return Ok(FilesystemOut::value());
    }
    Ok(output_string(&directory_walk_next_value(handle)?))
}

fn directory_walk_next_value(handle: u64) -> Result<String, FsError> {
    let root = {
        let walkers = directory_walkers()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        walkers.get(&handle).ok_or(FsError::Closed)?.root.clone()
    };
    authorize_operation(
        "filesystem.read",
        &root,
        "directory.walk",
        FilesystemScopeExtent::Descendants,
    )?;
    let mut walkers = directory_walkers()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let walker = walkers.get_mut(&handle).ok_or(FsError::Closed)?;
    loop {
        let Some(mut frame) = walker.frames.pop() else {
            return Ok(String::new());
        };
        let next = frame.entries.next();
        match next {
            None => continue,
            Some(Err(error)) => return Err(map_io(error)),
            Some(Ok(entry)) => {
                walker.visited = walker.visited.checked_add(1).ok_or(FsError::Limit)?;
                if walker.visited > walker.max_entries {
                    return Err(FsError::Limit);
                }
                let name = entry.file_name();
                let path = path_text(&HostPath::new(&frame.path).join(&name))?;
                let kind = entry.file_type().map_err(map_io)?;
                let marker = if kind.is_file() {
                    'f'
                } else if kind.is_dir() {
                    'd'
                } else if kind.is_symlink() {
                    'l'
                } else {
                    'o'
                };
                if kind.is_dir() && frame.depth >= walker.max_depth {
                    return Err(FsError::Limit);
                }
                let descend = kind.is_dir();
                let child = if descend {
                    match frame.directory.open_dir_nofollow(&name) {
                        Ok(directory) => Some(directory),
                        Err(error) if error.kind() == io::ErrorKind::NotFound => {
                            return Err(FsError::InvalidPath);
                        }
                        Err(error) => return Err(map_io(error)),
                    }
                } else {
                    None
                };
                walker.frames.push(frame);
                if let Some(directory) = child {
                    let entries = directory.entries().map_err(map_io)?;
                    let depth = walker
                        .frames
                        .last()
                        .map(|parent| parent.depth.saturating_add(1))
                        .ok_or(FsError::InvalidPath)?;
                    walker.frames.push(WalkFrame {
                        directory,
                        path: path.clone(),
                        depth,
                        entries,
                    });
                }
                return Ok(format!("{marker}{path}"));
            }
        }
    }
}

fn file_watchers() -> &'static Mutex<HashMap<u64, FileWatcherResource>> {
    FILE_WATCHERS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn notify_watcher_waiters(waiters: &Arc<Mutex<HashSet<(u64, crate::task::TaskId)>>>) {
    let targets = std::mem::take(
        &mut *waiters
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
    );
    for (executor, task) in targets {
        crate::executor::notify_network_task(executor, task);
    }
}

struct WatcherWaiterRegistration {
    waiters: Arc<Mutex<HashSet<(u64, crate::task::TaskId)>>>,
    target: (u64, crate::task::TaskId),
}

impl Drop for WatcherWaiterRegistration {
    fn drop(&mut self) {
        self.waiters
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&self.target);
    }
}

fn register_watcher_waiter(handle: u64) -> Result<WatcherWaiterRegistration, FsError> {
    if !crate::executor::in_executor_task() {
        return Err(FsError::Unsupported);
    }
    let waiters = {
        let watchers = file_watchers()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Arc::clone(&watchers.get(&handle).ok_or(FsError::Closed)?.async_waiters)
    };
    let target = (
        crate::executor::current_executor_id(),
        crate::executor::current_task(),
    );
    let mut registered = waiters
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if !registered.contains(&target) && registered.len() >= crate::task::MAX_ACTIVE_TASKS {
        return Err(FsError::Limit);
    }
    registered.insert(target);
    drop(registered);
    Ok(WatcherWaiterRegistration { waiters, target })
}

fn map_notify_error(error: notify::Error) -> FsError {
    match error.kind {
        notify::ErrorKind::Io(error) => map_io(error),
        notify::ErrorKind::PathNotFound => FsError::NotFound,
        notify::ErrorKind::WatchNotFound => FsError::Closed,
        notify::ErrorKind::MaxFilesWatch => FsError::Limit,
        _ => FsError::Io,
    }
}

fn watcher_record_from_notify_error(error: notify::Error) -> WatcherRecord {
    if matches!(error.kind, notify::ErrorKind::WatchNotFound) {
        WatcherRecord::Closed
    } else {
        WatcherRecord::Error(map_notify_error(error))
    }
}

fn watcher_record_from_kind(marker: char, path: String) -> WatcherRecord {
    match marker {
        'c' => WatcherRecord::Created(path),
        'm' => WatcherRecord::Modified(path),
        'd' => WatcherRecord::Removed(path),
        'r' => WatcherRecord::Renamed(path),
        _ => WatcherRecord::Error(FsError::Io),
    }
}

fn watcher_event_marker(kind: &notify::EventKind) -> Option<char> {
    use notify::event::{EventKind, ModifyKind};
    match kind {
        EventKind::Create(_) => Some('c'),
        EventKind::Remove(_) => Some('d'),
        EventKind::Modify(ModifyKind::Name(_)) => Some('r'),
        EventKind::Modify(_) | EventKind::Any => Some('m'),
        EventKind::Access(_) | EventKind::Other => None,
    }
}

fn watcher_event_path(root: &HostPath, event_path: &HostPath, marker: char) -> Option<String> {
    let absolute = if event_path.is_absolute() {
        event_path.to_path_buf()
    } else {
        root.join(event_path)
    };
    let relative = absolute.strip_prefix(root).ok()?;
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
    {
        return None;
    }
    let parent = absolute.parent()?;
    let canonical_parent = std::fs::canonicalize(parent).ok()?;
    if !canonical_parent.starts_with(root) {
        return None;
    }
    if matches!(marker, 'm' | 'r')
        && std::fs::symlink_metadata(&absolute)
            .ok()
            .is_some_and(|metadata| metadata.file_type().is_symlink())
    {
        // Recursive host backends can report a target change using the link's
        // spelling. Never publish that as an event from the approved subtree.
        return None;
    }
    let text = path_text(relative).ok()?;
    (text.len() <= MAX_WATCH_EVENT_PATH_BYTES).then_some(text)
}

fn encode_watcher_record(record: WatcherRecord) -> String {
    match record {
        WatcherRecord::Created(path) => format!("c{path}"),
        WatcherRecord::Modified(path) => format!("m{path}"),
        WatcherRecord::Removed(path) => format!("d{path}"),
        WatcherRecord::Renamed(path) => format!("r{path}"),
        WatcherRecord::Closed => "x".to_owned(),
        WatcherRecord::Error(error) => {
            char::from(b'A' + error.file_discriminant() as u8).to_string()
        }
    }
}

fn enqueue_watcher_record(
    sender: &mpsc::SyncSender<WatcherRecord>,
    overflow: &AtomicBool,
    record: WatcherRecord,
) -> Result<(), ()> {
    match sender.try_send(record) {
        Ok(()) => Ok(()),
        Err(mpsc::TrySendError::Full(_)) => {
            overflow.store(true, Ordering::Release);
            Err(())
        }
        Err(mpsc::TrySendError::Disconnected(_)) => Err(()),
    }
}

fn watcher_request_extent(recursive: bool) -> FilesystemScopeExtent {
    if recursive {
        FilesystemScopeExtent::Descendants
    } else {
        FilesystemScopeExtent::Directory
    }
}

#[cfg(unix)]
fn directory_identity(metadata: &cap_std::fs::Metadata) -> (u64, u64) {
    use cap_std::fs::MetadataExt;
    (metadata.dev(), metadata.ino())
}

#[cfg(windows)]
fn directory_identity(metadata: &cap_std::fs::Metadata) -> (u64, u64) {
    use cap_fs_ext::MetadataExt;
    (metadata.dev(), metadata.ino())
}

#[cfg(unix)]
fn collect_watch_subdirectories(
    root: &HostPath,
    directory: &Dir,
    path: &HostPath,
    depth: u32,
    visited_entries: &mut u64,
    identities: &mut HashSet<(u64, u64)>,
    output: &mut Vec<(PathBuf, (u64, u64))>,
) -> Result<(), FsError> {
    for entry in directory.entries().map_err(map_io)? {
        let entry = entry.map_err(map_io)?;
        *visited_entries = visited_entries.checked_add(1).ok_or(FsError::Limit)?;
        if *visited_entries > MAX_DIRECTORY_ENTRIES {
            return Err(FsError::Limit);
        }
        let name = entry.file_name();
        let expected = directory.symlink_metadata(&name).map_err(map_io)?;
        if expected.file_type().is_symlink() || !expected.is_dir() {
            continue;
        }
        if depth >= MAX_DIRECTORY_DEPTH {
            return Err(FsError::Limit);
        }
        let candidate = path.join(&name);
        let candidate_scope = path_scope(&candidate)?;
        authorize_operation(
            "filesystem.metadata",
            &candidate_scope,
            "directory.watch",
            FilesystemScopeExtent::Descendants,
        )?;
        let child = directory.open_dir_nofollow(&name).map_err(map_io)?;
        let current = child.dir_metadata().map_err(map_io)?;
        if !same_cap_metadata_snapshot(&expected, &current) {
            return Err(FsError::InvalidPath);
        }
        let canonical = std::fs::canonicalize(&candidate).map_err(map_io)?;
        if !canonical.starts_with(root) {
            return Err(FsError::PermissionDenied);
        }
        let identity = directory_identity(&current);
        if !identities.insert(identity) {
            // Bind-mounted cycles and aliases are not recursively watched a
            // second time; the root notification still allows a safe rescan.
            continue;
        }
        if output.len() >= MAX_WATCHED_DIRECTORIES as usize {
            return Err(FsError::Limit);
        }
        output.push((canonical.clone(), identity));
        collect_watch_subdirectories(
            root,
            &child,
            &canonical,
            depth.saturating_add(1),
            visited_entries,
            identities,
            output,
        )?;
    }
    Ok(())
}

#[cfg(unix)]
fn register_native_watch(
    watcher: &mut notify::RecommendedWatcher,
    root: &HostPath,
    path: &HostPath,
    expected_identity: (u64, u64),
    extent: FilesystemScopeExtent,
) -> Result<(), FsError> {
    if !path.starts_with(root) {
        return Err(FsError::PermissionDenied);
    }
    let scope = path_scope(path)?;
    let opened = open_directory_chain(path)?;
    if directory_identity(&opened.dir_metadata().map_err(map_io)?) != expected_identity {
        return Err(FsError::InvalidPath);
    }
    let verified = open_directory_chain(path)?;
    if directory_identity(&verified.dir_metadata().map_err(map_io)?) != expected_identity {
        return Err(FsError::InvalidPath);
    }
    authorize_operation("filesystem.metadata", &scope, "directory.watch", extent)?;
    watcher
        .watch(path, notify::RecursiveMode::NonRecursive)
        .map_err(map_notify_error)?;
    let after = open_directory_chain(path);
    if after
        .as_ref()
        .ok()
        .and_then(|directory| directory.dir_metadata().ok())
        .is_none_or(|metadata| directory_identity(&metadata) != expected_identity)
    {
        let _ = watcher.unwatch(path);
        return Err(FsError::InvalidPath);
    }
    Ok(())
}

#[cfg(unix)]
fn directory_watch(args: &[i64]) -> Result<FilesystemOut, FsError> {
    let parent_path = unsafe { argument_string(args, 0)? };
    let recursive = argument_u64(args, 1)? != 0;
    let capacity = u32::try_from(argument_u64(args, 2)?).map_err(|_| FsError::Limit)?;
    if capacity == 0 || capacity > MAX_WATCHER_QUEUE_CAPACITY {
        return Err(FsError::Limit);
    }
    let extent = watcher_request_extent(recursive);
    let reservation = WatcherReservation::reserve()?;
    let (root_text, root_directory) = open_authorized_directory(
        &parent_path,
        "filesystem.metadata",
        "directory.watch",
        extent,
    )?;
    let root = PathBuf::from(root_text);
    let identity = directory_identity(&root_directory.dir_metadata().map_err(map_io)?);
    let (sender, receiver) = mpsc::sync_channel::<WatcherRecord>(capacity as usize);
    let overflow = Arc::new(AtomicBool::new(false));
    let callback_overflow = Arc::clone(&overflow);
    let callback_root = root.clone();
    let async_waiters = Arc::new(Mutex::new(HashSet::new()));
    let callback_waiters = Arc::clone(&async_waiters);
    let watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        let event = match event {
            Ok(event) => event,
            Err(error) => {
                let record = watcher_record_from_notify_error(error);
                let _ = enqueue_watcher_record(&sender, &callback_overflow, record);
                notify_watcher_waiters(&callback_waiters);
                return;
            }
        };
        let mut should_notify = false;
        if event.need_rescan() || event.paths.len() > MAX_WATCH_PATHS_PER_EVENT {
            callback_overflow.store(true, Ordering::Release);
            should_notify = true;
            if event.paths.len() > MAX_WATCH_PATHS_PER_EVENT {
                notify_watcher_waiters(&callback_waiters);
                return;
            }
        }
        let Some(marker) = watcher_event_marker(&event.kind) else {
            if should_notify {
                notify_watcher_waiters(&callback_waiters);
            }
            return;
        };
        for path in event.paths {
            let Some(path) = watcher_event_path(&callback_root, &path, marker) else {
                continue;
            };
            if enqueue_watcher_record(
                &sender,
                &callback_overflow,
                watcher_record_from_kind(marker, path),
            )
            .is_err()
            {
                should_notify = true;
                break;
            }
            should_notify = true;
        }
        if should_notify {
            notify_watcher_waiters(&callback_waiters);
        }
    })
    .map_err(map_notify_error)?;
    let mut watcher = watcher;
    register_native_watch(&mut watcher, &root, &root, identity, extent)?;
    let mut watched_paths = HashSet::from([root.clone()]);
    if recursive {
        let mut visited_entries = 0;
        let mut identities = HashSet::from([identity]);
        let mut directories = Vec::new();
        collect_watch_subdirectories(
            &root,
            &root_directory,
            &root,
            0,
            &mut visited_entries,
            &mut identities,
            &mut directories,
        )?;
        if directories.len() as u64 + 1 > MAX_WATCHED_DIRECTORIES {
            return Err(FsError::Limit);
        }
        for (path, child_identity) in directories {
            register_native_watch(&mut watcher, &root, &path, child_identity, extent)?;
            watched_paths.insert(path);
        }
        // The root watch becomes active before existing descendants do. The
        // consumer must rescan once because changes could occur in that gap.
        if watched_paths.len() > 1 {
            overflow.store(true, Ordering::Release);
        }
    }
    let mut watchers = file_watchers()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let handle = loop {
        let candidate = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed).max(1);
        if !watchers.contains_key(&candidate) {
            break candidate;
        }
    };
    watchers.insert(
        handle,
        FileWatcherResource {
            root,
            recursive,
            watcher: Arc::new(Mutex::new(watcher)),
            receiver: Arc::new(Mutex::new(receiver)),
            overflow,
            async_waiters,
            watched_paths,
            closed_reported: false,
        },
    );
    drop(watchers);
    reservation.commit();
    Ok(FilesystemOut {
        scalar: handle,
        ..FilesystemOut::value()
    })
}

#[cfg(any(windows, not(any(unix, windows))))]
fn directory_watch(_args: &[i64]) -> Result<FilesystemOut, FsError> {
    Err(FsError::Unsupported)
}

#[cfg(unix)]
fn register_created_watch_subtree(handle: u64, relative: &str) -> Result<bool, FsError> {
    let (root, recursive, watcher_handle) = {
        let watchers = file_watchers()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let watcher = watchers.get(&handle).ok_or(FsError::Closed)?;
        (
            watcher.root.clone(),
            watcher.recursive,
            Arc::clone(&watcher.watcher),
        )
    };
    if !recursive {
        return Ok(false);
    }
    let path = root.join(relative);
    if !path.starts_with(&root) {
        return Err(FsError::PermissionDenied);
    }
    let resolved = resolve_target_entry(path.to_str().ok_or(FsError::InvalidPath)?)?;
    let metadata = match resolved.parent.symlink_metadata(&resolved.name) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(map_io(error)),
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Ok(false);
    }
    let (scope, directory) = open_authorized_directory(
        path.to_str().ok_or(FsError::InvalidPath)?,
        "filesystem.metadata",
        "directory.watch",
        FilesystemScopeExtent::Descendants,
    )?;
    let scope_path = PathBuf::from(scope);
    if !scope_path.starts_with(&root) {
        return Err(FsError::PermissionDenied);
    }
    let root_identity = directory_identity(&directory.dir_metadata().map_err(map_io)?);
    let mut identities = HashSet::from([root_identity]);
    let mut directories = vec![(scope_path.clone(), root_identity)];
    let mut visited_entries = 0;
    collect_watch_subdirectories(
        &root,
        &directory,
        &scope_path,
        0,
        &mut visited_entries,
        &mut identities,
        &mut directories,
    )?;
    let mut native_watcher = watcher_handle
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let existing = {
        let watchers = file_watchers()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let watcher = watchers.get(&handle).ok_or(FsError::Closed)?;
        if !watcher.recursive || !Arc::ptr_eq(&watcher_handle, &watcher.watcher) {
            return Err(FsError::Closed);
        }
        watcher.watched_paths.clone()
    };
    let additional = directories
        .iter()
        .filter(|(path, _)| !existing.contains(path))
        .count();
    if existing.len().saturating_add(additional) > MAX_WATCHED_DIRECTORIES as usize {
        return Err(FsError::Limit);
    }
    let mut added: Vec<PathBuf> = Vec::new();
    for (directory_path, identity) in directories {
        if existing.contains(&directory_path) {
            continue;
        }
        let result = register_native_watch(
            &mut native_watcher,
            &root,
            &directory_path,
            identity,
            FilesystemScopeExtent::Descendants,
        );
        if let Err(error) = result {
            for added_path in added {
                let _ = native_watcher.unwatch(&added_path);
            }
            return Err(error);
        }
        added.push(directory_path);
    }
    if !added.is_empty() {
        let mut watchers = file_watchers()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let watcher = watchers.get_mut(&handle).ok_or(FsError::Closed)?;
        if !Arc::ptr_eq(&watcher_handle, &watcher.watcher) {
            return Err(FsError::Closed);
        }
        watcher.watched_paths.extend(added.iter().cloned());
        // There may have been changes between the create event and installing
        // the new non-recursive watches. Report the gap instead of implying a
        // complete recursive history.
        watcher.overflow.store(true, Ordering::Release);
    }
    Ok(!added.is_empty())
}

#[cfg(unix)]
fn invalidate_recursive_watch_subtree(
    handle: u64,
    relative: &str,
    force_rescan: bool,
) -> Result<(), FsError> {
    let (root, watcher_handle, candidates) = {
        let watchers = file_watchers()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let watcher = watchers.get(&handle).ok_or(FsError::Closed)?;
        if !watcher.recursive {
            return Ok(());
        }
        let path = watcher.root.join(relative);
        if !path.starts_with(&watcher.root) {
            return Err(FsError::PermissionDenied);
        }
        let candidates = watcher
            .watched_paths
            .iter()
            .filter(|candidate| *candidate != &watcher.root && candidate.starts_with(&path))
            .cloned()
            .collect::<Vec<_>>();
        (
            watcher.root.clone(),
            Arc::clone(&watcher.watcher),
            candidates,
        )
    };
    if candidates.is_empty() && !force_rescan {
        return Ok(());
    }
    let had_candidates = !candidates.is_empty();

    let mut native_watcher = watcher_handle
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut removed = Vec::new();
    let mut first_error = None;
    for path in candidates {
        if !path.starts_with(&root) {
            first_error.get_or_insert(FsError::PermissionDenied);
            continue;
        }
        match native_watcher.unwatch(&path) {
            Ok(()) => removed.push(path),
            Err(error) if matches!(error.kind, notify::ErrorKind::WatchNotFound) => {
                removed.push(path)
            }
            Err(error) => {
                first_error.get_or_insert_with(|| map_notify_error(error));
            }
        }
    }
    drop(native_watcher);

    let mut watchers = file_watchers()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let watcher = watchers.get_mut(&handle).ok_or(FsError::Closed)?;
    if !Arc::ptr_eq(&watcher_handle, &watcher.watcher) {
        return Err(FsError::Closed);
    }
    for path in removed {
        watcher.watched_paths.remove(&path);
    }
    if force_rescan || had_candidates {
        watcher.overflow.store(true, Ordering::Release);
    }
    first_error.map_or(Ok(()), Err)
}

#[cfg(any(windows, not(any(unix, windows))))]
fn register_created_watch_subtree(_handle: u64, _relative: &str) -> Result<bool, FsError> {
    Err(FsError::Unsupported)
}

#[cfg(any(windows, not(any(unix, windows))))]
fn invalidate_recursive_watch_subtree(
    _handle: u64,
    _relative: &str,
    _force_rescan: bool,
) -> Result<(), FsError> {
    Err(FsError::Unsupported)
}

fn watcher_next(args: &[i64]) -> Result<FilesystemOut, FsError> {
    let handle = argument_u64(args, 0)?;
    let (root, recursive) = {
        let watchers = file_watchers()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let watcher = watchers.get(&handle).ok_or(FsError::Closed)?;
        (watcher.root.clone(), watcher.recursive)
    };
    authorize_operation(
        "filesystem.metadata",
        &path_scope(&root)?,
        "directory.watch",
        watcher_request_extent(recursive),
    )?;
    let (record, disconnected) = {
        let mut watchers = file_watchers()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let watcher = watchers.get_mut(&handle).ok_or(FsError::Closed)?;
        if watcher.closed_reported {
            return Err(FsError::Closed);
        }
        if watcher.overflow.swap(false, Ordering::AcqRel) {
            return Ok(output_string("o"));
        }
        let receiver = watcher
            .receiver
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match receiver.try_recv() {
            Ok(record) => (Some(record), false),
            Err(TryRecvError::Empty) => return Err(FsError::WouldBlock),
            Err(TryRecvError::Disconnected) => {
                watcher.closed_reported = true;
                (None, true)
            }
        }
    };
    if disconnected {
        return Ok(output_string("x"));
    }
    let record = record.ok_or(FsError::Closed)?;
    if matches!(&record, WatcherRecord::Closed) {
        let mut watchers = file_watchers()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(watcher) = watchers.get_mut(&handle) {
            watcher.closed_reported = true;
        }
        return Ok(output_string(&encode_watcher_record(record)));
    }
    if let WatcherRecord::Created(path) = &record
        && recursive
    {
        register_created_watch_subtree(handle, path)?;
    }
    match &record {
        WatcherRecord::Removed(path) if recursive => {
            invalidate_recursive_watch_subtree(handle, path, false)?;
        }
        WatcherRecord::Renamed(path) if recursive => {
            invalidate_recursive_watch_subtree(handle, path, true)?;
        }
        _ => {}
    }
    Ok(output_string(&encode_watcher_record(record)))
}

fn watcher_next_async(args: &[i64]) -> Result<FilesystemOut, FsError> {
    if !crate::executor::in_executor_task() {
        return Err(FsError::Unsupported);
    }
    let handle = argument_u64(args, 0)?;
    loop {
        if crate::executor::current_task_cancel_requested() {
            return Err(FsError::Cancelled);
        }
        let epoch = crate::executor::network_event_epoch();
        let _registration = register_watcher_waiter(handle)?;
        match watcher_next(args) {
            Ok(event) => return Ok(event),
            Err(FsError::WouldBlock) => {
                match crate::executor::park_current_for_network_event_if_unchanged(epoch) {
                    crate::executor::NetworkPark::EventOccurred
                    | crate::executor::NetworkPark::Parked => {}
                    crate::executor::NetworkPark::CapacityExceeded => {
                        return Err(FsError::Limit);
                    }
                }
            }
            Err(error) => return Err(error),
        }
    }
}

fn watcher_close(args: &[i64]) -> Result<FilesystemOut, FsError> {
    let handle = argument_u64(args, 0)?;
    let removed = file_watchers()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(&handle)
        .ok_or(FsError::Closed)?;
    notify_watcher_waiters(&removed.async_waiters);
    drop(removed);
    release_file_slot(&WATCHER_COUNT);
    Ok(FilesystemOut::value())
}

fn directory_size(
    path: &str,
    max_depth: u32,
    max_entries: u64,
    max_bytes: u64,
) -> Result<u64, FsError> {
    if max_depth > MAX_DIRECTORY_DEPTH
        || max_entries == 0
        || max_entries > MAX_DIRECTORY_ENTRIES
        || max_bytes == 0
        || max_bytes > MAX_DIRECTORY_BYTES
    {
        return Err(FsError::Limit);
    }
    let (root, directory) = open_authorized_directory(
        path,
        "filesystem.metadata",
        "directory.size",
        FilesystemScopeExtent::Descendants,
    )?;
    let entries = directory.entries().map_err(map_io)?;
    let mut frames = vec![WalkFrame {
        directory,
        path: root.clone(),
        depth: 0,
        entries,
    }];
    let mut visited = 0u64;
    let mut bytes = 0u64;
    loop {
        let Some(mut frame) = frames.pop() else {
            return Ok(bytes);
        };
        authorize_operation(
            "filesystem.metadata",
            &root,
            "directory.size",
            FilesystemScopeExtent::Descendants,
        )?;
        let next = frame.entries.next();
        match next {
            None => continue,
            Some(Err(error)) => return Err(map_io(error)),
            Some(Ok(entry)) => {
                visited = visited.checked_add(1).ok_or(FsError::Limit)?;
                if visited > max_entries {
                    return Err(FsError::Limit);
                }
                let name = entry.file_name();
                let kind = entry.file_type().map_err(map_io)?;
                if kind.is_dir() {
                    if frame.depth >= max_depth {
                        return Err(FsError::Limit);
                    }
                    let child = frame.directory.open_dir_nofollow(&name).map_err(map_io)?;
                    let child_entries = child.entries().map_err(map_io)?;
                    let child_path = path_text(&HostPath::new(&frame.path).join(&name))?;
                    let depth = frame.depth.saturating_add(1);
                    frames.push(frame);
                    frames.push(WalkFrame {
                        directory: child,
                        path: child_path,
                        depth,
                        entries: child_entries,
                    });
                } else {
                    let metadata = frame.directory.symlink_metadata(&name).map_err(map_io)?;
                    bytes = bytes
                        .checked_add(metadata.len())
                        .filter(|size| *size <= max_bytes)
                        .ok_or(FsError::Limit)?;
                    frames.push(frame);
                }
            }
        }
    }
}

fn link_is_link(path: &str) -> Result<FilesystemOut, FsError> {
    let resolved = resolve_target_entry(path)?;
    let scope = path_scope(&resolved.canonical)?;
    authorize_operation(
        "filesystem.metadata",
        &scope,
        "link.is_link",
        FilesystemScopeExtent::Exact,
    )?;
    let metadata = resolved
        .parent
        .symlink_metadata(&resolved.name)
        .map_err(map_io)?;
    Ok(FilesystemOut {
        scalar: u64::from(metadata.file_type().is_symlink()),
        ..FilesystemOut::value()
    })
}

fn link_read_target(path: &str) -> Result<FilesystemOut, FsError> {
    let resolved = resolve_target_entry(path)?;
    let scope = path_scope(&resolved.canonical)?;
    authorize_operation(
        "filesystem.metadata",
        &scope,
        "link.read_target",
        FilesystemScopeExtent::Exact,
    )?;
    let metadata = resolved
        .parent
        .symlink_metadata(&resolved.name)
        .map_err(map_io)?;
    if !metadata.file_type().is_symlink() {
        return Err(FsError::WrongType);
    }
    authorize_operation(
        "filesystem.metadata",
        &scope,
        "link.read_target",
        FilesystemScopeExtent::Exact,
    )?;
    let target = resolved
        .parent
        .read_link_contents(&resolved.name)
        .map_err(map_io)?;
    Ok(output_string(&target.to_str().ok_or(FsError::InvalidPath)?))
}

fn link_create(target: &str, destination: &str, kind: u64) -> Result<(), FsError> {
    if target.is_empty() || target.contains('\0') {
        return Err(FsError::InvalidPath);
    }
    if kind > 1 {
        return Err(FsError::InvalidOptions);
    }
    let destination = resolve_target_entry(destination)?;
    let scope = path_scope(&destination.canonical)?;
    authorize_operation(
        "filesystem.write",
        &scope,
        "link.create.destination",
        FilesystemScopeExtent::Exact,
    )?;
    match destination.parent.symlink_metadata(&destination.name) {
        Ok(_) => return Err(FsError::AlreadyExists),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(map_io(error)),
    }
    authorize_operation(
        "filesystem.write",
        &scope,
        "link.create.destination",
        FilesystemScopeExtent::Exact,
    )?;
    let target = HostPath::new(target);
    match kind {
        0 => {
            #[cfg(unix)]
            destination
                .parent
                .symlink_contents(target, &destination.name)
                .map_err(map_io)?;
            #[cfg(windows)]
            destination
                .parent
                .symlink_file(target, &destination.name)
                .map_err(map_io)?;
            #[cfg(not(any(unix, windows)))]
            return Err(FsError::Unsupported);
        }
        1 => {
            #[cfg(unix)]
            destination
                .parent
                .symlink_contents(target, &destination.name)
                .map_err(map_io)?;
            #[cfg(windows)]
            destination
                .parent
                .symlink_dir(target, &destination.name)
                .map_err(map_io)?;
            #[cfg(not(any(unix, windows)))]
            return Err(FsError::Unsupported);
        }
        _ => return Err(FsError::InvalidOptions),
    }
    Ok(())
}

fn open_regular_file_nofollow(
    directory: &Dir,
    name: &std::ffi::OsStr,
) -> Result<cap_std::fs::File, FsError> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use cap_std::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    #[cfg(windows)]
    {
        use cap_std::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = directory.open_with(name, &options).map_err(map_io)?;
    if !file.metadata().map_err(map_io)?.is_file() {
        return Err(FsError::WrongType);
    }
    Ok(file)
}

fn open_new_file_nofollow(
    directory: &Dir,
    name: &std::ffi::OsStr,
) -> Result<cap_std::fs::File, FsError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use cap_std::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    #[cfg(windows)]
    {
        use cap_std::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    directory.open_with(name, &options).map_err(map_io)
}

fn copy_directory_tree(
    source: &Dir,
    destination: &Dir,
    source_scope: &str,
    destination_scope: &str,
    depth: u32,
    max_depth: u32,
    visited: &mut u64,
    max_entries: u64,
    copied_bytes: &mut u64,
) -> Result<(), FsError> {
    authorize_operation(
        "filesystem.read",
        source_scope,
        "directory.copy.source",
        FilesystemScopeExtent::Descendants,
    )?;
    authorize_operation(
        "filesystem.write",
        destination_scope,
        "directory.copy.destination",
        FilesystemScopeExtent::Descendants,
    )?;
    for entry in source.entries().map_err(map_io)? {
        let entry = entry.map_err(map_io)?;
        *visited = visited.checked_add(1).ok_or(FsError::Limit)?;
        if *visited > max_entries {
            return Err(FsError::Limit);
        }
        let name = entry.file_name();
        let kind = entry.file_type().map_err(map_io)?;
        authorize_operation(
            "filesystem.read",
            source_scope,
            "directory.copy.source",
            FilesystemScopeExtent::Descendants,
        )?;
        authorize_operation(
            "filesystem.write",
            destination_scope,
            "directory.copy.destination",
            FilesystemScopeExtent::Descendants,
        )?;
        if kind.is_symlink() {
            return Err(FsError::Unsupported);
        }
        if kind.is_dir() {
            if depth >= max_depth {
                return Err(FsError::Limit);
            }
            let source_child = source.open_dir_nofollow(&name).map_err(map_io)?;
            authorize_operation(
                "filesystem.write",
                destination_scope,
                "directory.copy.destination",
                FilesystemScopeExtent::Descendants,
            )?;
            destination.create_dir(&name).map_err(map_io)?;
            let destination_child = destination.open_dir_nofollow(&name).map_err(map_io)?;
            copy_directory_tree(
                &source_child,
                &destination_child,
                source_scope,
                destination_scope,
                depth.saturating_add(1),
                max_depth,
                visited,
                max_entries,
                copied_bytes,
            )?;
            continue;
        }
        if !kind.is_file() {
            return Err(FsError::Unsupported);
        }

        let expected = source.symlink_metadata(&name).map_err(map_io)?;
        if !expected.is_file() || expected.file_type().is_symlink() {
            return Err(FsError::InvalidPath);
        }
        #[cfg(windows)]
        {
            use cap_std::fs::MetadataExt;
            use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
            if expected.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                return Err(FsError::InvalidPath);
            }
        }
        authorize_operation(
            "filesystem.read",
            source_scope,
            "directory.copy.source",
            FilesystemScopeExtent::Descendants,
        )?;
        let mut input = open_regular_file_nofollow(source, &name)?;
        #[cfg(unix)]
        if !same_cap_metadata_snapshot(&expected, &input.metadata().map_err(map_io)?) {
            return Err(FsError::InvalidPath);
        }
        let remaining = MAX_DIRECTORY_BYTES
            .checked_sub(*copied_bytes)
            .ok_or(FsError::Limit)?;
        let source_size = input.metadata().map_err(map_io)?.len();
        if source_size > remaining {
            return Err(FsError::Limit);
        }
        authorize_operation(
            "filesystem.read",
            source_scope,
            "directory.copy.source",
            FilesystemScopeExtent::Descendants,
        )?;
        authorize_operation(
            "filesystem.write",
            destination_scope,
            "directory.copy.destination",
            FilesystemScopeExtent::Descendants,
        )?;
        let mut output = open_new_file_nofollow(destination, &name)?;
        authorize_operation(
            "filesystem.read",
            source_scope,
            "directory.copy.source",
            FilesystemScopeExtent::Descendants,
        )?;
        authorize_operation(
            "filesystem.write",
            destination_scope,
            "directory.copy.destination",
            FilesystemScopeExtent::Descendants,
        )?;
        let mut limited = (&mut input).take(remaining.saturating_add(1));
        let copied = io::copy(&mut limited, &mut output).map_err(map_io)?;
        if copied > remaining {
            return Err(FsError::Limit);
        }
        *copied_bytes = copied_bytes.checked_add(copied).ok_or(FsError::Limit)?;
    }
    Ok(())
}

fn replace_destination_entry(
    parent: &Dir,
    name: &std::ffi::OsStr,
    scope: &str,
    operation: &'static str,
    max_depth: u32,
    max_entries: u64,
) -> Result<(), FsError> {
    authorize_operation(
        "filesystem.write",
        scope,
        operation,
        FilesystemScopeExtent::Descendants,
    )?;
    let metadata = parent.symlink_metadata(name).map_err(map_io)?;
    let mut visited = 0;
    remove_tree_entry_with_operation(
        parent,
        name,
        scope,
        operation,
        0,
        max_depth,
        &mut visited,
        max_entries,
        metadata,
    )
}

fn remove_tree_entry_with_operation(
    parent: &Dir,
    name: &std::ffi::OsStr,
    scope: &str,
    operation: &'static str,
    depth: u32,
    max_depth: u32,
    visited: &mut u64,
    max_entries: u64,
    metadata: cap_std::fs::Metadata,
) -> Result<(), FsError> {
    *visited = visited.checked_add(1).ok_or(FsError::Limit)?;
    if *visited > max_entries || depth > max_depth {
        return Err(FsError::Limit);
    }
    authorize_operation(
        "filesystem.write",
        scope,
        operation,
        FilesystemScopeExtent::Descendants,
    )?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        parent.remove_file(name).map_err(map_io)?;
        return Ok(());
    }
    let directory = parent.open_dir_nofollow(name).map_err(map_io)?;
    #[cfg(windows)]
    let identity = directory_identity(&directory.dir_metadata().map_err(map_io)?);
    for entry in directory.entries().map_err(map_io)? {
        let entry = entry.map_err(map_io)?;
        let child_name = entry.file_name();
        let child_metadata = directory.symlink_metadata(&child_name).map_err(map_io)?;
        remove_tree_entry_with_operation(
            &directory,
            &child_name,
            scope,
            operation,
            depth.saturating_add(1),
            max_depth,
            visited,
            max_entries,
            child_metadata,
        )?;
    }
    authorize_operation(
        "filesystem.write",
        scope,
        operation,
        FilesystemScopeExtent::Descendants,
    )?;
    drop(directory);
    #[cfg(windows)]
    {
        let current = parent.open_dir_nofollow(name).map_err(map_io)?;
        if directory_identity(&current.dir_metadata().map_err(map_io)?) != identity {
            return Err(FsError::InvalidPath);
        }
        drop(current);
    }
    parent.remove_dir(name).map_err(map_io)
}

fn copy_directory(
    source_path: &str,
    destination_path: &str,
    replace: bool,
    max_depth: u32,
    max_entries: u64,
) -> Result<(), FsError> {
    if max_depth > MAX_DIRECTORY_DEPTH || max_entries == 0 || max_entries > MAX_DIRECTORY_ENTRIES {
        return Err(FsError::Limit);
    }
    let (source_scope, source) = open_authorized_directory(
        source_path,
        "filesystem.read",
        "directory.copy.source",
        FilesystemScopeExtent::Descendants,
    )?;
    let destination = resolve_target_entry(destination_path)?;
    if PathBuf::from(&destination.canonical).starts_with(&source_scope) {
        return Err(FsError::InvalidPath);
    }
    let destination_scope = path_scope(&destination.canonical)?;
    authorize_operation(
        "filesystem.write",
        &destination_scope,
        "directory.copy.destination",
        FilesystemScopeExtent::Descendants,
    )?;
    match destination.parent.symlink_metadata(&destination.name) {
        Ok(_) if !replace => return Err(FsError::AlreadyExists),
        Ok(_) => replace_destination_entry(
            &destination.parent,
            &destination.name,
            &destination_scope,
            "directory.copy.replace",
            max_depth,
            max_entries,
        )?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(map_io(error)),
    }
    authorize_operation(
        "filesystem.write",
        &destination_scope,
        "directory.copy.destination",
        FilesystemScopeExtent::Descendants,
    )?;
    destination
        .parent
        .create_dir(&destination.name)
        .map_err(map_io)?;
    let destination_root = destination
        .parent
        .open_dir_nofollow(&destination.name)
        .map_err(map_io)?;
    let mut visited = 0;
    let mut copied_bytes = 0;
    copy_directory_tree(
        &source,
        &destination_root,
        &source_scope,
        &destination_scope,
        0,
        max_depth,
        &mut visited,
        max_entries,
        &mut copied_bytes,
    )
}

fn rename_without_replacement(
    source_parent: &Dir,
    source_name: &std::ffi::OsStr,
    destination_parent: &Dir,
    destination_name: &std::ffi::OsStr,
) -> Result<(), FsError> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        use std::os::fd::AsRawFd;
        use std::os::unix::ffi::OsStrExt;

        let source_name =
            std::ffi::CString::new(source_name.as_bytes()).map_err(|_| FsError::InvalidPath)?;
        let destination_name = std::ffi::CString::new(destination_name.as_bytes())
            .map_err(|_| FsError::InvalidPath)?;
        #[cfg(target_os = "linux")]
        let result = unsafe {
            libc::renameat2(
                source_parent.as_raw_fd(),
                source_name.as_ptr(),
                destination_parent.as_raw_fd(),
                destination_name.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        };
        #[cfg(target_os = "macos")]
        let result = unsafe {
            libc::renameatx_np(
                source_parent.as_raw_fd(),
                source_name.as_ptr(),
                destination_parent.as_raw_fd(),
                destination_name.as_ptr(),
                libc::RENAME_EXCL,
            )
        };
        if result == 0 {
            Ok(())
        } else {
            Err(map_io(io::Error::last_os_error()))
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = (
            source_parent,
            source_name,
            destination_parent,
            destination_name,
        );
        Err(FsError::Unsupported)
    }
}

fn move_directory_entry(
    source_path: &str,
    destination_path: &str,
    replace: bool,
) -> Result<(), FsError> {
    let source = resolve_target_entry(source_path)?;
    let destination = resolve_target_entry(destination_path)?;
    let source_scope = path_scope(&source.canonical)?;
    let destination_scope = path_scope(&destination.canonical)?;

    authorize_operation(
        "filesystem.write",
        &source_scope,
        "directory.move.source",
        FilesystemScopeExtent::Exact,
    )?;
    authorize_operation(
        "filesystem.write",
        &destination_scope,
        "directory.move.destination",
        FilesystemScopeExtent::Exact,
    )?;

    let source_metadata = source
        .parent
        .symlink_metadata(&source.name)
        .map_err(map_io)?;
    if source_scope == destination_scope {
        return Ok(());
    }
    if source_metadata.is_dir() && destination.canonical.starts_with(&source.canonical) {
        return Err(FsError::InvalidPath);
    }

    let destination_exists = match destination.parent.symlink_metadata(&destination.name) {
        Ok(_) => true,
        Err(error) if error.kind() == io::ErrorKind::NotFound => false,
        Err(error) => return Err(map_io(error)),
    };
    if destination_exists && !replace {
        return Err(FsError::AlreadyExists);
    }
    if destination_exists {
        authorize_operation(
            "filesystem.write",
            &destination_scope,
            "directory.move.replace",
            FilesystemScopeExtent::Exact,
        )?;
    }
    authorize_operation(
        "filesystem.write",
        &source_scope,
        "directory.move.source",
        FilesystemScopeExtent::Exact,
    )?;
    authorize_operation(
        "filesystem.write",
        &destination_scope,
        if destination_exists {
            "directory.move.replace"
        } else {
            "directory.move.destination"
        },
        FilesystemScopeExtent::Exact,
    )?;

    if replace {
        source
            .parent
            .rename(&source.name, &destination.parent, &destination.name)
            .map_err(map_io)
    } else {
        rename_without_replacement(
            &source.parent,
            &source.name,
            &destination.parent,
            &destination.name,
        )
    }
}

fn remove_tree_entry(
    parent: &Dir,
    name: &std::ffi::OsStr,
    scope: &str,
    depth: u32,
    max_depth: u32,
    visited: &mut u64,
    max_entries: u64,
) -> Result<(), FsError> {
    *visited = visited.checked_add(1).ok_or(FsError::Limit)?;
    if *visited > max_entries || depth > max_depth {
        return Err(FsError::Limit);
    }
    authorize_operation(
        "filesystem.write",
        scope,
        "directory.remove_all",
        FilesystemScopeExtent::Descendants,
    )?;
    let metadata = parent.symlink_metadata(name).map_err(map_io)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        authorize_operation(
            "filesystem.write",
            scope,
            "directory.remove_all",
            FilesystemScopeExtent::Descendants,
        )?;
        parent.remove_file(name).map_err(map_io)?;
        return Ok(());
    }

    let directory = parent.open_dir_nofollow(name).map_err(map_io)?;
    #[cfg(windows)]
    let identity = directory_identity(&directory.dir_metadata().map_err(map_io)?);
    for entry in directory.entries().map_err(map_io)? {
        let entry = entry.map_err(map_io)?;
        remove_tree_entry(
            &directory,
            &entry.file_name(),
            scope,
            depth.saturating_add(1),
            max_depth,
            visited,
            max_entries,
        )?;
    }
    authorize_operation(
        "filesystem.write",
        scope,
        "directory.remove_all",
        FilesystemScopeExtent::Descendants,
    )?;
    drop(directory);
    #[cfg(windows)]
    {
        let current = parent.open_dir_nofollow(name).map_err(map_io)?;
        if directory_identity(&current.dir_metadata().map_err(map_io)?) != identity {
            return Err(FsError::InvalidPath);
        }
        drop(current);
    }
    parent.remove_dir(name).map_err(map_io)
}

fn remove_one(path: &str) -> Result<(), FsError> {
    let resolved = resolve_target_entry(path)?;
    let scope = path_scope(&resolved.canonical)?;
    authorize_operation(
        "filesystem.write",
        &scope,
        "directory.remove",
        FilesystemScopeExtent::Exact,
    )?;
    let metadata = resolved
        .parent
        .symlink_metadata(&resolved.name)
        .map_err(map_io)?;
    authorize_operation(
        "filesystem.write",
        &scope,
        "directory.remove",
        FilesystemScopeExtent::Exact,
    )?;
    if metadata.is_dir() && !metadata.file_type().is_symlink() {
        resolved.parent.remove_dir(&resolved.name).map_err(map_io)
    } else {
        resolved.parent.remove_file(&resolved.name).map_err(map_io)
    }
}

fn remove_all(path: &str, max_depth: u32, max_entries: u64) -> Result<(), FsError> {
    if max_depth > MAX_DIRECTORY_DEPTH || max_entries == 0 || max_entries > MAX_DIRECTORY_ENTRIES {
        return Err(FsError::Limit);
    }
    let resolved = resolve_target_entry(path)?;
    let scope = path_scope(&resolved.canonical)?;
    authorize_operation(
        "filesystem.write",
        &scope,
        "directory.remove_all",
        FilesystemScopeExtent::Descendants,
    )?;
    let mut visited = 0;
    remove_tree_entry(
        &resolved.parent,
        &resolved.name,
        &scope,
        0,
        max_depth,
        &mut visited,
        max_entries,
    )
}

impl OpenFileReservation {
    fn reserve() -> Result<Self, FsError> {
        reserve_file_slot(&OPEN_FILE_COUNT)?;
        Ok(Self { committed: false })
    }

    fn commit(mut self) {
        self.committed = true;
    }
}

impl TemporaryReservation {
    fn reserve() -> Result<Self, FsError> {
        TEMPORARY_COUNT
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                (current < MAX_TEMPORARY_RESOURCES).then_some(current + 1)
            })
            .map_err(|_| FsError::Limit)?;
        Ok(Self { committed: false })
    }

    fn commit(mut self) {
        self.committed = true;
    }
}

impl WatcherReservation {
    fn reserve() -> Result<Self, FsError> {
        WATCHER_COUNT
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                (current < MAX_OPEN_WATCHERS).then_some(current + 1)
            })
            .map_err(|_| FsError::Limit)?;
        Ok(Self { committed: false })
    }

    fn commit(mut self) {
        self.committed = true;
    }
}

impl Drop for WatcherReservation {
    fn drop(&mut self) {
        if !self.committed {
            release_file_slot(&WATCHER_COUNT);
        }
    }
}

impl Drop for TemporaryReservation {
    fn drop(&mut self) {
        if !self.committed {
            release_file_slot(&TEMPORARY_COUNT);
        }
    }
}

fn temporaries() -> &'static Mutex<HashMap<u64, TemporaryResource>> {
    TEMPORARIES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn file_locks() -> &'static Mutex<HashMap<u64, FileLockResource>> {
    FILE_LOCKS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn locked_file_handles() -> &'static Mutex<HashSet<u64>> {
    LOCKED_FILE_HANDLES.get_or_init(|| Mutex::new(HashSet::new()))
}

impl LockReservation {
    fn reserve() -> Result<Self, FsError> {
        LOCK_COUNT
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                (current < MAX_OPEN_LOCKS).then_some(current + 1)
            })
            .map_err(|_| FsError::Limit)?;
        Ok(Self { committed: false })
    }

    fn commit(mut self) {
        self.committed = true;
    }
}

impl Drop for LockReservation {
    fn drop(&mut self) {
        if !self.committed {
            release_file_slot(&LOCK_COUNT);
        }
    }
}

fn acquire_file_lock(
    file_handle: u64,
    exclusive: bool,
    nonblocking: bool,
) -> Result<FilesystemOut, FsError> {
    let acquired = acquire_file_lock_resource(file_handle, exclusive, nonblocking, None)?;
    let handle = register_acquired_file_lock(acquired);
    Ok(FilesystemOut {
        scalar: handle,
        ..FilesystemOut::value()
    })
}

impl Drop for AcquiredFileLock {
    fn drop(&mut self) {
        #[cfg(unix)]
        if self.native_locked
            && let Some(file) = &self.file
        {
            use std::os::fd::AsRawFd;
            unsafe {
                libc::flock(file.as_raw_fd(), libc::LOCK_UN);
            }
        }
        if self.registered_handle {
            locked_file_handles()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .remove(&self.file_handle);
        }
        drop(self.reservation.take());
    }
}

fn acquire_file_lock_resource(
    file_handle: u64,
    exclusive: bool,
    nonblocking: bool,
    cancellation: Option<crate::managed_blocking::OwnedJobCancellation>,
) -> Result<AcquiredFileLock, FsError> {
    #[cfg(not(unix))]
    {
        let _ = (file_handle, exclusive, nonblocking);
        return Err(FsError::Unsupported);
    }

    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;

        let reservation = LockReservation::reserve()?;
        let (path, file) = {
            let files = files()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let open = files.get(&file_handle).ok_or(FsError::Closed)?;
            if exclusive && !open.write {
                return Err(FsError::InvalidOptions);
            }
            if !exclusive && !open.read {
                return Err(FsError::InvalidOptions);
            }
            (open.path.clone(), open.file.try_clone().map_err(map_io)?)
        };
        {
            let mut locked = locked_file_handles()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if !locked.insert(file_handle) {
                return Err(FsError::WouldBlock);
            }
        }
        let mut acquired = AcquiredFileLock {
            file: Some(file),
            file_handle,
            reservation: Some(reservation),
            registered_handle: true,
            native_locked: false,
        };
        let operation = if exclusive {
            "file.lock.exclusive"
        } else {
            "file.lock.shared"
        };
        authorize_operation(
            if exclusive {
                "filesystem.write"
            } else {
                "filesystem.read"
            },
            &path,
            operation,
            FilesystemScopeExtent::Exact,
        )?;
        let mut flags = if exclusive {
            libc::LOCK_EX
        } else {
            libc::LOCK_SH
        };
        if let Some(cancellation) = cancellation {
            loop {
                if cancellation.is_cancelled() {
                    return Err(FsError::Cancelled);
                }
                authorize_operation(
                    if exclusive {
                        "filesystem.write"
                    } else {
                        "filesystem.read"
                    },
                    &path,
                    operation,
                    FilesystemScopeExtent::Exact,
                )?;
                let result = unsafe {
                    libc::flock(
                        acquired.file.as_ref().unwrap().as_raw_fd(),
                        flags | libc::LOCK_NB,
                    )
                };
                if result == 0 {
                    break;
                }
                let error = io::Error::last_os_error();
                if error.kind() != io::ErrorKind::WouldBlock {
                    return Err(map_io(error));
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        } else {
            authorize_operation(
                if exclusive {
                    "filesystem.write"
                } else {
                    "filesystem.read"
                },
                &path,
                operation,
                FilesystemScopeExtent::Exact,
            )?;
            if nonblocking {
                flags |= libc::LOCK_NB;
            }
            if unsafe { libc::flock(acquired.file.as_ref().unwrap().as_raw_fd(), flags) } != 0 {
                return Err(map_io(io::Error::last_os_error()));
            }
        }
        acquired.native_locked = true;
        Ok(acquired)
    }
}

fn register_acquired_file_lock(mut acquired: AcquiredFileLock) -> u64 {
    let mut locks = file_locks()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let handle = loop {
        let candidate = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed).max(1);
        if !locks.contains_key(&candidate) {
            break candidate;
        }
    };
    locks.insert(
        handle,
        FileLockResource {
            file: acquired
                .file
                .take()
                .expect("newly acquired lock owns its file"),
            file_handle: acquired.file_handle,
        },
    );
    acquired.native_locked = false;
    acquired.registered_handle = false;
    if let Some(reservation) = acquired.reservation.take() {
        reservation.commit();
    }
    handle
}

fn release_file_lock(handle: u64) -> Result<(), FsError> {
    #[cfg(windows)]
    return Err(FsError::Unsupported);

    #[cfg(not(unix))]
    return Err(FsError::Unsupported);

    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;

        let resource = file_locks()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&handle)
            .ok_or(FsError::Closed)?;
        if unsafe { libc::flock(resource.file.as_raw_fd(), libc::LOCK_UN) } != 0 {
            let error = map_io(io::Error::last_os_error());
            file_locks()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(handle, resource);
            return Err(error);
        }
        locked_file_handles()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&resource.file_handle);
        drop(resource);
        release_file_slot(&LOCK_COUNT);
        Ok(())
    }
}

fn temporary_identity(metadata: &cap_std::fs::Metadata) -> Result<TemporaryIdentity, FsError> {
    #[cfg(unix)]
    {
        use cap_std::fs::MetadataExt;
        Ok(TemporaryIdentity {
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }
    #[cfg(windows)]
    {
        let _ = metadata;
        Err(FsError::Unsupported)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = metadata;
        Err(FsError::Unsupported)
    }
}

fn temporary_identity_matches(
    identity: TemporaryIdentity,
    metadata: &cap_std::fs::Metadata,
) -> Result<bool, FsError> {
    #[cfg(unix)]
    {
        use cap_std::fs::MetadataExt;
        Ok(identity.device == metadata.dev() && identity.inode == metadata.ino())
    }
    #[cfg(windows)]
    {
        let _ = (identity, metadata);
        Err(FsError::Unsupported)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (identity, metadata);
        Err(FsError::Unsupported)
    }
}

fn remove_temporary_file_if_owned(
    parent: &Dir,
    name: &std::ffi::OsStr,
    identity: TemporaryIdentity,
    authorize_remove: impl FnOnce() -> Result<(), FsError>,
) -> Result<(), FsError> {
    let current = parent.symlink_metadata(name).map_err(map_io)?;
    if !temporary_identity_matches(identity, &current)? || current.is_dir() {
        return Err(FsError::InvalidPath);
    }
    authorize_remove()?;
    parent.remove_file(name).map_err(map_io)
}

fn remove_owned_temporary_contents(
    directory: &Dir,
    parent_scope: &str,
    operation: &'static str,
    depth: u32,
    visited: &mut u64,
) -> Result<(), FsError> {
    if depth > MAX_DIRECTORY_DEPTH {
        return Err(FsError::Limit);
    }
    for entry in directory.entries().map_err(map_io)? {
        let entry = entry.map_err(map_io)?;
        *visited = visited.checked_add(1).ok_or(FsError::Limit)?;
        if *visited > MAX_DIRECTORY_ENTRIES {
            return Err(FsError::Limit);
        }
        authorize_operation(
            "filesystem.write",
            parent_scope,
            operation,
            FilesystemScopeExtent::Directory,
        )?;
        let name = entry.file_name();
        let metadata = directory.symlink_metadata(&name).map_err(map_io)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            authorize_operation(
                "filesystem.write",
                parent_scope,
                operation,
                FilesystemScopeExtent::Directory,
            )?;
            directory.remove_file(&name).map_err(map_io)?;
        } else {
            let child = directory.open_dir_nofollow(&name).map_err(map_io)?;
            remove_owned_temporary_contents(
                &child,
                parent_scope,
                operation,
                depth.saturating_add(1),
                visited,
            )?;
            authorize_operation(
                "filesystem.write",
                parent_scope,
                operation,
                FilesystemScopeExtent::Directory,
            )?;
            directory.remove_dir(&name).map_err(map_io)?;
        }
    }
    Ok(())
}

fn cleanup_temporary_resource(resource: &TemporaryResource) -> Result<(), FsError> {
    authorize_operation(
        "filesystem.write",
        &resource.parent_scope,
        resource.operation,
        FilesystemScopeExtent::Directory,
    )?;
    let metadata = resource
        .parent
        .symlink_metadata(&resource.name)
        .map_err(map_io)?;
    if !temporary_identity_matches(resource.identity, &metadata)? {
        return Err(FsError::InvalidPath);
    }
    authorize_operation(
        "filesystem.write",
        &resource.parent_scope,
        resource.operation,
        FilesystemScopeExtent::Directory,
    )?;
    match &resource._object {
        TemporaryObject::File(_) => remove_temporary_file_if_owned(
            &resource.parent,
            &resource.name,
            resource.identity,
            || {
                authorize_operation(
                    "filesystem.write",
                    &resource.parent_scope,
                    resource.operation,
                    FilesystemScopeExtent::Directory,
                )
            },
        ),
        TemporaryObject::Directory(_) => {
            let directory = resource
                .parent
                .open_dir_nofollow(&resource.name)
                .map_err(map_io)?;
            if !temporary_identity_matches(
                resource.identity,
                &directory.dir_metadata().map_err(map_io)?,
            )? {
                return Err(FsError::InvalidPath);
            }
            let mut visited = 0;
            remove_owned_temporary_contents(
                &directory,
                &resource.parent_scope,
                resource.operation,
                0,
                &mut visited,
            )?;
            authorize_operation(
                "filesystem.write",
                &resource.parent_scope,
                resource.operation,
                FilesystemScopeExtent::Directory,
            )?;
            let current = resource
                .parent
                .symlink_metadata(&resource.name)
                .map_err(map_io)?;
            if !temporary_identity_matches(resource.identity, &current)? {
                return Err(FsError::InvalidPath);
            }
            resource.parent.remove_dir(&resource.name).map_err(map_io)
        }
    }
}

fn temporary_create(parent_path: &str, is_directory: bool) -> Result<FilesystemOut, FsError> {
    #[cfg(windows)]
    return Err(FsError::Unsupported);

    let operation = if is_directory {
        "temporary.directory"
    } else {
        "temporary.file"
    };
    let reservation = TemporaryReservation::reserve()?;
    let (parent_scope, parent) = open_authorized_directory(
        parent_path,
        "filesystem.write",
        operation,
        FilesystemScopeExtent::Directory,
    )?;
    for _ in 0..32 {
        let mut entropy = [0u8; 16];
        getrandom::getrandom(&mut entropy).map_err(|_| FsError::Io)?;
        let suffix = entropy
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let name = std::ffi::OsString::from(format!(".zirk-tmp-{suffix}"));
        authorize_operation(
            "filesystem.write",
            &parent_scope,
            operation,
            FilesystemScopeExtent::Directory,
        )?;
        let object = if is_directory {
            match parent.create_dir(&name) {
                Ok(()) => {
                    TemporaryObject::Directory(parent.open_dir_nofollow(&name).map_err(map_io)?)
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(map_io(error)),
            }
        } else {
            let mut options = OpenOptions::new();
            options.read(true).write(true).create_new(true);
            #[cfg(unix)]
            {
                use cap_std::fs::OpenOptionsExt;
                options.custom_flags(libc::O_NOFOLLOW);
            }
            match parent.open_with(&name, &options) {
                Ok(file) => TemporaryObject::File(file),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(map_io(error)),
            }
        };
        let metadata = match &object {
            TemporaryObject::File(file) => file.metadata().map_err(map_io)?,
            TemporaryObject::Directory(directory) => directory.dir_metadata().map_err(map_io)?,
        };
        let identity = temporary_identity(&metadata)?;
        let path = HostPath::new(&parent_scope).join(&name);
        let path = path_text(&path)?;
        let mut resources = temporaries()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let handle = loop {
            let candidate = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed).max(1);
            if !resources.contains_key(&candidate) {
                break candidate;
            }
        };
        resources.insert(
            handle,
            TemporaryResource {
                path,
                parent_scope,
                operation,
                parent,
                name,
                identity,
                _object: object,
            },
        );
        reservation.commit();
        return Ok(FilesystemOut {
            scalar: handle,
            ..FilesystemOut::value()
        });
    }
    Err(FsError::Limit)
}

fn temporary_path(handle: u64) -> Result<FilesystemOut, FsError> {
    let resources = temporaries()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let resource = resources.get(&handle).ok_or(FsError::Closed)?;
    Ok(output_string(&resource.path))
}

fn temporary_release(handle: u64) -> Result<(), FsError> {
    let resource = temporaries()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(&handle)
        .ok_or(FsError::Closed)?;
    drop(resource);
    release_file_slot(&TEMPORARY_COUNT);
    Ok(())
}

fn temporary_cleanup(handle: u64) -> Result<(), FsError> {
    let resource = temporaries()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(&handle)
        .ok_or(FsError::Closed)?;
    let cleanup = cleanup_temporary_resource(&resource);
    if cleanup.is_err() {
        temporaries()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(handle, resource);
    } else {
        release_file_slot(&TEMPORARY_COUNT);
    }
    cleanup
}

impl Drop for OpenFileReservation {
    fn drop(&mut self) {
        if !self.committed {
            release_file_slot(&OPEN_FILE_COUNT);
        }
    }
}

fn files() -> &'static Mutex<HashMap<u64, OpenFile>> {
    FILES.get_or_init(|| Mutex::new(HashMap::new()))
}

struct ResolvedPath {
    canonical: PathBuf,
    parent: Dir,
    name: std::ffi::OsString,
    metadata: Option<std::fs::Metadata>,
}

fn lexical_absolute(path: &HostPath) -> Result<PathBuf, FsError> {
    if path.as_os_str().is_empty() || path.as_os_str().to_string_lossy().contains('\0') {
        return Err(FsError::InvalidPath);
    }
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().map_err(map_io)?.join(path)
    };
    Ok(normalize_host_path(&absolute))
}

fn normalize_host_path(path: &HostPath) -> PathBuf {
    let mut output = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => output.push(prefix.as_os_str()),
            Component::RootDir => output.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if output.file_name().is_some() {
                    output.pop();
                } else if !output.has_root() {
                    output.push(component.as_os_str());
                }
            }
            Component::Normal(value) => output.push(value),
        }
    }
    if output.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        output
    }
}

fn resolve_path(path: &str, allow_missing: bool) -> Result<ResolvedPath, FsError> {
    if path.is_empty() || path.contains('\0') {
        return Err(FsError::InvalidPath);
    }
    let requested = lexical_absolute(HostPath::new(path))?;
    let (canonical, metadata) = match std::fs::canonicalize(&requested) {
        Ok(canonical) => {
            let metadata = std::fs::metadata(&canonical).map_err(map_io)?;
            (canonical, Some(metadata))
        }
        Err(error) if allow_missing && error.kind() == io::ErrorKind::NotFound => {
            let name = requested.file_name().ok_or(FsError::InvalidPath)?;
            if name == "." || name == ".." {
                return Err(FsError::InvalidPath);
            }
            let parent = requested.parent().ok_or(FsError::InvalidPath)?;
            let parent = std::fs::canonicalize(parent).map_err(map_io)?;
            (parent.join(name), None)
        }
        Err(error) => return Err(map_io(error)),
    };
    let parent_path = canonical.parent().ok_or(FsError::InvalidPath)?;
    let name = canonical
        .file_name()
        .ok_or(FsError::InvalidPath)?
        .to_os_string();
    let expected_parent = std::fs::metadata(parent_path).map_err(map_io)?;
    let parent = open_verified_parent(parent_path, &expected_parent)?;
    Ok(ResolvedPath {
        canonical,
        parent,
        name,
        metadata,
    })
}

fn open_verified_parent(
    parent_path: &HostPath,
    expected: &std::fs::Metadata,
) -> Result<Dir, FsError> {
    let parent = open_directory_chain(parent_path)?;
    #[cfg(unix)]
    if !same_metadata_snapshot(expected, &parent.dir_metadata().map_err(map_io)?) {
        return Err(FsError::InvalidPath);
    }
    #[cfg(windows)]
    {
        let _ = expected;
        if !windows_handle_matches_path(&parent, parent_path)? {
            return Err(FsError::InvalidPath);
        }
    }
    Ok(parent)
}

#[cfg(unix)]
fn open_directory_chain(path: &HostPath) -> Result<Dir, FsError> {
    if !path.is_absolute() {
        return Err(FsError::InvalidPath);
    }
    let mut current =
        Dir::open_ambient_dir(HostPath::new("/"), ambient_authority()).map_err(map_io)?;
    for component in path.components() {
        match component {
            Component::RootDir | Component::CurDir => {}
            Component::Normal(name) => {
                // Open exactly one component at a time without following a
                // symlink. Keeping each resulting Dir open pins the validated
                // parent while the next component is resolved relative to it.
                current = current.open_dir_nofollow(name).map_err(map_io)?;
            }
            Component::ParentDir | Component::Prefix(_) => return Err(FsError::InvalidPath),
        }
    }
    Ok(current)
}

#[cfg(windows)]
fn open_directory_chain(path: &HostPath) -> Result<Dir, FsError> {
    if !path.is_absolute() {
        return Err(FsError::InvalidPath);
    }
    let mut components = path.components();
    let Some(Component::Prefix(prefix)) = components.next() else {
        return Err(FsError::InvalidPath);
    };
    if !matches!(components.next(), Some(Component::RootDir)) {
        return Err(FsError::InvalidPath);
    }
    let mut root_path = PathBuf::from(prefix.as_os_str());
    root_path.push(std::path::MAIN_SEPARATOR_STR);
    let mut current = Dir::open_ambient_dir(&root_path, ambient_authority()).map_err(map_io)?;
    for component in components {
        match component {
            Component::RootDir | Component::CurDir => {}
            Component::Normal(name) => {
                current = current.open_dir_nofollow(name).map_err(map_io)?;
            }
            Component::ParentDir | Component::Prefix(_) => return Err(FsError::InvalidPath),
        }
    }
    Ok(current)
}

#[cfg(not(any(unix, windows)))]
fn open_directory_chain(_path: &HostPath) -> Result<Dir, FsError> {
    Err(FsError::Unsupported)
}

fn path_text(path: &HostPath) -> Result<String, FsError> {
    path.to_str().map(str::to_owned).ok_or(FsError::InvalidPath)
}

fn path_scope(path: &HostPath) -> Result<String, FsError> {
    path_text(path)
}

#[cfg(unix)]
fn same_metadata_snapshot(left: &std::fs::Metadata, right: &cap_std::fs::Metadata) -> bool {
    use cap_std::fs::MetadataExt as CapMetadataExt;
    use std::os::unix::fs::MetadataExt as StdMetadataExt;
    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(windows)]
fn windows_handle_matches_path<T>(handle_owner: &T, expected: &HostPath) -> Result<bool, FsError>
where
    T: std::os::windows::io::AsRawHandle,
{
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_NAME_NORMALIZED, GetFinalPathNameByHandleW, VOLUME_NAME_DOS,
    };

    let handle = handle_owner.as_raw_handle().cast();
    let flags = FILE_NAME_NORMALIZED | VOLUME_NAME_DOS;
    let mut capacity = 256usize;
    loop {
        let mut buffer = vec![0u16; capacity];
        let written = unsafe {
            GetFinalPathNameByHandleW(handle, buffer.as_mut_ptr(), capacity as u32, flags)
        } as usize;
        if written == 0 {
            return Err(map_io(io::Error::last_os_error()));
        }
        if written < capacity {
            buffer.truncate(written);
            let actual = String::from_utf16(&buffer).map_err(|_| FsError::InvalidPath)?;
            let expected = expected.to_str().ok_or(FsError::InvalidPath)?;
            return Ok(normalize_windows_path(&actual) == normalize_windows_path(expected));
        }
        capacity = written.checked_add(1).ok_or(FsError::InvalidPath)?;
        if capacity > 32768 {
            return Err(FsError::InvalidPath);
        }
    }
}

#[cfg(windows)]
fn normalize_windows_path(path: &str) -> String {
    let normalized = path.replace('/', "\\");
    let normalized = normalized
        .strip_prefix("\\\\?\\UNC\\")
        .map(|rest| format!("\\\\{rest}"))
        .or_else(|| normalized.strip_prefix("\\\\?\\").map(str::to_owned))
        .unwrap_or(normalized);
    if normalized.len() > 3 {
        normalized.trim_end_matches('\\').to_owned()
    } else {
        normalized
    }
}

fn verify_opened_identity(
    file: &cap_std::fs::File,
    resolved: &ResolvedPath,
) -> Result<(), FsError> {
    let actual = file.metadata().map_err(map_io)?;
    #[cfg(unix)]
    if let Some(expected) = &resolved.metadata {
        if !same_metadata_snapshot(expected, &actual) {
            return Err(FsError::InvalidPath);
        }
    } else if resolved.parent.symlink_metadata(&resolved.name).is_err() {
        return Err(FsError::InvalidPath);
    }
    #[cfg(windows)]
    if !windows_handle_matches_path(file, &resolved.canonical)? {
        return Err(FsError::InvalidPath);
    }
    #[cfg(not(any(unix, windows)))]
    return Err(FsError::Unsupported);
    if !actual.is_file() {
        return Err(FsError::WrongType);
    }
    #[cfg(windows)]
    {
        use cap_std::fs::MetadataExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
        if actual.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(FsError::InvalidPath);
        }
    }
    Ok(())
}

fn authorize(capability: &str, path: &str) -> Result<(), FsError> {
    if crate::permissions::request_resource(capability, path) {
        Ok(())
    } else {
        Err(FsError::PermissionDenied)
    }
}

fn authorize_operation(
    capability: &str,
    path: &str,
    operation: &str,
    extent: FilesystemScopeExtent,
) -> Result<(), FsError> {
    if crate::permissions::request_filesystem_operation(capability, path, operation, extent) {
        Ok(())
    } else {
        Err(FsError::PermissionDenied)
    }
}

/// Resolves a target through its verified parent while preserving the final
/// directory entry itself. This lets directory and link operations reject a
/// final symlink instead of silently authorizing its target.
fn resolve_target_entry(path: &str) -> Result<ResolvedPath, FsError> {
    if path.is_empty() || path.contains('\0') {
        return Err(FsError::InvalidPath);
    }
    let requested = lexical_absolute(HostPath::new(path))?;
    let name = requested
        .file_name()
        .ok_or(FsError::InvalidPath)?
        .to_os_string();
    let parent_path = requested.parent().ok_or(FsError::InvalidPath)?;
    let parent_path = std::fs::canonicalize(parent_path).map_err(map_io)?;
    let expected_parent = std::fs::metadata(&parent_path).map_err(map_io)?;
    let parent = open_verified_parent(&parent_path, &expected_parent)?;
    let canonical = parent_path.join(&name);
    Ok(ResolvedPath {
        canonical,
        parent,
        name,
        metadata: None,
    })
}

fn create_directories(path: &str) -> Result<(), FsError> {
    let requested = lexical_absolute(HostPath::new(path))?;
    let mut existing = requested.clone();
    let metadata = loop {
        match std::fs::symlink_metadata(&existing) {
            Ok(metadata) => break metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                if !existing.pop() {
                    return Err(FsError::InvalidPath);
                }
            }
            Err(error) => return Err(map_io(error)),
        }
    };
    if metadata.file_type().is_symlink() {
        return Err(FsError::InvalidPath);
    }
    if !metadata.is_dir() {
        return Err(FsError::WrongType);
    }
    let relative = requested
        .strip_prefix(&existing)
        .map_err(|_| FsError::InvalidPath)?
        .to_path_buf();
    let mut component_count = 0usize;
    for component in relative.components() {
        if matches!(component, Component::Normal(_)) {
            component_count += 1;
        } else if !matches!(component, Component::CurDir) {
            return Err(FsError::InvalidPath);
        }
    }
    if component_count > 256 {
        return Err(FsError::Limit);
    }

    let approved_root = std::fs::canonicalize(&existing).map_err(map_io)?;
    let scope = path_scope(&approved_root)?;
    authorize_operation(
        "filesystem.write",
        &scope,
        "directory.create_all",
        FilesystemScopeExtent::Descendants,
    )?;
    let mut current = open_directory_chain(&approved_root)?;
    for component in relative.components() {
        let Component::Normal(name) = component else {
            if matches!(component, Component::CurDir) {
                continue;
            }
            return Err(FsError::InvalidPath);
        };
        // Revalidate the operation scope immediately before each possible
        // directory creation, rather than relying on the initial prompt.
        authorize_operation(
            "filesystem.write",
            &scope,
            "directory.create_all",
            FilesystemScopeExtent::Descendants,
        )?;
        match current.create_dir(name) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(map_io(error)),
        }
        current = current.open_dir_nofollow(name).map_err(map_io)?;
    }
    Ok(())
}

fn open_authorized_directory(
    path: &str,
    capability: &'static str,
    operation: &'static str,
    extent: FilesystemScopeExtent,
) -> Result<(String, Dir), FsError> {
    let requested = lexical_absolute(HostPath::new(path))?;
    if requested.parent().is_none() {
        let canonical = std::fs::canonicalize(&requested).map_err(map_io)?;
        let scope = path_scope(&canonical)?;
        authorize_operation(capability, &scope, operation, extent)?;
        let directory = open_directory_chain(&canonical)?;
        return Ok((scope, directory));
    }
    let resolved = resolve_target_entry(path)?;
    let scope = path_scope(&resolved.canonical)?;
    authorize_operation(capability, &scope, operation, extent)?;
    let expected = resolved
        .parent
        .symlink_metadata(&resolved.name)
        .map_err(map_io)?;
    let kind = expected.file_type();
    if kind.is_symlink() {
        return Err(FsError::InvalidPath);
    }
    if !kind.is_dir() {
        return Err(FsError::WrongType);
    }
    let current = resolved
        .parent
        .symlink_metadata(&resolved.name)
        .map_err(map_io)?;
    authorize_operation(capability, &scope, operation, extent)?;
    #[cfg(unix)]
    if !same_cap_metadata_snapshot(&expected, &current) {
        return Err(FsError::InvalidPath);
    }
    let dir = resolved
        .parent
        .open_dir_nofollow(&resolved.name)
        .map_err(map_io)?;
    #[cfg(unix)]
    if !same_cap_metadata_snapshot(&current, &dir.dir_metadata().map_err(map_io)?) {
        return Err(FsError::InvalidPath);
    }
    #[cfg(windows)]
    if !windows_handle_matches_path(&dir, &resolved.canonical)? {
        return Err(FsError::InvalidPath);
    }
    Ok((scope, dir))
}

#[cfg(unix)]
fn same_cap_metadata_snapshot(left: &cap_std::fs::Metadata, right: &cap_std::fs::Metadata) -> bool {
    use cap_std::fs::MetadataExt as _;
    left.dev() == right.dev() && left.ino() == right.ino()
}

fn resolve_authorized(
    path: &str,
    capability: &str,
    allow_missing: bool,
) -> Result<ResolvedPath, FsError> {
    match resolve_path(path, allow_missing) {
        Ok(resolved) => {
            let scope = path_scope(&resolved.canonical)?;
            authorize(capability, &scope)?;
            Ok(resolved)
        }
        Err(FsError::NotFound) => {
            // An absent path has no physical identity yet. Authorize its
            // normalized absolute spelling before returning absence so an
            // unapproved caller cannot use NotFound as an existence oracle.
            let requested = lexical_absolute(HostPath::new(path))?;
            let scope = path_scope(&requested)?;
            authorize(capability, &scope)?;
            Err(FsError::NotFound)
        }
        Err(error) => Err(error),
    }
}

fn open_resolved(
    resolved: &ResolvedPath,
    read: bool,
    write: bool,
    append: bool,
    create_new: bool,
) -> Result<cap_std::fs::File, FsError> {
    let mut options = OpenOptions::new();
    options.read(read).write(write && !append).append(append);
    if create_new || resolved.metadata.is_none() {
        options.create_new(true);
    }
    #[cfg(unix)]
    {
        use cap_std::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    #[cfg(windows)]
    {
        use cap_std::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = resolved
        .parent
        .open_with(&resolved.name, &options)
        .map_err(map_io)?;
    verify_opened_identity(&file, resolved)?;
    Ok(file)
}

fn validate_open_flags(flags: u32) -> Result<(), FsError> {
    if flags
        & !(FLAG_READ | FLAG_WRITE | FLAG_APPEND | FLAG_CREATE | FLAG_CREATE_NEW | FLAG_TRUNCATE)
        != 0
        || flags & (FLAG_READ | FLAG_WRITE | FLAG_APPEND) == 0
        || (flags & FLAG_CREATE != 0 && flags & FLAG_CREATE_NEW != 0)
        || (flags & FLAG_APPEND != 0 && flags & FLAG_TRUNCATE != 0)
        || (flags & (FLAG_CREATE | FLAG_CREATE_NEW | FLAG_TRUNCATE) != 0
            && flags & (FLAG_WRITE | FLAG_APPEND) == 0)
    {
        return Err(FsError::InvalidOptions);
    }
    Ok(())
}

fn encode_text(text: &str, encoding: u64) -> Result<Vec<u8>, FsError> {
    match encoding {
        0 => Ok(text.as_bytes().to_vec()),
        1 => text
            .chars()
            .map(|character| u8::try_from(u32::from(character)).map_err(|_| FsError::Decoding))
            .collect(),
        2 | 3 => {
            let mut bytes = Vec::with_capacity(text.len().saturating_mul(2));
            for unit in text.encode_utf16() {
                let encoded = if encoding == 2 {
                    unit.to_le_bytes()
                } else {
                    unit.to_be_bytes()
                };
                bytes.extend_from_slice(&encoded);
            }
            Ok(bytes)
        }
        _ => Err(FsError::Unsupported),
    }
}

fn decode_text(bytes: &[u8], encoding: u64) -> Result<String, FsError> {
    match encoding {
        0 => String::from_utf8(bytes.to_vec()).map_err(|_| FsError::Decoding),
        1 => Ok(bytes.iter().map(|byte| char::from(*byte)).collect()),
        2 | 3 => {
            if !bytes.len().is_multiple_of(2) {
                return Err(FsError::Decoding);
            }
            let units = bytes
                .chunks_exact(2)
                .map(|pair| {
                    if encoding == 2 {
                        u16::from_le_bytes([pair[0], pair[1]])
                    } else {
                        u16::from_be_bytes([pair[0], pair[1]])
                    }
                })
                .collect::<Vec<_>>();
            String::from_utf16(&units).map_err(|_| FsError::Decoding)
        }
        _ => Err(FsError::Unsupported),
    }
}

fn read_bounded(file: &mut cap_std::fs::File, requested: u64) -> Result<Vec<u8>, FsError> {
    read_bounded_authorized(file, requested, || Ok(()))
}

fn read_bounded_authorized(
    file: &mut cap_std::fs::File,
    requested: u64,
    mut authorize_read: impl FnMut() -> Result<(), FsError>,
) -> Result<Vec<u8>, FsError> {
    if requested > MAX_WHOLE_FILE_BYTES {
        return Err(FsError::Limit);
    }
    authorize_read()?;
    let metadata = file.metadata().map_err(map_io)?;
    if metadata.len() > requested || metadata.len() > MAX_WHOLE_FILE_BYTES {
        return Err(FsError::Limit);
    }
    let initial = usize::try_from(metadata.len()).unwrap_or(0);
    let mut output = Vec::with_capacity(initial);
    let mut buffer = [0u8; 16 * 1024];
    loop {
        let remaining = requested
            .min(MAX_WHOLE_FILE_BYTES)
            .saturating_sub(output.len() as u64);
        let read_limit = usize::try_from(remaining.saturating_add(1).min(buffer.len() as u64))
            .unwrap_or(buffer.len());
        if read_limit == 0 {
            let mut probe = [0u8; 1];
            authorize_read()?;
            if file.read(&mut probe).map_err(map_io)? != 0 {
                return Err(FsError::Limit);
            }
            break;
        }
        authorize_read()?;
        let count = file.read(&mut buffer[..read_limit]).map_err(map_io)?;
        if count == 0 {
            break;
        }
        output.extend_from_slice(&buffer[..count]);
        if output.len() as u64 > requested || output.len() as u64 > MAX_WHOLE_FILE_BYTES {
            return Err(FsError::Limit);
        }
    }
    Ok(output)
}

fn read_open_file(
    open: &mut OpenFile,
    max_bytes: u64,
    authorize_operation: impl FnOnce(&str, &str) -> Result<(), FsError>,
) -> Result<Vec<u8>, FsError> {
    if max_bytes > MAX_CHUNK_BYTES {
        return Err(FsError::Limit);
    }
    if !open.read {
        return Err(FsError::InvalidOptions);
    }
    authorize_operation("filesystem.read", &open.path)?;
    let mut bytes = vec![0; usize::try_from(max_bytes).map_err(|_| FsError::Limit)?];
    let read = open.file.read(&mut bytes).map_err(map_io)?;
    bytes.truncate(read);
    Ok(bytes)
}

fn make_metadata_array(metadata: &cap_std::fs::Metadata) -> *mut c_void {
    let modified = metadata
        .modified()
        .ok()
        .and_then(|time| time.into_std().duration_since(UNIX_EPOCH).ok())
        .and_then(|duration| u64::try_from(duration.as_millis()).ok());
    let accessed = metadata
        .accessed()
        .ok()
        .and_then(|time| time.into_std().duration_since(UNIX_EPOCH).ok())
        .and_then(|duration| u64::try_from(duration.as_millis()).ok());
    let created = metadata
        .created()
        .ok()
        .and_then(|time| time.into_std().duration_since(UNIX_EPOCH).ok())
        .and_then(|duration| u64::try_from(duration.as_millis()).ok());
    let values = [
        metadata.len(),
        u64::from(metadata.is_file()),
        u64::from(metadata.is_dir()),
        modified.unwrap_or_default(),
        u64::from(modified.is_some()),
        u64::from(metadata.file_type().is_symlink()),
        u64::from(metadata.permissions().readonly()),
        accessed.unwrap_or_default(),
        u64::from(accessed.is_some()),
        created.unwrap_or_default(),
        u64::from(created.is_some()),
    ];
    crate::array::alloc_u64_array(&values)
}

fn output_string(text: &str) -> FilesystemOut {
    FilesystemOut {
        payload: crate::string::alloc_owned(text),
        ..FilesystemOut::value()
    }
}

fn output_bytes(bytes: &[u8]) -> FilesystemOut {
    FilesystemOut {
        payload: crate::array::alloc_byte_array(bytes),
        ..FilesystemOut::value()
    }
}

fn output_metadata(metadata: &cap_std::fs::Metadata) -> FilesystemOut {
    FilesystemOut {
        payload: make_metadata_array(metadata),
        ..FilesystemOut::value()
    }
}

fn metadata_in_resolved(resolved: &ResolvedPath) -> Result<cap_std::fs::Metadata, FsError> {
    let actual = resolved
        .parent
        .symlink_metadata(&resolved.name)
        .map_err(map_io)?;
    #[cfg(unix)]
    if let Some(expected) = &resolved.metadata
        && !same_metadata_snapshot(expected, &actual)
    {
        return Err(FsError::InvalidPath);
    }
    #[cfg(windows)]
    {
        use cap_std::fs::MetadataExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
        if actual.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(FsError::InvalidPath);
        }
    }
    Ok(actual)
}

fn open_read_path(path: &str) -> Result<(ResolvedPath, cap_std::fs::File), FsError> {
    let resolved = resolve_authorized(path, "filesystem.read", false)?;
    let file = open_resolved(&resolved, true, false, false, false)?;
    Ok((resolved, file))
}

fn open_write_path(path: &str, append: bool) -> Result<(ResolvedPath, cap_std::fs::File), FsError> {
    let resolved = resolve_authorized(path, "filesystem.write", true)?;
    let file = open_resolved(&resolved, false, true, append, false)?;
    Ok((resolved, file))
}

fn run_async_fs_job<T: Send + 'static>(
    job: impl FnOnce() -> Result<T, FsError> + Send + 'static,
) -> Result<T, FsError> {
    let requester = crate::permissions::capture_active_requester();
    crate::managed_blocking::run_owned_job(move || {
        crate::permissions::with_active_requester(requester, job)
    })
    .map_err(|error| match error {
        crate::managed_blocking::OwnedJobError::Cancelled => FsError::Cancelled,
        crate::managed_blocking::OwnedJobError::Panicked
        | crate::managed_blocking::OwnedJobError::ShuttingDown => FsError::Io,
        crate::managed_blocking::OwnedJobError::NotInTask => FsError::Unsupported,
    })?
}

fn run_async_fs_job_cancellable<T: Send + 'static>(
    job: impl FnOnce(crate::managed_blocking::OwnedJobCancellation) -> Result<T, FsError>
    + Send
    + 'static,
) -> Result<T, FsError> {
    let requester = crate::permissions::capture_active_requester();
    crate::managed_blocking::run_owned_job_cancellable(move |cancellation| {
        crate::permissions::with_active_requester(requester, || job(cancellation))
    })
    .map_err(|error| match error {
        crate::managed_blocking::OwnedJobError::Cancelled => FsError::Cancelled,
        crate::managed_blocking::OwnedJobError::Panicked
        | crate::managed_blocking::OwnedJobError::ShuttingDown => FsError::Io,
        crate::managed_blocking::OwnedJobError::NotInTask => FsError::Unsupported,
    })?
}

fn open_file_resource(path: &str, flags: u32) -> Result<OpenFile, FsError> {
    validate_open_flags(flags)?;
    let read = flags & FLAG_READ != 0;
    let write = flags & (FLAG_WRITE | FLAG_APPEND) != 0;
    let append = flags & FLAG_APPEND != 0;
    let create_new = flags & FLAG_CREATE_NEW != 0;
    let resolved = resolve_authorized(
        path,
        if read {
            "filesystem.read"
        } else {
            "filesystem.write"
        },
        flags & (FLAG_CREATE | FLAG_CREATE_NEW) != 0,
    )?;
    let scope = path_scope(&resolved.canonical)?;
    if read {
        authorize("filesystem.read", &scope)?;
    }
    if write {
        authorize("filesystem.write", &scope)?;
    }
    let file = open_resolved(&resolved, read, write, append, create_new)?;
    if flags & FLAG_TRUNCATE != 0 {
        authorize("filesystem.write", &scope)?;
        file.set_len(0).map_err(map_io)?;
    }
    Ok(OpenFile {
        file,
        path: scope,
        read,
        write,
        append,
        io_lock: Arc::new(Mutex::new(())),
    })
}

fn snapshot_open_file(
    handle: u64,
) -> Result<(cap_std::fs::File, String, bool, bool, bool, Arc<Mutex<()>>), FsError> {
    let files = files()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let open = files.get(&handle).ok_or(FsError::Closed)?;
    Ok((
        open.file.try_clone().map_err(map_io)?,
        open.path.clone(),
        open.read,
        open.write,
        open.append,
        Arc::clone(&open.io_lock),
    ))
}

fn async_read_open_file(handle: u64, max_bytes: u64) -> Result<Vec<u8>, FsError> {
    if max_bytes > MAX_CHUNK_BYTES {
        return Err(FsError::Limit);
    }
    let (mut file, path, read, _, _, io_lock) = snapshot_open_file(handle)?;
    if !read {
        return Err(FsError::InvalidOptions);
    }
    let _io = io_lock.lock().unwrap_or_else(|error| error.into_inner());
    authorize("filesystem.read", &path)?;
    let mut bytes = vec![0; usize::try_from(max_bytes).map_err(|_| FsError::Limit)?];
    let read = file.read(&mut bytes).map_err(map_io)?;
    bytes.truncate(read);
    Ok(bytes)
}

fn async_read_exact_open_file(handle: u64, count: u64) -> Result<Vec<u8>, FsError> {
    if count > MAX_CHUNK_BYTES {
        return Err(FsError::Limit);
    }
    let (mut file, path, read, _, _, io_lock) = snapshot_open_file(handle)?;
    if !read {
        return Err(FsError::InvalidOptions);
    }
    let _io = io_lock.lock().unwrap_or_else(|error| error.into_inner());
    authorize("filesystem.read", &path)?;
    let mut bytes = vec![0; usize::try_from(count).map_err(|_| FsError::Limit)?];
    file.read_exact(&mut bytes).map_err(|error| {
        if error.kind() == io::ErrorKind::UnexpectedEof {
            FsError::EndOfStream
        } else {
            map_io(error)
        }
    })?;
    Ok(bytes)
}

fn async_write_open_file(handle: u64, bytes: &[u8]) -> Result<u64, FsError> {
    let (mut file, path, _, write, _, io_lock) = snapshot_open_file(handle)?;
    if !write {
        return Err(FsError::InvalidOptions);
    }
    let _io = io_lock.lock().unwrap_or_else(|error| error.into_inner());
    authorize("filesystem.write", &path)?;
    file.write_all(bytes).map_err(map_io)?;
    u64::try_from(bytes.len()).map_err(|_| FsError::Limit)
}

fn async_append_open_file(handle: u64, bytes: &[u8]) -> Result<(), FsError> {
    let (mut file, path, _, write, append, io_lock) = snapshot_open_file(handle)?;
    if !write || !append {
        return Err(FsError::InvalidOptions);
    }
    let _io = io_lock.lock().unwrap_or_else(|error| error.into_inner());
    authorize("filesystem.write", &path)?;
    file.write_all(bytes).map_err(map_io)
}

fn direct_write(path: &str, bytes: &[u8]) -> Result<(), FsError> {
    let resolved = resolve_authorized(path, "filesystem.write", true)?;
    let scope = path_scope(&resolved.canonical)?;
    let mut file = open_resolved(&resolved, false, true, false, false)?;
    authorize("filesystem.write", &scope)?;
    file.set_len(0).map_err(map_io)?;
    authorize("filesystem.write", &scope)?;
    file.write_all(bytes).map_err(map_io)
}

fn append_to_path(path: &str, bytes: &[u8]) -> Result<(), FsError> {
    let (resolved, mut file) = open_write_path(path, true)?;
    let scope = path_scope(&resolved.canonical)?;
    authorize("filesystem.write", &scope)?;
    file.write_all(bytes).map_err(map_io)
}

fn atomic_write(path: &str, bytes: &[u8]) -> Result<(), FsError> {
    let resolved = resolve_authorized(path, "filesystem.write", true)?;
    let scope = path_scope(&resolved.canonical)?;
    let temporary_name = loop {
        let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let candidate = format!(".zirk-tmp-{}-{sequence}", std::process::id());
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use cap_std::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW);
        }
        authorize("filesystem.write", &scope)?;
        match resolved.parent.open_with(&candidate, &options) {
            Ok(mut file) => {
                let write = authorize("filesystem.write", &scope)
                    .and_then(|()| file.write_all(bytes).map_err(map_io))
                    .and_then(|()| authorize("filesystem.write", &scope))
                    .and_then(|()| file.sync_all().map_err(map_io));
                if let Err(error) = write {
                    let _ = resolved.parent.remove_file(&candidate);
                    return Err(error);
                }
                break candidate;
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(map_io(error)),
        }
    };
    if let Err(error) = authorize("filesystem.write", &scope) {
        let _ = resolved.parent.remove_file(&temporary_name);
        return Err(error);
    }
    if let Err(error) = resolved
        .parent
        .rename(&temporary_name, &resolved.parent, &resolved.name)
    {
        let _ = resolved.parent.remove_file(&temporary_name);
        return Err(map_io(error));
    }
    Ok(())
}

unsafe fn argument_string(args: &[i64], index: usize) -> Result<String, FsError> {
    let raw = *args.get(index).ok_or(FsError::InvalidOptions)? as usize as *const c_void;
    let Some(value) = (unsafe { crate::string::borrow(raw) }) else {
        return Err(FsError::InvalidPath);
    };
    Ok(unsafe { value.as_str() }.to_owned())
}

unsafe fn argument_bytes(args: &[i64], index: usize) -> Result<Vec<u8>, FsError> {
    let raw = *args.get(index).ok_or(FsError::InvalidOptions)? as usize as *mut c_void;
    let Some(bytes) =
        (unsafe { crate::array::copy_byte_array(raw, MAX_WHOLE_FILE_BYTES as usize) })
    else {
        return Err(FsError::Limit);
    };
    Ok(bytes)
}

fn argument_u64(args: &[i64], index: usize) -> Result<u64, FsError> {
    Ok(*args.get(index).ok_or(FsError::InvalidOptions)? as u64)
}

fn lexical_path_operation(operation: i32, args: &[i64]) -> Result<FilesystemOut, FsError> {
    // SAFETY: codegen passes live managed String handles for string arguments.
    let left = unsafe { argument_string(args, 0)? };
    let path = HostPath::new(&left);
    match operation {
        PATH_JOIN => {
            let right = unsafe { argument_string(args, 1)? };
            let mut result = PathBuf::from(path);
            result.push(right);
            Ok(output_string(&result.to_string_lossy()))
        }
        PATH_COMPONENTS => {
            let components = path
                .components()
                .map(|component| component.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>();
            Ok(FilesystemOut {
                payload: alloc_string_list(&components),
                ..FilesystemOut::value()
            })
        }
        PATH_PARENT => Ok(path
            .parent()
            .map(|parent| output_string(&parent.to_string_lossy()))
            .unwrap_or_else(FilesystemOut::value)),
        PATH_NAME => Ok(path
            .file_name()
            .map(|name| output_string(&name.to_string_lossy()))
            .unwrap_or_else(FilesystemOut::value)),
        PATH_NORMALIZE => {
            let result = normalize_host_path(path);
            Ok(output_string(&result.to_string_lossy()))
        }
        _ => Err(FsError::Unsupported),
    }
}

fn file_metadata_from_path(path: &str) -> Result<FilesystemOut, FsError> {
    let resolved = resolve_authorized(path, "filesystem.metadata", false)?;
    let metadata = metadata_in_resolved(&resolved)?;
    Ok(output_metadata(&metadata))
}

fn file_exists(path: &str) -> Result<FilesystemOut, FsError> {
    let resolved = match resolve_path(path, true) {
        Ok(resolved) => {
            let scope = path_scope(&resolved.canonical)?;
            authorize("filesystem.metadata", &scope)?;
            resolved
        }
        Err(FsError::NotFound) => {
            let requested = lexical_absolute(HostPath::new(path))?;
            let scope = path_scope(&requested)?;
            authorize("filesystem.metadata", &scope)?;
            return Ok(FilesystemOut {
                scalar: 0,
                ..FilesystemOut::value()
            });
        }
        Err(error) => return Err(error),
    };
    let exists = match metadata_in_resolved(&resolved) {
        Ok(_) => true,
        Err(FsError::NotFound) => false,
        Err(error) => return Err(error),
    };
    Ok(FilesystemOut {
        scalar: u64::from(exists),
        ..FilesystemOut::value()
    })
}

fn path_host_operation(operation: i32, args: &[i64]) -> Result<FilesystemOut, FsError> {
    let left = unsafe { argument_string(args, 0)? };
    match operation {
        PATH_ABSOLUTE => {
            let absolute = lexical_absolute(HostPath::new(&left))?;
            let resource = path_scope(&absolute)?;
            authorize("filesystem.read", &resource)?;
            Ok(output_string(&resource))
        }
        PATH_CANONICALIZE => {
            let resolved = resolve_authorized(&left, "filesystem.metadata", false)?;
            let resource = path_scope(&resolved.canonical)?;
            Ok(output_string(&resource))
        }
        _ => Err(FsError::Unsupported),
    }
}

unsafe fn execute(operation: i32, args: &[i64]) -> Result<FilesystemOut, FsError> {
    if (PATH_JOIN..=PATH_NORMALIZE).contains(&operation) {
        return lexical_path_operation(operation, args);
    }
    if (PATH_ABSOLUTE..=PATH_CANONICALIZE).contains(&operation) {
        return path_host_operation(operation, args);
    }
    match operation {
        DIRECTORY_CREATE => {
            let path = unsafe { argument_string(args, 0)? };
            let resolved = resolve_target_entry(&path)?;
            let resource = path_scope(&resolved.canonical)?;
            authorize_operation(
                "filesystem.write",
                &resource,
                "directory.create",
                FilesystemScopeExtent::Directory,
            )?;
            resolved.parent.create_dir(&resolved.name).map_err(map_io)?;
            Ok(FilesystemOut::value())
        }
        DIRECTORY_CREATE_ALL => {
            let path = unsafe { argument_string(args, 0)? };
            create_directories(&path)?;
            Ok(FilesystemOut::value())
        }
        DIRECTORY_ENTRIES_OPEN => {
            let path = unsafe { argument_string(args, 0)? };
            let (root, directory) = open_authorized_directory(
                &path,
                "filesystem.read",
                "directory.entries",
                FilesystemScopeExtent::Directory,
            )?;
            let entries = directory.entries().map_err(map_io)?;
            let handle = register_directory_reader(root, "directory.entries", entries)?;
            Ok(FilesystemOut {
                scalar: handle,
                ..FilesystemOut::value()
            })
        }
        DIRECTORY_ENTRIES_NEXT | DIRECTORY_ENTRIES_CLOSE => directory_entry(operation, args),
        DIRECTORY_WALK_OPEN => {
            let path = unsafe { argument_string(args, 0)? };
            let max_depth = u32::try_from(argument_u64(args, 1)?).map_err(|_| FsError::Limit)?;
            let max_entries = argument_u64(args, 2)?;
            let (root, directory) = open_authorized_directory(
                &path,
                "filesystem.read",
                "directory.walk",
                FilesystemScopeExtent::Descendants,
            )?;
            let handle = register_directory_walker(root, directory, max_depth, max_entries)?;
            Ok(FilesystemOut {
                scalar: handle,
                ..FilesystemOut::value()
            })
        }
        DIRECTORY_WALK_NEXT | DIRECTORY_WALK_CLOSE => directory_walk_entry(operation, args),
        DIRECTORY_IS_EMPTY => {
            let path = unsafe { argument_string(args, 0)? };
            let (_root, directory) = open_authorized_directory(
                &path,
                "filesystem.read",
                "directory.is_empty",
                FilesystemScopeExtent::Directory,
            )?;
            let mut entries = directory.entries().map_err(map_io)?;
            let empty = match entries.next() {
                None => true,
                Some(Ok(_)) => false,
                Some(Err(error)) => return Err(map_io(error)),
            };
            Ok(FilesystemOut {
                scalar: u64::from(empty),
                ..FilesystemOut::value()
            })
        }
        DIRECTORY_SIZE => {
            let path = unsafe { argument_string(args, 0)? };
            let max_depth = u32::try_from(argument_u64(args, 1)?).map_err(|_| FsError::Limit)?;
            let max_entries = argument_u64(args, 2)?;
            let max_bytes = argument_u64(args, 3)?;
            let size = directory_size(&path, max_depth, max_entries, max_bytes)?;
            Ok(FilesystemOut {
                scalar: size,
                ..FilesystemOut::value()
            })
        }
        DIRECTORY_COPY => {
            let source = unsafe { argument_string(args, 0)? };
            let destination = unsafe { argument_string(args, 1)? };
            let replace = argument_u64(args, 2)? != 0;
            let max_depth = u32::try_from(argument_u64(args, 3)?).map_err(|_| FsError::Limit)?;
            let max_entries = argument_u64(args, 4)?;
            copy_directory(&source, &destination, replace, max_depth, max_entries)?;
            Ok(FilesystemOut::value())
        }
        DIRECTORY_MOVE => {
            let source = unsafe { argument_string(args, 0)? };
            let destination = unsafe { argument_string(args, 1)? };
            let replace = argument_u64(args, 2)? != 0;
            move_directory_entry(&source, &destination, replace)?;
            Ok(FilesystemOut::value())
        }
        DIRECTORY_WATCH => directory_watch(args),
        WATCHER_NEXT => watcher_next(args),
        WATCHER_NEXT_ASYNC => watcher_next_async(args),
        WATCHER_CLOSE => watcher_close(args),
        LINK_IS_LINK => {
            let path = unsafe { argument_string(args, 0)? };
            link_is_link(&path)
        }
        LINK_READ_TARGET => {
            let path = unsafe { argument_string(args, 0)? };
            link_read_target(&path)
        }
        LINK_CREATE => {
            let target = unsafe { argument_string(args, 0)? };
            let destination = unsafe { argument_string(args, 1)? };
            let kind = argument_u64(args, 2)?;
            link_create(&target, &destination, kind)?;
            Ok(FilesystemOut::value())
        }
        TEMP_CREATE_FILE | TEMP_CREATE_DIRECTORY => {
            let parent = unsafe { argument_string(args, 0)? };
            temporary_create(&parent, operation == TEMP_CREATE_DIRECTORY)
        }
        TEMP_PATH => temporary_path(argument_u64(args, 0)?),
        TEMP_CLEANUP => {
            temporary_cleanup(argument_u64(args, 0)?)?;
            Ok(FilesystemOut::value())
        }
        TEMP_RELEASE => {
            temporary_release(argument_u64(args, 0)?)?;
            Ok(FilesystemOut::value())
        }
        FILE_LOCK => {
            let file_handle = argument_u64(args, 0)?;
            let exclusive = argument_u64(args, 1)? != 0;
            let nonblocking = argument_u64(args, 2)? != 0;
            acquire_file_lock(file_handle, exclusive, nonblocking)
        }
        FILE_UNLOCK => {
            release_file_lock(argument_u64(args, 0)?)?;
            Ok(FilesystemOut::value())
        }
        DIRECTORY_REMOVE => {
            let path = unsafe { argument_string(args, 0)? };
            remove_one(&path)?;
            Ok(FilesystemOut::value())
        }
        DIRECTORY_REMOVE_ALL => {
            let path = unsafe { argument_string(args, 0)? };
            let max_depth = u32::try_from(argument_u64(args, 1)?).map_err(|_| FsError::Limit)?;
            let max_entries = argument_u64(args, 2)?;
            remove_all(&path, max_depth, max_entries)?;
            Ok(FilesystemOut::value())
        }
        FILE_SET_READONLY => {
            let path = unsafe { argument_string(args, 0)? };
            let readonly = argument_u64(args, 1)? != 0;
            let mut resolved = resolve_target_entry(&path)?;
            let scope = path_scope(&resolved.canonical)?;
            authorize_operation(
                "filesystem.write",
                &scope,
                "file.set_readonly",
                FilesystemScopeExtent::Exact,
            )?;
            resolved.metadata =
                Some(std::fs::symlink_metadata(&resolved.canonical).map_err(map_io)?);
            let file = open_resolved(&resolved, true, false, false, false)?;
            let mut permissions = file.metadata().map_err(map_io)?.permissions();
            permissions.set_readonly(readonly);
            authorize_operation(
                "filesystem.write",
                &scope,
                "file.set_readonly",
                FilesystemScopeExtent::Exact,
            )?;
            drop(file);
            resolved
                .parent
                .set_permissions(&resolved.name, permissions)
                .map_err(map_io)?;
            Ok(FilesystemOut::value())
        }
        FILE_READ_BYTES_ASYNC | FILE_READ_TEXT_ASYNC => {
            let path = unsafe { argument_string(args, 0)? };
            let limit = argument_u64(args, 1)?;
            let encoding = if operation == FILE_READ_TEXT_ASYNC {
                argument_u64(args, 2)?
            } else {
                0
            };
            let output = run_async_fs_job(move || {
                let (resolved, mut file) = open_read_path(&path)?;
                let scope = path_scope(&resolved.canonical)?;
                let bytes = read_bounded_authorized(&mut file, limit, || {
                    authorize("filesystem.read", &scope)
                })?;
                if operation == FILE_READ_BYTES_ASYNC {
                    Ok(AsyncFileValue::Bytes(bytes))
                } else {
                    Ok(AsyncFileValue::Text(decode_text(&bytes, encoding)?))
                }
            })?;
            match output {
                AsyncFileValue::Bytes(bytes) => Ok(output_bytes(&bytes)),
                AsyncFileValue::Text(text) => Ok(output_string(&text)),
            }
        }
        FILE_WRITE_BYTES_ASYNC | FILE_WRITE_TEXT_ASYNC => {
            let path = unsafe { argument_string(args, 0)? };
            let bytes = if operation == FILE_WRITE_BYTES_ASYNC {
                unsafe { argument_bytes(args, 1)? }
            } else {
                let text = unsafe { argument_string(args, 1)? };
                encode_text(&text, argument_u64(args, 2)?)?
            };
            run_async_fs_job(move || direct_write(&path, &bytes))?;
            Ok(FilesystemOut::value())
        }
        FILE_OPEN_ASYNC => {
            let path = unsafe { argument_string(args, 0)? };
            let flags =
                u32::try_from(argument_u64(args, 1)?).map_err(|_| FsError::InvalidOptions)?;
            let reservation = OpenFileReservation::reserve()?;
            let open = run_async_fs_job(move || open_file_resource(&path, flags))?;
            let mut open_files = files()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let handle = loop {
                let candidate = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed).max(1);
                if !open_files.contains_key(&candidate) {
                    break candidate;
                }
            };
            open_files.insert(handle, open);
            drop(open_files);
            reservation.commit();
            Ok(FilesystemOut {
                scalar: handle,
                ..FilesystemOut::value()
            })
        }
        FILE_READ_HANDLE_ASYNC => {
            let handle = argument_u64(args, 0)?;
            let max_bytes = argument_u64(args, 1)?;
            let bytes = run_async_fs_job(move || async_read_open_file(handle, max_bytes))?;
            Ok(output_bytes(&bytes))
        }
        FILE_WRITE_HANDLE_ASYNC => {
            let handle = argument_u64(args, 0)?;
            let bytes = unsafe { argument_bytes(args, 1)? };
            let written = run_async_fs_job(move || async_write_open_file(handle, &bytes))?;
            Ok(FilesystemOut {
                scalar: written,
                ..FilesystemOut::value()
            })
        }
        FILE_READ_EXACT_ASYNC => {
            let handle = argument_u64(args, 0)?;
            let count = argument_u64(args, 1)?;
            let bytes = run_async_fs_job(move || async_read_exact_open_file(handle, count))?;
            Ok(output_bytes(&bytes))
        }
        FILE_APPEND_BYTES_ASYNC | FILE_APPEND_TEXT_ASYNC => {
            let path = unsafe { argument_string(args, 0)? };
            let bytes = if operation == FILE_APPEND_BYTES_ASYNC {
                unsafe { argument_bytes(args, 1)? }
            } else {
                let text = unsafe { argument_string(args, 1)? };
                encode_text(&text, argument_u64(args, 2)?)?
            };
            run_async_fs_job(move || append_to_path(&path, &bytes))?;
            Ok(FilesystemOut::value())
        }
        FILE_APPEND_HANDLE_ASYNC => {
            let handle = argument_u64(args, 0)?;
            let bytes = unsafe { argument_bytes(args, 1)? };
            run_async_fs_job(move || async_append_open_file(handle, &bytes))?;
            Ok(FilesystemOut::value())
        }
        FILE_LOCK_ASYNC => {
            let file_handle = argument_u64(args, 0)?;
            let exclusive = argument_u64(args, 1)? != 0;
            let acquired = run_async_fs_job_cancellable(move |cancellation| {
                acquire_file_lock_resource(file_handle, exclusive, false, Some(cancellation))
            })?;
            let handle = register_acquired_file_lock(acquired);
            Ok(FilesystemOut {
                scalar: handle,
                ..FilesystemOut::value()
            })
        }
        DIRECTORY_ENTRIES_OPEN_ASYNC => {
            let path = unsafe { argument_string(args, 0)? };
            let reader = run_async_fs_job(move || {
                let (root, directory) = open_authorized_directory(
                    &path,
                    "filesystem.read",
                    "directory.entries",
                    FilesystemScopeExtent::Directory,
                )?;
                Ok(DirectoryReader {
                    path: root,
                    operation: "directory.entries",
                    entries: directory.entries().map_err(map_io)?,
                })
            })?;
            let handle = register_directory_reader_resource(reader)?;
            Ok(FilesystemOut {
                scalar: handle,
                ..FilesystemOut::value()
            })
        }
        DIRECTORY_ENTRIES_NEXT_ASYNC => {
            let handle = argument_u64(args, 0)?;
            let value = run_async_fs_job(move || directory_entry_value(handle))?;
            Ok(output_string(&value))
        }
        DIRECTORY_WALK_OPEN_ASYNC => {
            let path = unsafe { argument_string(args, 0)? };
            let max_depth = u32::try_from(argument_u64(args, 1)?).map_err(|_| FsError::Limit)?;
            let max_entries = argument_u64(args, 2)?;
            let walker = run_async_fs_job(move || {
                let (root, directory) = open_authorized_directory(
                    &path,
                    "filesystem.read",
                    "directory.walk",
                    FilesystemScopeExtent::Descendants,
                )?;
                open_directory_walker(root, directory, max_depth, max_entries)
            })?;
            let handle = register_directory_walker_resource(walker)?;
            Ok(FilesystemOut {
                scalar: handle,
                ..FilesystemOut::value()
            })
        }
        DIRECTORY_WALK_NEXT_ASYNC => {
            let handle = argument_u64(args, 0)?;
            let value = run_async_fs_job(move || directory_walk_next_value(handle))?;
            Ok(output_string(&value))
        }
        FILE_READ_BYTES | FILE_READ_TEXT => {
            let path = unsafe { argument_string(args, 0)? };
            let limit = argument_u64(args, 1)?;
            let encoding = if operation == FILE_READ_TEXT {
                argument_u64(args, 2)?
            } else {
                0
            };
            let (resolved, mut file) = open_read_path(&path)?;
            let scope = path_scope(&resolved.canonical)?;
            let bytes =
                read_bounded_authorized(&mut file, limit, || authorize("filesystem.read", &scope))?;
            if operation == FILE_READ_BYTES {
                Ok(output_bytes(&bytes))
            } else {
                Ok(output_string(&decode_text(&bytes, encoding)?))
            }
        }
        FILE_WRITE_BYTES | FILE_WRITE_TEXT => {
            let path = unsafe { argument_string(args, 0)? };
            let bytes = if operation == FILE_WRITE_BYTES {
                unsafe { argument_bytes(args, 1)? }
            } else {
                let text = unsafe { argument_string(args, 1)? };
                encode_text(&text, argument_u64(args, 2)?)?
            };
            direct_write(&path, &bytes)?;
            Ok(FilesystemOut::value())
        }
        FILE_OPEN => {
            let path = unsafe { argument_string(args, 0)? };
            let flags =
                u32::try_from(argument_u64(args, 1)?).map_err(|_| FsError::InvalidOptions)?;
            validate_open_flags(flags)?;
            // Reserve capacity before path resolution or a possible consent
            // prompt so a saturated process does not request authority for
            // an operation that cannot acquire a managed handle.
            let reservation = OpenFileReservation::reserve()?;
            let read = flags & FLAG_READ != 0;
            let write = flags & (FLAG_WRITE | FLAG_APPEND) != 0;
            let append = flags & FLAG_APPEND != 0;
            let create_new = flags & FLAG_CREATE_NEW != 0;
            let resolved = resolve_authorized(
                &path,
                if read {
                    "filesystem.read"
                } else {
                    "filesystem.write"
                },
                flags & (FLAG_CREATE | FLAG_CREATE_NEW) != 0,
            )?;
            let scope = path_scope(&resolved.canonical)?;
            if read {
                authorize("filesystem.read", &scope)?;
            }
            if write {
                authorize("filesystem.write", &scope)?;
            }
            let file = open_resolved(&resolved, read, write, append, create_new)?;
            if flags & FLAG_TRUNCATE != 0 {
                file.set_len(0).map_err(map_io)?;
            }
            let mut open_files = files()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let handle = loop {
                let candidate = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed).max(1);
                if !open_files.contains_key(&candidate) {
                    break candidate;
                }
            };
            open_files.insert(
                handle,
                OpenFile {
                    file,
                    path: scope,
                    read,
                    write,
                    append,
                    io_lock: Arc::new(Mutex::new(())),
                },
            );
            drop(open_files);
            reservation.commit();
            Ok(FilesystemOut {
                scalar: handle,
                ..FilesystemOut::value()
            })
        }
        FILE_CLOSE => {
            let handle = argument_u64(args, 0)?;
            let removed = files()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .remove(&handle);
            if removed.is_some() {
                release_file_slot(&OPEN_FILE_COUNT);
            }
            Ok(FilesystemOut::value())
        }
        FILE_IS_CLOSED => {
            let handle = argument_u64(args, 0)?;
            let closed = !files()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .contains_key(&handle);
            Ok(FilesystemOut {
                scalar: u64::from(closed),
                ..FilesystemOut::value()
            })
        }
        FILE_READ_HANDLE => {
            let handle = argument_u64(args, 0)?;
            let max_bytes = argument_u64(args, 1)?;
            let mut files = files()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let open = files.get_mut(&handle).ok_or(FsError::Closed)?;
            let io_lock = Arc::clone(&open.io_lock);
            let _io = io_lock.lock().unwrap_or_else(|error| error.into_inner());
            let bytes = read_open_file(open, max_bytes, authorize)?;
            Ok(output_bytes(&bytes))
        }
        FILE_READ_EXACT => {
            let handle = argument_u64(args, 0)?;
            let count = argument_u64(args, 1)?;
            if count > MAX_CHUNK_BYTES {
                return Err(FsError::Limit);
            }
            let mut files = files()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let open = files.get_mut(&handle).ok_or(FsError::Closed)?;
            if !open.read {
                return Err(FsError::InvalidOptions);
            }
            let io_lock = Arc::clone(&open.io_lock);
            let _io = io_lock.lock().unwrap_or_else(|error| error.into_inner());
            authorize("filesystem.read", &open.path)?;
            let mut bytes = vec![0; usize::try_from(count).map_err(|_| FsError::Limit)?];
            match open.file.read_exact(&mut bytes) {
                Ok(()) => Ok(output_bytes(&bytes)),
                Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => {
                    Err(FsError::EndOfStream)
                }
                Err(error) => Err(map_io(error)),
            }
        }
        FILE_READ_LINE => {
            let handle = argument_u64(args, 0)?;
            let mut files = files()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let open = files.get_mut(&handle).ok_or(FsError::Closed)?;
            if !open.read {
                return Err(FsError::InvalidOptions);
            }
            let io_lock = Arc::clone(&open.io_lock);
            let _io = io_lock.lock().unwrap_or_else(|error| error.into_inner());
            authorize("filesystem.read", &open.path)?;
            let mut bytes = Vec::new();
            loop {
                let mut byte = [0u8; 1];
                match open.file.read(&mut byte) {
                    Ok(0) if bytes.is_empty() => {
                        return Ok(FilesystemOut {
                            kind: 2,
                            error: 0,
                            scalar: 0,
                            payload: std::ptr::null_mut(),
                        });
                    }
                    Ok(0) => break,
                    Ok(_) if byte[0] == b'\n' => break,
                    Ok(_) => {
                        bytes.push(byte[0]);
                        if bytes.len() > MAX_LINE_BYTES {
                            return Err(FsError::Limit);
                        }
                    }
                    Err(error) => return Err(map_io(error)),
                }
            }
            if bytes.last() == Some(&b'\r') {
                bytes.pop();
            }
            Ok(output_string(&decode_text(&bytes, 0)?))
        }
        FILE_APPEND_BYTES | FILE_APPEND_TEXT => {
            let path = unsafe { argument_string(args, 0)? };
            let bytes = if operation == FILE_APPEND_BYTES {
                unsafe { argument_bytes(args, 1)? }
            } else {
                let text = unsafe { argument_string(args, 1)? };
                encode_text(&text, argument_u64(args, 2)?)?
            };
            append_to_path(&path, &bytes)?;
            Ok(FilesystemOut::value())
        }
        FILE_APPEND_HANDLE => {
            let handle = argument_u64(args, 0)?;
            let bytes = unsafe { argument_bytes(args, 1)? };
            let mut files = files()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let open = files.get_mut(&handle).ok_or(FsError::Closed)?;
            if !open.write || !open.append {
                return Err(FsError::InvalidOptions);
            }
            let io_lock = Arc::clone(&open.io_lock);
            let _io = io_lock.lock().unwrap_or_else(|error| error.into_inner());
            authorize("filesystem.write", &open.path)?;
            open.file.write_all(&bytes).map_err(map_io)?;
            Ok(FilesystemOut::value())
        }
        FILE_METADATA_HANDLE => {
            let handle = argument_u64(args, 0)?;
            let files = files()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let open = files.get(&handle).ok_or(FsError::Closed)?;
            authorize("filesystem.metadata", &open.path)?;
            let metadata = open.file.metadata().map_err(map_io)?;
            Ok(output_metadata(&metadata))
        }
        FILE_EXISTS => {
            let path = unsafe { argument_string(args, 0)? };
            file_exists(&path)
        }
        FILE_METADATA_PATH => {
            let path = unsafe { argument_string(args, 0)? };
            file_metadata_from_path(&path)
        }
        FILE_WRITE_BYTES_ATOMIC | FILE_WRITE_TEXT_ATOMIC => {
            let path = unsafe { argument_string(args, 0)? };
            let bytes = if operation == FILE_WRITE_BYTES_ATOMIC {
                unsafe { argument_bytes(args, 1)? }
            } else {
                let text = unsafe { argument_string(args, 1)? };
                encode_text(&text, 0)?
            };
            atomic_write(&path, &bytes)?;
            Ok(FilesystemOut::value())
        }
        _ => Err(FsError::Unsupported),
    }
}

/// Compiler-owned entry point for the closed `std.fs`/`std.path` binding set.
/// Arguments are native machine words whose interpretation is fixed by the
/// operation id; the compiler validates each public signature before emission.
///
/// # Safety
/// `args` must point to `arg_count` live words when `arg_count` is nonzero and
/// `out` must point to writable `FilesystemOut` storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_fs_call(
    operation: i32,
    args: *const i64,
    arg_count: u64,
    out: *mut FilesystemOut,
) {
    if out.is_null() {
        return;
    }
    let count = match usize::try_from(arg_count) {
        Ok(count) if count <= 8 => count,
        _ => {
            unsafe { *out = FilesystemOut::error(FsError::InvalidOptions, false) };
            return;
        }
    };
    if count > 0 && args.is_null() {
        unsafe { *out = FilesystemOut::error(FsError::InvalidOptions, false) };
        return;
    }
    let args = if count == 0 {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(args, count) }
    };
    let path_error = matches!(operation, PATH_ABSOLUTE | PATH_CANONICALIZE);
    let result = std::panic::catch_unwind(|| unsafe { execute(operation, args) });
    let outcome = match result {
        Ok(Ok(outcome)) => outcome,
        Ok(Err(error)) => FilesystemOut::error(error, path_error),
        Err(_) => FilesystemOut::error(FsError::Io, path_error),
    };
    unsafe { *out = outcome };
}

fn alloc_string_list(values: &[String]) -> *mut c_void {
    let element_size = std::mem::size_of::<*mut c_void>();
    let element_align = std::mem::align_of::<*mut c_void>();
    let list = unsafe { crate::list::zirk_rt_list_new(element_size, element_align, true) };
    let mut rooted_list = list;
    let mut current = std::ptr::null_mut();
    let mut roots = [
        (&mut rooted_list as *mut *mut c_void).cast(),
        (&mut current as *mut *mut c_void).cast(),
    ];
    unsafe { crate::zirk_rt_push_frame(roots.as_mut_ptr(), roots.len() as i64) };
    for value in values {
        current = crate::string::alloc_owned(value);
        unsafe {
            crate::list::zirk_rt_list_add(
                rooted_list,
                (&current as *const *mut c_void).cast(),
                element_size,
                element_align,
                true,
            )
        };
    }
    crate::zirk_rt_pop_frame();
    rooted_list
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn scratch(tag: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("zirk-fs-{}-{tag}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn lexical_normalization_does_not_access_the_filesystem() {
        assert_eq!(
            normalize_host_path(HostPath::new("data/../config.json")),
            PathBuf::from("config.json")
        );
        assert_eq!(
            normalize_host_path(HostPath::new("../../data/./x")),
            PathBuf::from("../../data/x")
        );
    }

    #[test]
    fn absolute_path_rejects_empty_and_nul_containing_inputs() {
        assert_eq!(
            lexical_absolute(HostPath::new("")),
            Err(FsError::InvalidPath)
        );
        assert_eq!(
            lexical_absolute(HostPath::new("bad\0path")),
            Err(FsError::InvalidPath)
        );
    }

    #[test]
    fn missing_paths_remain_distinct_from_invalid_paths() {
        let directory = scratch("missing-path");
        let target = directory.join("missing");
        assert_eq!(
            resolve_path(target.to_str().unwrap(), false).err(),
            Some(FsError::NotFound)
        );
    }

    #[cfg(unix)]
    #[test]
    fn temporary_cleanup_refuses_to_delete_a_replacement_entry() {
        let directory = scratch("temporary-replacement");
        let parent = Dir::open_ambient_dir(&directory, ambient_authority()).unwrap();
        let name = std::ffi::OsStr::new("owned.tmp");
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        let original = parent.open_with(name, &options).unwrap();
        let identity = temporary_identity(&original.metadata().unwrap()).unwrap();
        parent.remove_file(name).unwrap();
        fs::write(directory.join(name), "replacement").unwrap();

        let mut authorize_remove_called = false;
        assert_eq!(
            remove_temporary_file_if_owned(&parent, name, identity, || {
                authorize_remove_called = true;
                Ok(())
            }),
            Err(FsError::InvalidPath)
        );
        assert!(!authorize_remove_called);
        assert_eq!(
            fs::read_to_string(directory.join(name)).unwrap(),
            "replacement"
        );
    }

    #[cfg(unix)]
    #[test]
    fn target_resolution_preserves_the_final_symlink_for_nofollow_checks() {
        use std::os::unix::fs::symlink;

        let directory = scratch("directory-final-symlink");
        let target = directory.join("target");
        let link = directory.join("link");
        fs::create_dir(&target).unwrap();
        symlink(&target, &link).unwrap();

        let resolved = resolve_target_entry(link.to_str().unwrap()).unwrap();
        assert_eq!(
            resolved.canonical,
            fs::canonicalize(&directory).unwrap().join("link")
        );
        assert!(
            resolved
                .parent
                .symlink_metadata(&resolved.name)
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }

    #[test]
    fn cross_device_move_errors_are_reported_as_cross_device() {
        #[cfg(unix)]
        let error = io::Error::from_raw_os_error(libc::EXDEV);
        #[cfg(windows)]
        let error = io::Error::from_raw_os_error(17);
        assert_eq!(map_io(error), FsError::CrossDevice);
    }

    #[test]
    fn file_error_ordinals_keep_cancelled_after_existing_variants() {
        assert_eq!(FsError::NotEmpty.file_discriminant(), 14);
        assert_eq!(FsError::Cancelled.file_discriminant(), 15);
    }

    #[test]
    fn watcher_event_kinds_have_stable_best_effort_mappings() {
        use notify::event::{CreateKind, DataChange, ModifyKind, RemoveKind, RenameMode};

        assert_eq!(
            watcher_event_marker(&notify::EventKind::Create(CreateKind::File)),
            Some('c')
        );
        assert_eq!(
            watcher_event_marker(&notify::EventKind::Modify(ModifyKind::Data(
                DataChange::Any
            ))),
            Some('m')
        );
        assert_eq!(
            watcher_event_marker(&notify::EventKind::Remove(RemoveKind::File)),
            Some('d')
        );
        assert_eq!(
            watcher_event_marker(&notify::EventKind::Modify(ModifyKind::Name(
                RenameMode::Both
            ))),
            Some('r')
        );
        assert_eq!(
            watcher_event_marker(&notify::EventKind::Access(notify::event::AccessKind::Any)),
            None
        );
        assert_eq!(
            encode_watcher_record(watcher_record_from_kind('c', "new.txt".into())),
            "cnew.txt"
        );
        assert_eq!(
            encode_watcher_record(watcher_record_from_kind('m', "changed.txt".into())),
            "mchanged.txt"
        );
        assert_eq!(
            encode_watcher_record(watcher_record_from_kind('d', "removed.txt".into())),
            "dremoved.txt"
        );
        assert_eq!(
            encode_watcher_record(watcher_record_from_kind('r', "renamed.txt".into())),
            "rrenamed.txt"
        );
        assert_eq!(encode_watcher_record(WatcherRecord::Closed), "x");
        assert_eq!(
            encode_watcher_record(WatcherRecord::Error(FsError::PermissionDenied)),
            "D"
        );
        assert!(matches!(
            watcher_record_from_notify_error(notify::Error::watch_not_found()),
            WatcherRecord::Closed
        ));
        assert!(matches!(
            watcher_record_from_notify_error(notify::Error::path_not_found()),
            WatcherRecord::Error(FsError::NotFound)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn native_notify_backend_registers_nonrecursive_directory_watch() {
        let root = scratch("native-watcher-registration");
        let mut watcher = notify::recommended_watcher(|_: notify::Result<notify::Event>| {})
            .expect("construct native filesystem watcher");
        watcher
            .watch(&root, notify::RecursiveMode::NonRecursive)
            .expect("register native directory watch");
    }

    #[test]
    fn full_watcher_queue_reports_overflow_instead_of_claiming_complete_history() {
        let (sender, receiver) = mpsc::sync_channel(1);
        let overflow = AtomicBool::new(false);
        assert!(
            enqueue_watcher_record(
                &sender,
                &overflow,
                WatcherRecord::Created("first.txt".into())
            )
            .is_ok()
        );
        assert!(
            enqueue_watcher_record(
                &sender,
                &overflow,
                WatcherRecord::Removed("dropped.txt".into())
            )
            .is_err()
        );
        assert!(overflow.swap(false, Ordering::AcqRel));
        assert!(matches!(
            receiver.try_recv(),
            Ok(WatcherRecord::Created(path)) if path == "first.txt"
        ));
        assert!(matches!(receiver.try_recv(), Err(TryRecvError::Empty)));
    }

    #[cfg(unix)]
    #[test]
    fn watcher_event_paths_stay_inside_root_and_suppress_symlink_target_changes() {
        use std::os::unix::fs::symlink;

        let root = fs::canonicalize(scratch("watcher-path-containment")).unwrap();
        let nested = root.join("nested");
        let outside = root
            .parent()
            .unwrap()
            .join(format!("zirk-fs-outside-{}", std::process::id()));
        fs::create_dir(&nested).unwrap();
        fs::create_dir_all(&outside).unwrap();
        let link = nested.join("external");
        symlink(&outside, &link).unwrap();

        assert_eq!(
            watcher_event_path(&root, &nested.join("new.txt"), 'c').as_deref(),
            Some("nested/new.txt")
        );
        assert_eq!(
            watcher_event_path(&root, &outside.join("secret.txt"), 'c'),
            None
        );
        assert_eq!(watcher_event_path(&root, &link, 'm'), None);
        assert_eq!(
            watcher_event_path(&root, &link, 'c').as_deref(),
            Some("nested/external")
        );
        let _ = fs::remove_dir_all(outside);
    }

    #[cfg(windows)]
    #[test]
    fn windows_watchers_and_sync_locks_are_typed_unsupported() {
        assert!(matches!(directory_watch(&[]), Err(FsError::Unsupported)));
        assert!(matches!(
            acquire_file_lock(0, false, true),
            Err(FsError::Unsupported)
        ));
    }

    #[test]
    fn directory_walk_rejects_unbounded_limits() {
        let directory = fs::canonicalize(scratch("directory-walk-bounds")).unwrap();
        let opened = open_directory_chain(&directory).unwrap();
        assert_eq!(
            register_directory_walker(
                directory.to_str().unwrap().to_owned(),
                opened,
                MAX_DIRECTORY_DEPTH + 1,
                1,
            ),
            Err(FsError::Limit)
        );
    }

    #[cfg(unix)]
    #[test]
    fn native_paths_without_zirk_string_representation_fail_conversion() {
        use std::os::unix::ffi::OsStrExt;

        let non_unicode = HostPath::new(std::ffi::OsStr::from_bytes(b"bad-\xff-path"));
        assert_eq!(path_text(non_unicode), Err(FsError::InvalidPath));
    }

    #[test]
    fn open_options_reject_empty_and_conflicting_modes() {
        assert_eq!(validate_open_flags(0), Err(FsError::InvalidOptions));
        assert_eq!(
            validate_open_flags(FLAG_WRITE | FLAG_APPEND | FLAG_TRUNCATE),
            Err(FsError::InvalidOptions)
        );
        assert_eq!(
            validate_open_flags(FLAG_WRITE | FLAG_CREATE | FLAG_CREATE_NEW),
            Err(FsError::InvalidOptions)
        );
        assert_eq!(validate_open_flags(FLAG_READ), Ok(()));
        assert_eq!(validate_open_flags(FLAG_WRITE | FLAG_CREATE_NEW), Ok(()));
    }

    #[test]
    fn create_new_is_atomic_when_competing_callers_open_the_same_path() {
        let directory = scratch("create-new-race");
        let target = directory.join("target");
        let barrier = std::sync::Barrier::new(2);
        let successes = std::thread::scope(|scope| {
            let mut joins = Vec::new();
            for _ in 0..2 {
                let path = target.clone();
                let barrier = &barrier;
                joins.push(scope.spawn(move || {
                    barrier.wait();
                    let resolved = resolve_path(path.to_str().unwrap(), true).unwrap();
                    open_resolved(&resolved, false, true, false, true).is_ok()
                }));
            }
            joins
                .into_iter()
                .map(|join| join.join().unwrap())
                .filter(|success| *success)
                .count()
        });
        assert_eq!(successes, 1);
        assert!(target.is_file());
    }

    #[cfg(unix)]
    #[test]
    fn opening_a_replaced_symlink_never_reads_or_modifies_its_target() {
        use std::os::unix::fs::symlink;

        let directory = scratch("symlink-race");
        let victim = directory.join("victim");
        let outside = directory.join("outside");
        fs::write(&victim, b"original").unwrap();
        fs::write(&outside, b"keep me safe").unwrap();

        let resolved = resolve_path(victim.to_str().unwrap(), false).unwrap();
        fs::remove_file(&victim).unwrap();
        symlink(&outside, &victim).unwrap();

        assert!(open_resolved(&resolved, false, true, false, false).is_err());
        assert_eq!(fs::read(&outside).unwrap(), b"keep me safe");
    }

    #[cfg(unix)]
    #[test]
    fn replacing_a_symlinked_parent_after_resolution_does_not_redirect_the_open() {
        use std::os::unix::fs::symlink;

        let directory = scratch("parent-symlink-race");
        let approved = directory.join("approved");
        let outside = directory.join("outside");
        let link = directory.join("link");
        fs::create_dir(&approved).unwrap();
        fs::create_dir(&outside).unwrap();
        let approved_file = approved.join("target");
        let outside_file = outside.join("target");
        fs::write(&approved_file, b"approved").unwrap();
        fs::write(&outside_file, b"attacker").unwrap();
        symlink(&approved, &link).unwrap();

        let resolved = resolve_path(link.join("target").to_str().unwrap(), false).unwrap();
        fs::remove_file(&link).unwrap();
        symlink(&outside, &link).unwrap();

        let mut file = open_resolved(&resolved, true, false, false, false).unwrap();
        let mut contents = Vec::new();
        file.read_to_end(&mut contents).unwrap();
        assert_eq!(contents, b"approved");
        assert_eq!(fs::read(&outside_file).unwrap(), b"attacker");
    }

    #[cfg(unix)]
    #[test]
    fn replacing_a_parent_component_between_stat_and_open_is_rejected() {
        let directory = scratch("parent-component-race");
        let parent = directory.join("parent");
        let replacement = directory.join("replacement");
        fs::create_dir(&parent).unwrap();
        fs::create_dir(&replacement).unwrap();
        fs::write(parent.join("inside"), b"approved").unwrap();
        fs::write(replacement.join("inside"), b"attacker").unwrap();

        let canonical_parent = fs::canonicalize(&parent).unwrap();
        let expected = fs::metadata(&canonical_parent).unwrap();
        let moved = directory.join("original-parent");
        fs::rename(&parent, &moved).unwrap();
        fs::rename(&replacement, &parent).unwrap();

        assert!(matches!(
            open_verified_parent(&canonical_parent, &expected),
            Err(FsError::InvalidPath)
        ));
        assert_eq!(fs::read(parent.join("inside")).unwrap(), b"attacker");
    }

    #[cfg(unix)]
    #[test]
    fn opening_a_directory_chain_never_follows_a_replaced_component_symlink() {
        use std::os::unix::fs::symlink;

        let directory = scratch("component-symlink-race");
        let parent = directory.join("parent");
        let outside = directory.join("outside");
        fs::create_dir(&parent).unwrap();
        fs::create_dir(&outside).unwrap();
        let canonical_parent = fs::canonicalize(&parent).unwrap();
        let expected = fs::metadata(&canonical_parent).unwrap();
        let moved = directory.join("original-parent");
        fs::rename(&parent, &moved).unwrap();
        symlink(&outside, &canonical_parent).unwrap();

        assert!(open_verified_parent(&canonical_parent, &expected).is_err());
        assert!(fs::read_dir(&outside).unwrap().next().is_none());
    }

    #[cfg(windows)]
    #[test]
    fn windows_parent_handle_validation_rejects_an_out_of_scope_directory() {
        let directory = scratch("windows-parent-scope");
        let approved = directory.join("approved");
        let outside = directory.join("outside");
        fs::create_dir(&approved).unwrap();
        fs::create_dir(&outside).unwrap();
        let approved = fs::canonicalize(approved).unwrap();
        let outside = fs::canonicalize(outside).unwrap();

        let outside_handle = open_directory_chain(&outside).unwrap();
        assert!(!windows_handle_matches_path(&outside_handle, &approved).unwrap());
        assert!(windows_handle_matches_path(&outside_handle, &outside).unwrap());
    }

    #[cfg(windows)]
    #[test]
    fn windows_path_scope_preserves_extended_absolute_paths() {
        let regular = HostPath::new(r"C:\zirk\project\data.txt");
        let extended = HostPath::new(r"\\?\C:\zirk\project\data.txt");

        assert_eq!(path_scope(regular).unwrap(), r"C:\zirk\project\data.txt");
        assert_eq!(
            path_scope(extended).unwrap(),
            r"\\?\C:\zirk\project\data.txt"
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_no_replace_rename_is_typed_unsupported() {
        let directory = scratch("windows-no-replace-rename");
        let source = directory.join("source.txt");
        fs::write(&source, b"preserve source").unwrap();
        let parent = Dir::open_ambient_dir(&directory, ambient_authority()).unwrap();

        assert!(matches!(
            rename_without_replacement(
                &parent,
                std::ffi::OsStr::new("source.txt"),
                &parent,
                std::ffi::OsStr::new("destination.txt"),
            ),
            Err(FsError::Unsupported)
        ));
        assert_eq!(fs::read(source).unwrap(), b"preserve source");
        assert!(!directory.join("destination.txt").exists());
    }

    #[cfg(windows)]
    #[test]
    fn windows_directory_chain_rejects_an_intermediate_reparse_point() {
        use std::os::windows::fs::symlink_dir;

        let directory = scratch("windows-reparse-component");
        let target = directory.join("target");
        let link = directory.join("link");
        fs::create_dir(&target).unwrap();
        fs::create_dir(target.join("child")).unwrap();
        if let Err(error) = symlink_dir(&target, &link) {
            if error.kind() == io::ErrorKind::PermissionDenied {
                eprintln!(
                    "skipping reparse traversal check: symbolic-link privilege is unavailable"
                );
                return;
            }
            panic!("failed to create test directory link: {error}");
        }

        assert!(open_directory_chain(&link.join("child")).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn windows_file_reparse_replacement_never_opens_its_target() {
        use std::os::windows::fs::symlink_file;

        let directory = scratch("windows-file-reparse-race");
        let victim = directory.join("victim");
        let outside = directory.join("outside");
        fs::write(&victim, b"approved").unwrap();
        fs::write(&outside, b"keep me safe").unwrap();

        let resolved = resolve_path(victim.to_str().unwrap(), false).unwrap();
        fs::remove_file(&victim).unwrap();
        if let Err(error) = symlink_file(&outside, &victim) {
            if error.kind() == io::ErrorKind::PermissionDenied {
                eprintln!("skipping file reparse check: symbolic-link privilege is unavailable");
                return;
            }
            panic!("failed to create test file link: {error}");
        }

        let opened = open_resolved(&resolved, true, false, false, false);
        assert!(
            opened.is_err(),
            "reparse-point path unexpectedly opened: {opened:?}"
        );
        assert_eq!(fs::read(&outside).unwrap(), b"keep me safe");
    }

    #[test]
    fn an_open_parent_handle_is_not_redirected_by_later_component_replacement() {
        let directory = scratch("pinned-parent-race");
        let parent = directory.join("parent");
        let replacement = directory.join("replacement");
        fs::create_dir(&parent).unwrap();
        fs::create_dir(&replacement).unwrap();
        let original_file = parent.join("target");
        let replacement_file = replacement.join("target");
        fs::write(&original_file, b"approved").unwrap();
        fs::write(&replacement_file, b"attacker").unwrap();

        let resolved = resolve_path(original_file.to_str().unwrap(), false).unwrap();
        let moved = directory.join("moved-parent");
        #[cfg(windows)]
        {
            // cap-std intentionally opens Windows directories without
            // FILE_SHARE_DELETE, so the pinned capability prevents replacing
            // this path component while the operation is in flight.
            assert!(fs::rename(&parent, &moved).is_err());
        }
        #[cfg(not(windows))]
        {
            fs::rename(&parent, &moved).unwrap();
            fs::rename(&replacement, &parent).unwrap();
        }

        let mut file = open_resolved(&resolved, true, false, false, false).unwrap();
        let mut contents = Vec::new();
        file.read_to_end(&mut contents).unwrap();
        assert_eq!(contents, b"approved");
        #[cfg(not(windows))]
        assert_eq!(fs::read(parent.join("target")).unwrap(), b"attacker");
    }

    #[test]
    fn bounded_read_checks_growth_while_consuming() {
        let directory = scratch("bounded-read");
        let path = directory.join("source");
        fs::write(&path, b"12345").unwrap();
        let resolved = resolve_path(path.to_str().unwrap(), false).unwrap();
        let mut file = open_resolved(&resolved, true, false, false, false).unwrap();
        assert_eq!(read_bounded(&mut file, 5).unwrap(), b"12345");
        file.rewind().unwrap();
        assert_eq!(read_bounded(&mut file, 4), Err(FsError::Limit));
    }

    #[test]
    fn partial_handle_reads_require_content_permission_not_metadata_permission() {
        let directory = scratch("metadata-does-not-read-handle");
        let path = directory.join("source");
        fs::write(&path, b"secret").unwrap();
        let file = cap_std::fs::File::from_std(fs::File::open(&path).unwrap());
        let mut open = OpenFile {
            file,
            path: path.to_str().unwrap().to_owned(),
            read: true,
            write: false,
            append: false,
            io_lock: Arc::new(Mutex::new(())),
        };
        let mut requested = String::new();

        let result = read_open_file(&mut open, 4, |capability, _| {
            requested = capability.to_owned();
            if capability == "filesystem.metadata" {
                Ok(())
            } else {
                Err(FsError::PermissionDenied)
            }
        });

        assert_eq!(requested, "filesystem.read");
        assert_eq!(result, Err(FsError::PermissionDenied));
        assert_eq!(
            read_open_file(&mut open, 4, |capability, _| {
                if capability == "filesystem.read" {
                    Ok(())
                } else {
                    Err(FsError::PermissionDenied)
                }
            })
            .unwrap(),
            b"secr"
        );
    }

    #[test]
    fn strict_text_decoding_and_explicit_encodings_are_distinct() {
        assert_eq!(decode_text(b"\xff", 0), Err(FsError::Decoding));
        assert_eq!(decode_text(b"\xff", 1).unwrap(), "ÿ");
        assert_eq!(encode_text("é", 1).unwrap(), vec![0xe9]);
        assert_eq!(
            decode_text(&encode_text("snowman ☃", 2).unwrap(), 2).unwrap(),
            "snowman ☃"
        );
    }

    #[test]
    fn concurrent_file_slot_reservations_never_exceed_the_process_limit() {
        let count = AtomicUsize::new(0);
        let admitted = AtomicUsize::new(0);
        std::thread::scope(|scope| {
            for _ in 0..8 {
                scope.spawn(|| {
                    for _ in 0..(MAX_OPEN_FILES * 2) {
                        if reserve_file_slot(&count).is_ok() {
                            admitted.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                });
            }
        });
        assert_eq!(admitted.load(Ordering::Acquire), MAX_OPEN_FILES);
        assert_eq!(count.load(Ordering::Acquire), MAX_OPEN_FILES);
        assert_eq!(reserve_file_slot(&count), Err(FsError::Limit));
    }

    #[test]
    fn temporary_replacement_is_sibling_scoped_and_replaces_whole_file() {
        let directory = scratch("atomic-write");
        let path = directory.join("target");
        fs::write(&path, b"old contents").unwrap();
        let resolved = resolve_path(path.to_str().unwrap(), false).unwrap();
        let temporary = directory.join("temporary-check");
        fs::write(&temporary, b"new contents").unwrap();
        resolved
            .parent
            .rename("temporary-check", &resolved.parent, &resolved.name)
            .unwrap();
        assert_eq!(fs::read(path).unwrap(), b"new contents");
    }
}
