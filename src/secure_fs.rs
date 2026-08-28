use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
#[cfg(unix)]
use std::os::unix::io::AsRawFd;

const DIR_MODE: u32 = 0o700;
const FILE_MODE: u32 = 0o600;

#[cfg(unix)]
const O_NOFOLLOW: i32 = 0o100_000;
#[cfg(unix)]
const O_CLOEXEC: i32 = 0o2_000_000;

pub fn ensure_private_dir(path: &Path) -> io::Result<()> {
    if path.as_os_str().is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "empty path"));
    }
    reject_symlink(path)?;
    if path.is_dir() {
        set_mode(path, DIR_MODE)?;
        return Ok(());
    }
    if path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "path exists and is not a directory",
        ));
    }
    if let Some(parent) = path.parent() {
        if parent.as_os_str().is_empty() {
            create_dir_mode(path, DIR_MODE)?;
            return Ok(());
        }
        ensure_private_dir(parent)?;
    }
    create_dir_mode(path, DIR_MODE)?;
    Ok(())
}

pub fn reject_symlink(path: &Path) -> io::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let meta = path.symlink_metadata()?;
    if meta.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "refusing to use symlink path",
        ));
    }
    Ok(())
}

pub fn reject_non_regular_file(path: &Path) -> io::Result<()> {
    reject_symlink(path)?;
    if !path.exists() {
        return Ok(());
    }
    let meta = path.metadata()?;
    if !meta.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "refusing to use non-regular file",
        ));
    }
    Ok(())
}

pub fn write_private_new_file(path: &Path, data: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        ensure_private_dir(parent)?;
    }
    reject_symlink(path)?;
    if path.exists() {
        return Err(io::Error::new(io::ErrorKind::AlreadyExists, "file exists"));
    }
    write_exclusive(path, data)
}

pub fn write_private_file(path: &Path, data: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        ensure_private_dir(parent)?;
    }
    reject_non_regular_file(path)?;
    if path.exists() {
        write_truncate(path, data)?;
    } else {
        write_exclusive(path, data)?;
    }
    Ok(())
}

pub fn write_private_file_if_missing(path: &Path, data: &[u8]) -> io::Result<bool> {
    if path.exists() {
        reject_non_regular_file(path)?;
        return Ok(false);
    }
    write_private_new_file(path, data)?;
    Ok(true)
}

pub fn harden_sqlite_files(db_path: &Path) -> io::Result<()> {
    harden_file(db_path)?;
    let base = db_path.to_string_lossy();
    for sidecar in [format!("{base}-wal"), format!("{base}-shm")] {
        let path = Path::new(&sidecar);
        if path.exists() {
            harden_file(path)?;
        }
    }
    Ok(())
}

pub fn open_sqlite_connection(path: &Path) -> rusqlite::Result<(rusqlite::Connection, File)> {
    use rusqlite::{Connection, OpenFlags};
    if let Some(parent) = path.parent() {
        ensure_private_dir(parent)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    }
    let file = open_private_db_file(path)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    #[cfg(unix)]
    {
        let fd_path = format!("/proc/self/fd/{}", file.as_raw_fd());
        let conn = Connection::open_with_flags(
            fd_path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )?;
        Ok((conn, file))
    }
    #[cfg(not(unix))]
    {
        let conn = Connection::open(path)?;
        Ok((conn, file))
    }
}

pub fn harden_private_file(path: &Path) -> io::Result<()> {
    harden_file(path)
}

pub fn remove_files_with_prefix(dir: &Path, prefix: &str) -> io::Result<()> {
    reject_symlink(dir)?;
    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if entry.file_type().map(|t| t.is_symlink()).unwrap_or(true) {
            continue;
        }
        let remove = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(prefix));
        if remove {
            let _ = fs::remove_file(path);
        }
    }
    Ok(())
}

fn open_private_db_file(path: &Path) -> io::Result<File> {
    reject_symlink(path)?;
    if path.exists() {
        open_existing_private_file(path)
    } else {
        create_private_db_file(path)
    }
}

fn open_existing_private_file(path: &Path) -> io::Result<File> {
    reject_non_regular_file(path)?;
    open_private_flags(path, false)
}

fn create_private_db_file(path: &Path) -> io::Result<File> {
    open_private_flags(path, true)
}

#[cfg(unix)]
fn open_private_flags(path: &Path, create_new: bool) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options
        .read(true)
        .write(true)
        .custom_flags(O_NOFOLLOW | O_CLOEXEC);
    if create_new {
        options.create_new(true).mode(FILE_MODE);
    } else {
        options.create(false);
    }
    options.open(path)
}

#[cfg(not(unix))]
fn open_private_flags(path: &Path, create_new: bool) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true);
    if create_new {
        options.create_new(true);
    }
    options.open(path)
}

fn harden_file(path: &Path) -> io::Result<()> {
    reject_symlink(path)?;
    if path.exists() {
        set_mode(path, FILE_MODE)?;
    }
    Ok(())
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> io::Result<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) -> io::Result<()> {
    Ok(())
}

#[cfg(unix)]
fn create_dir_mode(path: &Path, mode: u32) -> io::Result<()> {
    fs::DirBuilder::new()
        .mode(mode)
        .recursive(false)
        .create(path)
}

