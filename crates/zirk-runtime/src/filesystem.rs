//! Synchronous, bounded filesystem host operations for the compiler-owned
//! `std.fs` and `std.path` declarations. Every effect is rooted at an opened
//! capability directory, uses a canonical exact-resource permission, and
//! validates an opened file handle before reading or changing its contents.

use std::collections::HashMap;
use std::ffi::c_void;
#[cfg(test)]
use std::io::Seek;
use std::io::{self, Read, Write};
use std::path::{Component, Path as HostPath, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::UNIX_EPOCH;

use cap_std::ambient_authority;
use cap_std::fs::{Dir, OpenOptions};

pub const MAX_WHOLE_FILE_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_CHUNK_BYTES: u64 = 4 * 1024 * 1024;
pub const MAX_LINE_BYTES: usize = 1024 * 1024;
pub const MAX_OPEN_FILES: usize = 256;

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
    match error.kind() {
        io::ErrorKind::NotFound => FsError::NotFound,
        io::ErrorKind::PermissionDenied => FsError::PermissionDenied,
        io::ErrorKind::AlreadyExists => FsError::AlreadyExists,
        io::ErrorKind::InvalidInput | io::ErrorKind::InvalidData => FsError::InvalidPath,
        io::ErrorKind::IsADirectory | io::ErrorKind::NotADirectory => FsError::WrongType,
        _ => FsError::Io,
    }
}

struct OpenFile {
    file: cap_std::fs::File,
    path: String,
    read: bool,
    write: bool,
    append: bool,
}

static FILES: OnceLock<Mutex<HashMap<u64, OpenFile>>> = OnceLock::new();
static NEXT_HANDLE: AtomicU64 = AtomicU64::new(1);
static NEXT_TEMP: AtomicU64 = AtomicU64::new(1);
static OPEN_FILE_COUNT: AtomicUsize = AtomicUsize::new(0);

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

impl OpenFileReservation {
    fn reserve() -> Result<Self, FsError> {
        reserve_file_slot(&OPEN_FILE_COUNT)?;
        Ok(Self { committed: false })
    }

    fn commit(mut self) {
        self.committed = true;
    }
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
    let parent = Dir::open_ambient_dir(parent_path, ambient_authority()).map_err(map_io)?;
    let actual_parent = parent.dir_metadata().map_err(map_io)?;
    if !same_metadata_snapshot(&expected_parent, &actual_parent) {
        return Err(FsError::InvalidPath);
    }
    #[cfg(windows)]
    if !windows_handle_matches_path(&parent, parent_path)? {
        return Err(FsError::InvalidPath);
    }
    Ok(ResolvedPath {
        canonical,
        parent,
        name,
        metadata,
    })
}

fn path_text(path: &HostPath) -> Result<String, FsError> {
    path.to_str().map(str::to_owned).ok_or(FsError::InvalidPath)
}

fn path_scope(path: &HostPath) -> Result<String, FsError> {
    path_text(path)
}

fn same_metadata_snapshot(left: &std::fs::Metadata, right: &cap_std::fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use cap_std::fs::MetadataExt as CapMetadataExt;
        use std::os::unix::fs::MetadataExt as StdMetadataExt;
        left.dev() == right.dev() && left.ino() == right.ino()
    }
    #[cfg(windows)]
    {
        use cap_std::fs::MetadataExt as CapMetadataExt;
        use std::os::windows::fs::MetadataExt as StdMetadataExt;
        left.file_attributes() == right.file_attributes()
            && left.file_size() == right.file_size()
            && left.creation_time() == right.creation_time()
            && left.last_write_time() == right.last_write_time()
    }
    #[cfg(not(any(unix, windows)))]
    {
        left.len() == right.len() && left.modified().ok() == right.modified().ok()
    }
}

#[cfg(windows)]
fn windows_handle_matches_path(directory: &Dir, expected: &HostPath) -> Result<bool, FsError> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_NAME_NORMALIZED, GetFinalPathNameByHandleW, VOLUME_NAME_DOS,
    };

    let handle = directory.as_raw_handle().cast();
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
            return Ok(normalize_windows_path(&actual)
                .eq_ignore_ascii_case(&normalize_windows_path(expected)));
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
    if let Some(expected) = &resolved.metadata {
        if !same_metadata_snapshot(expected, &actual) {
            return Err(FsError::InvalidPath);
        }
    } else if resolved.parent.symlink_metadata(&resolved.name).is_err() {
        return Err(FsError::InvalidPath);
    }
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
    if requested > MAX_WHOLE_FILE_BYTES {
        return Err(FsError::Limit);
    }
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
            if file.read(&mut probe).map_err(map_io)? != 0 {
                return Err(FsError::Limit);
            }
            break;
        }
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

fn make_metadata_array(metadata: &cap_std::fs::Metadata) -> *mut c_void {
    let modified = metadata
        .modified()
        .ok()
        .and_then(|time| time.into_std().duration_since(UNIX_EPOCH).ok())
        .and_then(|duration| u64::try_from(duration.as_millis()).ok());
    let values = [
        metadata.len(),
        u64::from(metadata.is_file()),
        u64::from(metadata.is_dir()),
        modified.unwrap_or_default(),
        u64::from(modified.is_some()),
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

fn direct_write(path: &str, bytes: &[u8]) -> Result<(), FsError> {
    let resolved = resolve_authorized(path, "filesystem.write", true)?;
    let mut file = open_resolved(&resolved, false, true, false, false)?;
    file.set_len(0).map_err(map_io)?;
    file.write_all(bytes).map_err(map_io)
}

fn append_to_path(path: &str, bytes: &[u8]) -> Result<(), FsError> {
    let (_resolved, mut file) = open_write_path(path, true)?;
    file.write_all(bytes).map_err(map_io)
}

fn atomic_write(path: &str, bytes: &[u8]) -> Result<(), FsError> {
    let resolved = resolve_authorized(path, "filesystem.write", true)?;
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
        match resolved.parent.open_with(&candidate, &options) {
            Ok(mut file) => {
                if let Err(error) = file.write_all(bytes).and_then(|()| file.sync_all()) {
                    let _ = resolved.parent.remove_file(&candidate);
                    return Err(map_io(error));
                }
                break candidate;
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(map_io(error)),
        }
    };
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
        FILE_READ_BYTES | FILE_READ_TEXT => {
            let path = unsafe { argument_string(args, 0)? };
            let limit = argument_u64(args, 1)?;
            let encoding = if operation == FILE_READ_TEXT {
                argument_u64(args, 2)?
            } else {
                0
            };
            let (_resolved, mut file) = open_read_path(&path)?;
            let bytes = read_bounded(&mut file, limit)?;
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
            if max_bytes > MAX_CHUNK_BYTES {
                return Err(FsError::Limit);
            }
            let mut files = files()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let open = files.get_mut(&handle).ok_or(FsError::Closed)?;
            if !open.read {
                return Err(FsError::InvalidOptions);
            }
            authorize("filesystem.metadata", &open.path)?;
            let mut bytes = vec![0; usize::try_from(max_bytes).map_err(|_| FsError::Limit)?];
            let read = open.file.read(&mut bytes).map_err(map_io)?;
            bytes.truncate(read);
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
