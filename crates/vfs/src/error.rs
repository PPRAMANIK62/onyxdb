//! The errors every file operation can return.
//!
//! Each variant says what was being done and to which path, and `fmt` turns
//! it into a message that says both.

use std::{error, fmt, io, path::PathBuf, result};

/// Everything that can go wrong with a file or directory.
#[derive(Debug)]
pub enum Error {
    /// Any other error from the operating system.
    Io {
        /// What was being done: "open", "write", "sync" and so on.
        op: &'static str,
        /// The path it was done to.
        path: PathBuf,
        /// What the operating system said.
        source: io::Error,
    },
    /// Nothing exists at this path.
    NotFound(PathBuf),
    /// Something already exists at this path.
    AlreadyExists(PathBuf),
    /// The path is a directory where a file was needed, or a file where a
    /// directory was needed.
    WrongType(PathBuf),
    /// The path is absolute, empty where a name is needed, or has `.` or `..` in it.
    BadPath(PathBuf),
    /// A rename between two different directories.
    CrossDirectory {
        /// The file being renamed.
        from: PathBuf,
        /// The name it was to get.
        to: PathBuf,
    },
    /// Another handle holds the lock on this file.
    Locked(PathBuf),
    /// A read asked for bytes past the end of the file.
    ShortRead {
        /// The file.
        path: PathBuf,
        /// Where the read started, in bytes.
        offset: u64,
        /// How many bytes it asked for.
        len: usize,
        /// The file's length, in bytes.
        file_len: u64,
    },
    /// The file handle was opened before a crash, so it no longer works.
    Stale(PathBuf),
    /// The power is off. `Mem::power_off_after` set how many changes could
    /// happen, and they're used up. Every change fails until `Mem::crash`.
    PowerOff,
}

/// What every function in this crate returns.
pub type Result<T> = result::Result<T, Error>;

impl Error {
    /// Turns an `io::Error` from `op` on `path` into an `Error`. Common
    /// errors, like a missing file, get their own variant.
    pub fn from_io(op: &'static str, path: impl Into<PathBuf>, source: io::Error) -> Error {
        let path = path.into();
        match source.kind() {
            io::ErrorKind::NotFound => Error::NotFound(path),
            io::ErrorKind::AlreadyExists => Error::AlreadyExists(path),
            io::ErrorKind::IsADirectory | io::ErrorKind::NotADirectory => Error::WrongType(path),
            _ => Error::Io { op, path, source },
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io { op, path, source } => write!(f, "{op} {}: {source}", path.display()),
            Error::NotFound(path) => write!(f, "{}: no such file or directory", path.display()),
            Error::AlreadyExists(path) => write!(f, "{}: already exists", path.display()),
            Error::WrongType(path) => write!(
                f,
                "{}: a file where a directory was needed, or the other way round",
                path.display()
            ),
            Error::BadPath(path) => {
                write!(f, "{:?}: paths must be relative, with no `.` or `..`", path)
            }
            Error::CrossDirectory { from, to } => {
                write!(
                    f,
                    "rename {} to {}: both must be in the same directory",
                    from.display(),
                    to.display()
                )
            }
            Error::Locked(path) => write!(f, "{}: another handle holds the lock", path.display()),
            Error::ShortRead {
                path,
                offset,
                len,
                file_len,
            } => write!(
                f,
                "read {}: {len} bytes at {offset} runs past the end of the file, which is {file_len} bytes",
                path.display()
            ),
            Error::Stale(path) => write!(
                f,
                "{}: this handle was opened before a crash",
                path.display()
            ),
            Error::PowerOff => write!(f, "the power is off"),
        }
    }
}

impl error::Error for Error {}