#[cfg(not(unix))]
fn create_dir_mode(path: &Path, _mode: u32) -> io::Result<()> {
    fs::create_dir(path)
}

#[cfg(unix)]
fn write_exclusive(path: &Path, data: &[u8]) -> io::Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(FILE_MODE)
        .custom_flags(O_NOFOLLOW | O_CLOEXEC)
        .open(path)?;
    file.write_all(data)?;
    Ok(())
}

#[cfg(not(unix))]
fn write_exclusive(path: &Path, data: &[u8]) -> io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(data)?;
    Ok(())
}

#[cfg(unix)]
fn write_truncate(path: &Path, data: &[u8]) -> io::Result<()> {
    reject_non_regular_file(path)?;
    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .custom_flags(O_NOFOLLOW | O_CLOEXEC)
        .open(path)?;
    file.write_all(data)?;
    set_mode(path, FILE_MODE)?;
    Ok(())
}

#[cfg(not(unix))]
fn write_truncate(path: &Path, data: &[u8]) -> io::Result<()> {
    let mut file = OpenOptions::new().write(true).truncate(true).open(path)?;
    file.write_all(data)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn private_dir_and_file_modes() {
        let dir = tempfile::TempDir::new().unwrap();
        let nested = dir.path().join("omapaste/data");
        ensure_private_dir(&nested).unwrap();
        assert!(nested.is_dir());
        #[cfg(unix)]
        assert_eq!(
            nested.metadata().unwrap().permissions().mode() & 0o777,
            DIR_MODE
        );

        let file = nested.join("history.sqlite");
        write_private_new_file(&file, b"db").unwrap();
        #[cfg(unix)]
        assert_eq!(
            file.metadata().unwrap().permissions().mode() & 0o777,
            FILE_MODE
        );
    }

    #[test]
    fn rejects_symlink_targets() {
        let dir = tempfile::TempDir::new().unwrap();
        let real = dir.path().join("real");
        fs::write(&real, b"x").unwrap();
        let link = dir.path().join("link");
        symlink(&real, &link).unwrap();
        assert!(reject_symlink(&link).is_err());
        assert!(write_private_file(&link, b"y").is_err());
    }

    #[test]
    fn write_if_missing_leaves_existing_bytes() {
        let dir = tempfile::TempDir::new().unwrap();
        ensure_private_dir(dir.path()).unwrap();
        let file = dir.path().join("clip.bin");
        assert!(write_private_file_if_missing(&file, b"first").unwrap());
        assert!(!write_private_file_if_missing(&file, b"second").unwrap());
        assert_eq!(fs::read(&file).unwrap(), b"first");
    }

    #[test]
    fn open_sqlite_rejects_symlink_db() {
        let dir = tempfile::TempDir::new().unwrap();
        ensure_private_dir(dir.path()).unwrap();
        let real = dir.path().join("real.sqlite");
        write_private_new_file(&real, b"").unwrap();
        let link = dir.path().join("history.sqlite");
        symlink(&real, &link).unwrap();
        assert!(open_sqlite_connection(&link).is_err());
    }

    #[test]
    fn open_sqlite_opens_regular_db_file() {
        let dir = tempfile::TempDir::new().unwrap();
        ensure_private_dir(dir.path()).unwrap();
        let path = dir.path().join("history.sqlite");
        let (conn, _file) = open_sqlite_connection(&path).unwrap();
        conn.execute_batch("CREATE TABLE t (x INTEGER);").unwrap();
        assert!(path.is_file());
        #[cfg(unix)]
        assert_eq!(
            path.metadata().unwrap().permissions().mode() & 0o777,
            FILE_MODE
        );
    }

    #[test]
    fn open_sqlite_hardens_wal_and_shm() {
        let dir = tempfile::TempDir::new().unwrap();
        ensure_private_dir(dir.path()).unwrap();
        let path = dir.path().join("history.sqlite");
        let (conn, _file) = open_sqlite_connection(&path).unwrap();
        conn.execute_batch("CREATE TABLE t (x INTEGER); INSERT INTO t VALUES (1);")
            .unwrap();
        harden_sqlite_files(&path).unwrap();
        let base = path.to_string_lossy();
        for sidecar in [format!("{base}-wal"), format!("{base}-shm")] {
            let sidecar_path = Path::new(&sidecar);
            if sidecar_path.exists() {
                #[cfg(unix)]
                assert_eq!(
                    sidecar_path.metadata().unwrap().permissions().mode() & 0o777,
                    FILE_MODE
                );
            }
        }
    }

    #[test]
    fn remove_files_with_prefix_keeps_other_names() {
        let dir = tempfile::TempDir::new().unwrap();
        ensure_private_dir(dir.path()).unwrap();
        write_private_new_file(&dir.path().join("drag-old.png"), b"x").unwrap();
        write_private_new_file(&dir.path().join("keep.txt"), b"y").unwrap();
        remove_files_with_prefix(dir.path(), "drag-").unwrap();
        assert!(!dir.path().join("drag-old.png").exists());
        assert!(dir.path().join("keep.txt").exists());
    }
}
