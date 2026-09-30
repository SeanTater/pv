//! Private IPC between Rust pv instances. The CLI matches upstream; its native
//! shared-memory protocol is intentionally not used by this implementation.
use crate::cli::Config;
use std::io;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Snapshot {
    pub bytes: u64,
    pub units: u64,
    pub total: Option<u64>,
    pub elapsed: f64,
    pub rate: f64,
    pub average: f64,
}

#[cfg(unix)]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Request {
    pub update: Option<Config>,
}

#[cfg(unix)]
mod unix {
    use super::*;
    use std::io::{BufRead, Read, Write};
    use std::os::unix::{
        fs::{DirBuilderExt, MetadataExt},
        net::{UnixListener, UnixStream},
    };
    use std::path::PathBuf;
    use std::time::Duration;

    pub(crate) fn directory() -> io::Result<PathBuf> {
        // SAFETY: geteuid has no preconditions.
        let uid = unsafe { libc::geteuid() };
        let directory = std::env::temp_dir().join(format!("pv-rust-{uid}"));
        match std::fs::DirBuilder::new().mode(0o700).create(&directory) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
        let meta = std::fs::symlink_metadata(&directory)?;
        if !meta.is_dir() || meta.uid() != uid || meta.mode() & 0o077 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "IPC directory must be private and owned by the current user",
            ));
        }
        Ok(directory)
    }
    fn address(pid: u32) -> io::Result<PathBuf> {
        Ok(directory()?.join(format!("{pid}.sock")))
    }

    pub struct Control {
        listener: UnixListener,
        path: PathBuf,
    }
    impl Control {
        pub fn bind() -> io::Result<Self> {
            let path = address(std::process::id())?;
            if path.exists() && UnixStream::connect(&path).is_err() {
                std::fs::remove_file(&path)?;
            }
            let listener = UnixListener::bind(&path)?;
            listener.set_nonblocking(true)?;
            Ok(Self { listener, path })
        }
        pub fn service(&self, snapshot: &Snapshot) -> io::Result<Vec<Config>> {
            let mut updates = Vec::new();
            // Bound requests per tick, and socket reads/writes, so another client
            // cannot indefinitely hold up payload transfer.
            for _ in 0..8 {
                let (mut stream, _) = match self.listener.accept() {
                    Ok(s) => s,
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                    Err(e) => return Err(e),
                };
                stream.set_read_timeout(Some(Duration::from_millis(20)))?;
                stream.set_write_timeout(Some(Duration::from_millis(20)))?;
                let mut message = Vec::new();
                let _ = std::io::BufReader::new((&mut stream).take(16384))
                    .read_until(b'\n', &mut message);
                let Ok(request) = serde_json::from_slice::<Request>(&message) else {
                    continue;
                };
                if let Some(update) = request.update {
                    updates.push(update);
                }
                let response = serde_json::to_vec(snapshot)?;
                // Disconnected clients must not abort the actual transfer.
                let _ = stream.write_all(&response);
                let _ = stream.write_all(b"\n");
            }
            Ok(updates)
        }
    }
    impl Drop for Control {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
        }
    }

    pub fn request(pid: u32, config: &Config) -> io::Result<Snapshot> {
        let mut stream = UnixStream::connect(address(pid)?).map_err(|e| {
            io::Error::new(
                e.kind(),
                format!(
                    "cannot contact Rust pv PID {pid} (upstream IPC is not interoperable): {e}"
                ),
            )
        })?;
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        stream.set_write_timeout(Some(Duration::from_secs(2)))?;
        let request = Request {
            update: if config.remote.is_some() {
                Some(config.clone())
            } else {
                None
            },
        };
        let bytes = serde_json::to_vec(&request)?;
        if bytes.len() >= 16384 {
            return Err(io::Error::other("remote request exceeds 16KiB"));
        }
        stream.write_all(&bytes)?;
        stream.write_all(b"\n")?;
        let mut response = String::new();
        stream.take(16384).read_to_string(&mut response)?;
        serde_json::from_str(&response).map_err(io::Error::other)
    }
}

#[cfg(unix)]
pub(crate) use unix::directory;
#[cfg(unix)]
pub use unix::{request, Control};

#[cfg(not(unix))]
pub struct Control;
#[cfg(not(unix))]
impl Control {
    pub fn service(&self, _: &Snapshot) -> io::Result<Vec<Config>> {
        Ok(Vec::new())
    }
}
#[cfg(not(unix))]
pub fn request(_: u32, _: &Config) -> io::Result<Snapshot> {
    Err(io::Error::other("remote/query require Unix domain sockets"))
}
