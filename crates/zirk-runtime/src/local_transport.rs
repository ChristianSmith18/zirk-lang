//! Permission-checked local IPC stream and listener operations.
//!
//! The public endpoint grammar and authority model are defined by
//! `std.net::local` and `EffectiveNetworkGrant`. Unix socket cleanup records
//! the bound inode and removes it only while that exact socket still occupies
//! the authorized path.

use std::time::Duration;

use crate::network_transport::{MAX_TCP_READ_BYTES, MAX_TCP_WRITE_BYTES, NetworkTransportError};
use zirk_runtime_core::network_policy::{EffectiveNetworkGrant, validate_local_endpoint};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaleEndpointPolicy {
    RejectExisting,
    RemoveStaleSocket,
}

enum LocalStreamKind {
    #[cfg(unix)]
    Unix(tokio::net::UnixStream),
    #[cfg(windows)]
    PipeClient(tokio::net::windows::named_pipe::NamedPipeClient),
    #[cfg(windows)]
    PipeServer(tokio::net::windows::named_pipe::NamedPipeServer),
}

/// An owner-managed local byte stream with bounded, cooperative I/O.
pub struct LocalStream {
    inner: LocalStreamKind,
}

impl LocalStream {
    pub fn connect(
        endpoint: &str,
        grant: &EffectiveNetworkGrant,
        timeout: Duration,
    ) -> Result<Self, NetworkTransportError> {
        grant
            .authorize_local_connect(endpoint)
            .map_err(NetworkTransportError::from)?;
        validate_local_endpoint(endpoint).map_err(NetworkTransportError::from)?;

        #[cfg(unix)]
        if let Some(path) = endpoint.strip_prefix("unix://") {
            let path = canonical_socket_path(path)?;
            let stream = block_on(async {
                tokio::time::timeout(timeout, tokio::net::UnixStream::connect(path))
                    .await
                    .map_err(|_| NetworkTransportError::Timeout)?
                    .map_err(map_io_error)
            })?;
            return Ok(Self {
                inner: LocalStreamKind::Unix(stream),
            });
        }

        #[cfg(windows)]
        if let Some(name) = endpoint.strip_prefix("npipe://./pipe/") {
            let path = named_pipe_path(name);
            let client = block_on(async move {
                let deadline = tokio::time::Instant::now() + timeout;
                loop {
                    match tokio::net::windows::named_pipe::ClientOptions::new().open(&path) {
                        Ok(client) => return Ok(client),
                        Err(error)
                            if error.raw_os_error()
                                == Some(windows_sys::Win32::Foundation::ERROR_PIPE_BUSY as i32) =>
                        {
                            tokio::time::timeout_at(
                                deadline,
                                tokio::time::sleep(Duration::from_millis(10)),
                            )
                            .await
                            .map_err(|_| NetworkTransportError::Timeout)?;
                        }
                        Err(error) => return Err(map_io_error(error)),
                    }
                }
            })?;
            return Ok(Self {
                inner: LocalStreamKind::PipeClient(client),
            });
        }

        Err(NetworkTransportError::UnsupportedTransport)
    }

    pub fn read(&mut self, max_bytes: usize) -> Result<Option<Vec<u8>>, NetworkTransportError> {
        if max_bytes > MAX_TCP_READ_BYTES {
            return Err(NetworkTransportError::BufferLimitExceeded);
        }
        if max_bytes == 0 {
            return Ok(Some(Vec::new()));
        }
        block_on(async {
            let mut bytes = vec![0; max_bytes];
            let count = match &mut self.inner {
                #[cfg(unix)]
                LocalStreamKind::Unix(stream) => {
                    tokio::io::AsyncReadExt::read(stream, &mut bytes).await
                }
                #[cfg(windows)]
                LocalStreamKind::PipeClient(stream) => {
                    tokio::io::AsyncReadExt::read(stream, &mut bytes).await
                }
                #[cfg(windows)]
                LocalStreamKind::PipeServer(stream) => {
                    tokio::io::AsyncReadExt::read(stream, &mut bytes).await
                }
            }
            .map_err(map_io_error)?;
            if count == 0 {
                Ok(None)
            } else {
                bytes.truncate(count);
                Ok(Some(bytes))
            }
        })
    }

    pub fn write(&mut self, bytes: &[u8]) -> Result<usize, NetworkTransportError> {
        if bytes.len() > MAX_TCP_WRITE_BYTES {
            return Err(NetworkTransportError::BufferLimitExceeded);
        }
        if bytes.is_empty() {
            return Ok(0);
        }
        let bytes = bytes.to_vec();
        block_on(async {
            match &mut self.inner {
                #[cfg(unix)]
                LocalStreamKind::Unix(stream) => {
                    tokio::io::AsyncWriteExt::write(stream, &bytes).await
                }
                #[cfg(windows)]
                LocalStreamKind::PipeClient(stream) => {
                    tokio::io::AsyncWriteExt::write(stream, &bytes).await
                }
                #[cfg(windows)]
                LocalStreamKind::PipeServer(stream) => {
                    tokio::io::AsyncWriteExt::write(stream, &bytes).await
                }
            }
            .map_err(map_io_error)
        })
    }

    pub fn write_all(&mut self, bytes: &[u8]) -> Result<(), NetworkTransportError> {
        if bytes.len() > MAX_TCP_WRITE_BYTES {
            return Err(NetworkTransportError::BufferLimitExceeded);
        }
        let bytes = bytes.to_vec();
        block_on(async {
            match &mut self.inner {
                #[cfg(unix)]
                LocalStreamKind::Unix(stream) => {
                    tokio::io::AsyncWriteExt::write_all(stream, &bytes).await
                }
                #[cfg(windows)]
                LocalStreamKind::PipeClient(stream) => {
                    tokio::io::AsyncWriteExt::write_all(stream, &bytes).await
                }
                #[cfg(windows)]
                LocalStreamKind::PipeServer(stream) => {
                    tokio::io::AsyncWriteExt::write_all(stream, &bytes).await
                }
            }
            .map_err(map_io_error)
        })
    }

    pub fn shutdown(&mut self) -> Result<(), NetworkTransportError> {
        block_on(async {
            match &mut self.inner {
                #[cfg(unix)]
                LocalStreamKind::Unix(stream) => tokio::io::AsyncWriteExt::shutdown(stream).await,
                #[cfg(windows)]
                LocalStreamKind::PipeClient(stream) => {
                    tokio::io::AsyncWriteExt::shutdown(stream).await
                }
                #[cfg(windows)]
                LocalStreamKind::PipeServer(stream) => {
                    tokio::io::AsyncWriteExt::shutdown(stream).await
                }
            }
            .map_err(map_io_error)
        })
    }
}

/// One owner-managed listener. Unix cleanup checks the socket inode before
/// unlinking, so a replaced file or symlink is left untouched.
pub struct LocalListener {
    #[cfg(unix)]
    listener: tokio::net::UnixListener,
    #[cfg(unix)]
    _cleanup: Option<SocketPathIdentity>,
    #[cfg(windows)]
    listener: Option<tokio::net::windows::named_pipe::NamedPipeServer>,
    #[cfg(windows)]
    path: String,
}

impl LocalListener {
    pub fn bind(
        endpoint: &str,
        stale_policy: StaleEndpointPolicy,
        grant: &EffectiveNetworkGrant,
    ) -> Result<Self, NetworkTransportError> {
        grant
            .authorize_local_listen(endpoint)
            .map_err(NetworkTransportError::from)?;
        validate_local_endpoint(endpoint).map_err(NetworkTransportError::from)?;

        #[cfg(unix)]
        if let Some(path) = endpoint.strip_prefix("unix://") {
            let path = canonical_socket_path(path)?;
            prepare_unix_endpoint(&path, stale_policy)?;
            let listener =
                block_on(async { tokio::net::UnixListener::bind(&path).map_err(map_io_error) })?;
            let cleanup = SocketPathIdentity::capture(path)?;
            return Ok(Self {
                listener,
                _cleanup: Some(cleanup),
            });
        }

        #[cfg(windows)]
        if let Some(name) = endpoint.strip_prefix("npipe://./pipe/") {
            if stale_policy == StaleEndpointPolicy::RemoveStaleSocket {
                return Err(NetworkTransportError::UnsupportedTransport);
            }
            let path = named_pipe_path(name);
            let listener = block_on(async {
                tokio::net::windows::named_pipe::ServerOptions::new()
                    .first_pipe_instance(true)
                    .create(&path)
                    .map_err(map_io_error)
            })?;
            return Ok(Self {
                listener: Some(listener),
                path,
            });
        }

        Err(NetworkTransportError::UnsupportedTransport)
    }

    pub fn accept(&mut self, timeout: Duration) -> Result<LocalStream, NetworkTransportError> {
        #[cfg(unix)]
        {
            let (stream, _) = block_on(async {
                tokio::time::timeout(timeout, self.listener.accept())
                    .await
                    .map_err(|_| NetworkTransportError::Timeout)?
                    .map_err(map_io_error)
            })?;
            Ok(LocalStream {
                inner: LocalStreamKind::Unix(stream),
            })
        }

        #[cfg(windows)]
        {
            let server = self
                .listener
                .as_mut()
                .ok_or(NetworkTransportError::CapacityExceeded)?;
            block_on(async {
                tokio::time::timeout(timeout, server.connect())
                    .await
                    .map_err(|_| NetworkTransportError::Timeout)?
                    .map_err(map_io_error)
            })?;
            let server = self
                .listener
                .take()
                .ok_or(NetworkTransportError::CapacityExceeded)?;
            let next = block_on(async {
                tokio::net::windows::named_pipe::ServerOptions::new()
                    .first_pipe_instance(false)
                    .create(&self.path)
                    .map_err(map_io_error)
            })?;
            self.listener = Some(next);
            Ok(LocalStream {
                inner: LocalStreamKind::PipeServer(server),
            })
        }
    }
}

#[cfg(unix)]
struct SocketPathIdentity {
    parent: std::fs::File,
    name: std::ffi::CString,
    device: libc::dev_t,
    inode: libc::ino_t,
}

#[cfg(unix)]
impl SocketPathIdentity {
    fn capture(path: std::path::PathBuf) -> Result<Self, NetworkTransportError> {
        use std::os::fd::AsRawFd;
        use std::os::unix::ffi::OsStrExt;

        let parent_path = path.parent().ok_or(NetworkTransportError::InvalidAddress)?;
        let name = path
            .file_name()
            .ok_or(NetworkTransportError::InvalidAddress)?;
        let parent = std::fs::File::open(parent_path).map_err(map_io_error)?;
        let name = std::ffi::CString::new(name.as_bytes())
            .map_err(|_| NetworkTransportError::InvalidAddress)?;
        let metadata = stat_entry_at(parent.as_raw_fd(), &name)?;
        if metadata.st_mode & libc::S_IFMT != libc::S_IFSOCK {
            return Err(NetworkTransportError::Io);
        }
        Ok(Self {
            parent,
            name,
            device: metadata.st_dev,
            inode: metadata.st_ino,
        })
    }
}

#[cfg(unix)]
impl Drop for SocketPathIdentity {
    fn drop(&mut self) {
        use std::os::fd::AsRawFd;

        let Ok(metadata) = stat_entry_at(self.parent.as_raw_fd(), &self.name) else {
            return;
        };
        if metadata.st_mode & libc::S_IFMT == libc::S_IFSOCK
            && metadata.st_dev == self.device
            && metadata.st_ino == self.inode
        {
            // `unlinkat` removes the directory entry itself. It never follows
            // a replacement symlink, and the open parent descriptor prevents
            // a renamed/replaced parent path from redirecting cleanup.
            unsafe {
                libc::unlinkat(self.parent.as_raw_fd(), self.name.as_ptr(), 0);
            }
        }
    }
}

#[cfg(unix)]
fn stat_entry_at(
    directory: std::os::fd::RawFd,
    name: &std::ffi::CStr,
) -> Result<libc::stat, NetworkTransportError> {
    let mut metadata = std::mem::MaybeUninit::<libc::stat>::zeroed();
    // SAFETY: the name is NUL-terminated and metadata points to writable
    // storage. AT_SYMLINK_NOFOLLOW inspects a symlink as a link entry.
    let result = unsafe {
        libc::fstatat(
            directory,
            name.as_ptr(),
            metadata.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    if result != 0 {
        return Err(map_io_error(std::io::Error::last_os_error()));
    }
    // SAFETY: fstatat initialized the value on success.
    Ok(unsafe { metadata.assume_init() })
}

#[cfg(unix)]
fn prepare_unix_endpoint(
    path: &std::path::Path,
    stale_policy: StaleEndpointPolicy,
) -> Result<(), NetworkTransportError> {
    use std::os::fd::AsRawFd;
    use std::os::unix::ffi::OsStrExt;

    let parent_path = path.parent().ok_or(NetworkTransportError::InvalidAddress)?;
    let file_name = path
        .file_name()
        .ok_or(NetworkTransportError::InvalidAddress)?;
    let parent = std::fs::File::open(parent_path).map_err(map_io_error)?;
    let name = std::ffi::CString::new(file_name.as_bytes())
        .map_err(|_| NetworkTransportError::InvalidAddress)?;
    let metadata = match stat_entry_at(parent.as_raw_fd(), &name) {
        Ok(metadata) => metadata,
        Err(NetworkTransportError::InvalidAddress) => return Ok(()),
        Err(error) => return Err(error),
    };
    if stale_policy != StaleEndpointPolicy::RemoveStaleSocket
        || metadata.st_mode & libc::S_IFMT != libc::S_IFSOCK
    {
        return Err(NetworkTransportError::Io);
    }
    match std::os::unix::net::UnixStream::connect(path) {
        Ok(_) => return Err(NetworkTransportError::AddressNotAllowed),
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::ConnectionRefused | std::io::ErrorKind::NotFound
            ) => {}
        Err(error) => return Err(map_io_error(error)),
    }
    let current = stat_entry_at(parent.as_raw_fd(), &name)?;
    if current.st_mode & libc::S_IFMT != libc::S_IFSOCK
        || current.st_dev != metadata.st_dev
        || current.st_ino != metadata.st_ino
    {
        return Err(NetworkTransportError::AddressNotAllowed);
    }
    // Anchor stale cleanup to the already-open canonical parent directory.
    // unlinkat removes the named entry and does not follow a replacement link.
    if unsafe { libc::unlinkat(parent.as_raw_fd(), name.as_ptr(), 0) } != 0 {
        return Err(map_io_error(std::io::Error::last_os_error()));
    }
    Ok(())
}

#[cfg(unix)]
fn canonical_socket_path(path: &str) -> Result<std::path::PathBuf, NetworkTransportError> {
    use std::path::Path;
    let path = Path::new(path);
    let parent = path.parent().ok_or(NetworkTransportError::InvalidAddress)?;
    let name = path
        .file_name()
        .ok_or(NetworkTransportError::InvalidAddress)?;
    let canonical_parent = std::fs::canonicalize(parent).map_err(map_io_error)?;
    Ok(canonical_parent.join(name))
}

#[cfg(windows)]
fn named_pipe_path(name: &str) -> String {
    format!(r"\\.\pipe\{name}")
}

fn block_on<T>(
    future: impl std::future::Future<Output = Result<T, NetworkTransportError>>,
) -> Result<T, NetworkTransportError> {
    crate::network_future::block_on_current(future).map_err(|error| match error {
        crate::network_future::NetworkFutureError::Cancelled => NetworkTransportError::Cancelled,
        crate::network_future::NetworkFutureError::CapacityExceeded => {
            NetworkTransportError::CapacityExceeded
        }
    })?
}

fn map_io_error(error: std::io::Error) -> NetworkTransportError {
    match error.kind() {
        std::io::ErrorKind::PermissionDenied => NetworkTransportError::PermissionDenied,
        std::io::ErrorKind::NotFound => NetworkTransportError::InvalidAddress,
        _ => NetworkTransportError::Io,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::Executor;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEST_PATH: AtomicU64 = AtomicU64::new(1);

    #[cfg(unix)]
    fn test_endpoint(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "zirk-local-{label}-{}-{}.sock",
            std::process::id(),
            NEXT_TEST_PATH.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[cfg(windows)]
    fn pipe_endpoint() -> String {
        format!(
            "npipe://./pipe/zirk-local-{}-{}",
            std::process::id(),
            NEXT_TEST_PATH.fetch_add(1, Ordering::Relaxed)
        )
    }

    fn grant(connect: &[String], listen: &[String]) -> EffectiveNetworkGrant {
        EffectiveNetworkGrant::from_verified_network_scopes(
            std::iter::empty(),
            std::iter::empty(),
            std::iter::empty(),
            std::iter::empty(),
            connect.iter().cloned(),
            listen.iter().cloned(),
            false,
        )
        .unwrap()
    }

    #[test]
    #[cfg(unix)]
    fn local_stream_round_trip_uses_distinct_connect_and_listen_scopes() {
        let path = test_endpoint("round-trip");
        let endpoint = format!("unix://{}", path.display());
        let authority = grant(
            std::slice::from_ref(&endpoint),
            std::slice::from_ref(&endpoint),
        );
        let outcome = Executor::new().run_with_root(move || {
            let mut listener =
                LocalListener::bind(&endpoint, StaleEndpointPolicy::RejectExisting, &authority)
                    .unwrap();
            let mut client =
                LocalStream::connect(&endpoint, &authority, Duration::from_secs(1)).unwrap();
            let mut server = listener.accept(Duration::from_secs(1)).unwrap();
            assert_eq!(client.write_all(b"request"), Ok(()));
            assert_eq!(server.read(16), Ok(Some(b"request".to_vec())));
            assert_eq!(server.write_all(b"reply"), Ok(()));
            assert_eq!(client.read(16), Ok(Some(b"reply".to_vec())));
            assert_eq!(client.shutdown(), Ok(()));
            drop(listener);
            assert!(!path.exists(), "owned socket path must be cleaned up");
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    #[cfg(unix)]
    fn local_listen_denial_precedes_socket_path_creation() {
        let path = test_endpoint("denied");
        let endpoint = format!("unix://{}", path.display());
        let authority = grant(&[], &[]);
        let result = LocalListener::bind(
            &endpoint,
            StaleEndpointPolicy::RemoveStaleSocket,
            &authority,
        );
        assert_eq!(result.err(), Some(NetworkTransportError::PermissionDenied));
        assert!(!path.exists());
    }

    #[test]
    #[cfg(unix)]
    fn stale_policy_never_removes_regular_files_or_live_sockets() {
        let path = test_endpoint("stale-policy");
        let endpoint = format!("unix://{}", path.display());
        let authority = grant(&[], std::slice::from_ref(&endpoint));
        std::fs::write(&path, b"unrelated").unwrap();
        assert!(
            LocalListener::bind(
                &endpoint,
                StaleEndpointPolicy::RemoveStaleSocket,
                &authority,
            )
            .is_err()
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"unrelated");
        std::fs::remove_file(&path).unwrap();

        let outcome = Executor::new().run_with_root(move || {
            let listener =
                LocalListener::bind(&endpoint, StaleEndpointPolicy::RejectExisting, &authority)
                    .unwrap();
            assert_eq!(
                LocalListener::bind(
                    &endpoint,
                    StaleEndpointPolicy::RemoveStaleSocket,
                    &authority,
                )
                .err(),
                Some(NetworkTransportError::AddressNotAllowed)
            );
            assert!(path.exists());
            drop(listener);
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));

        let stale_path = test_endpoint("stale-removal");
        let stale_endpoint = format!("unix://{}", stale_path.display());
        let stale_authority = grant(&[], std::slice::from_ref(&stale_endpoint));
        let stale_socket = std::os::unix::net::UnixListener::bind(&stale_path).unwrap();
        drop(stale_socket);
        assert!(stale_path.exists());
        let outcome = Executor::new().run_with_root(move || {
            let listener = LocalListener::bind(
                &stale_endpoint,
                StaleEndpointPolicy::RemoveStaleSocket,
                &stale_authority,
            )
            .unwrap();
            assert!(
                stale_path.exists(),
                "replacement listener owns the endpoint"
            );
            drop(listener);
            assert!(
                !stale_path.exists(),
                "owned replacement endpoint is cleaned up"
            );
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    #[cfg(unix)]
    fn listener_cleanup_leaves_a_replacement_symlink_untouched() {
        let path = test_endpoint("symlink-cleanup");
        let target = test_endpoint("symlink-target");
        let endpoint = format!("unix://{}", path.display());
        let authority = grant(&[], std::slice::from_ref(&endpoint));
        std::fs::write(&target, b"preserve").unwrap();
        let cleanup_path = path.clone();
        let cleanup_target = target.clone();
        let outcome = Executor::new().run_with_root(move || {
            let listener =
                LocalListener::bind(&endpoint, StaleEndpointPolicy::RejectExisting, &authority)
                    .unwrap();
            std::fs::remove_file(&path).unwrap();
            std::os::unix::fs::symlink(&target, &path).unwrap();
            drop(listener);
            assert!(
                std::fs::symlink_metadata(&path)
                    .unwrap()
                    .file_type()
                    .is_symlink()
            );
            assert_eq!(std::fs::read(&target).unwrap(), b"preserve");
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
        let _ = std::fs::remove_file(cleanup_path);
        let _ = std::fs::remove_file(cleanup_target);
    }

    #[test]
    #[cfg(unix)]
    fn listener_cleanup_stays_anchored_to_the_original_parent_directory() {
        let directory = std::env::temp_dir().join(format!(
            "zl{}{}",
            std::process::id(),
            NEXT_TEST_PATH.fetch_add(1, Ordering::Relaxed)
        ));
        let moved = directory.with_extension("moved");
        let replacement = directory.with_extension("replacement");
        std::fs::create_dir(&directory).unwrap();
        std::fs::create_dir(&replacement).unwrap();
        let socket_path = directory.join("service.sock");
        let moved_socket = moved.join("service.sock");
        let endpoint = format!("unix://{}", socket_path.display());
        let authority = grant(&[], std::slice::from_ref(&endpoint));
        let cleanup_directory = directory.clone();
        let cleanup_moved = moved.clone();
        let cleanup_replacement = replacement.clone();

        let outcome = Executor::new().run_with_root(move || {
            let listener =
                LocalListener::bind(&endpoint, StaleEndpointPolicy::RejectExisting, &authority)
                    .unwrap();
            std::fs::rename(&directory, &moved).unwrap();
            std::os::unix::fs::symlink(&replacement, &directory).unwrap();
            drop(listener);
            assert!(
                std::fs::symlink_metadata(&moved_socket).is_err(),
                "owned socket should be removed in its original directory"
            );
            assert!(
                std::fs::symlink_metadata(directory.join("service.sock")).is_err(),
                "cleanup must not create or follow entries through the replacement path"
            );
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
        std::fs::remove_file(cleanup_directory).unwrap();
        std::fs::remove_dir_all(cleanup_moved).unwrap();
        std::fs::remove_dir_all(cleanup_replacement).unwrap();
    }

    #[test]
    #[cfg(windows)]
    fn named_pipe_stream_round_trip_uses_distinct_connect_and_listen_scopes() {
        let endpoint = pipe_endpoint();
        let authority = grant(
            std::slice::from_ref(&endpoint),
            std::slice::from_ref(&endpoint),
        );
        let task_endpoint = endpoint.clone();
        let outcome = Executor::new().run_with_root(move || {
            let mut listener = LocalListener::bind(
                &task_endpoint,
                StaleEndpointPolicy::RejectExisting,
                &authority,
            )
            .unwrap();
            let mut client =
                LocalStream::connect(&task_endpoint, &authority, Duration::from_secs(2)).unwrap();
            let mut server = listener.accept(Duration::from_secs(2)).unwrap();
            assert_eq!(client.write_all(b"ping"), Ok(()));
            assert_eq!(server.read(8), Ok(Some(b"ping".to_vec())));
            assert_eq!(server.write_all(b"pong"), Ok(()));
            assert_eq!(client.read(8), Ok(Some(b"pong".to_vec())));
            drop(client);
            drop(server);
            drop(listener);
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    #[cfg(windows)]
    fn named_pipe_permission_denial_precedes_pipe_creation() {
        let endpoint = pipe_endpoint();
        let authority = grant(&[], &[]);
        assert_eq!(
            LocalListener::bind(&endpoint, StaleEndpointPolicy::RejectExisting, &authority,).err(),
            Some(NetworkTransportError::PermissionDenied)
        );
    }

    #[test]
    #[cfg(windows)]
    fn named_pipe_listener_releases_first_instance_on_close() {
        let endpoint = pipe_endpoint();
        let authority = grant(&[], std::slice::from_ref(&endpoint));
        let task_endpoint = endpoint.clone();
        let task_authority = authority.clone();
        let outcome = Executor::new().run_with_root(move || {
            let listener = LocalListener::bind(
                &task_endpoint,
                StaleEndpointPolicy::RejectExisting,
                &task_authority,
            )
            .unwrap();
            assert!(
                LocalListener::bind(
                    &task_endpoint,
                    StaleEndpointPolicy::RejectExisting,
                    &task_authority,
                )
                .is_err(),
                "first-pipe-instance prevents a duplicate listener"
            );
            drop(listener);
            let replacement = LocalListener::bind(
                &task_endpoint,
                StaleEndpointPolicy::RejectExisting,
                &task_authority,
            )
            .unwrap();
            drop(replacement);
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }
}
