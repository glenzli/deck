//! Bounded file intake and atomic project publication; no application content interpretation.
use crate::{Result, fail};
use serde_json::{Value, json};
use std::{
    fs::{self, File},
    io::Read,
    path::{Component, Path, PathBuf},
};

pub fn read(path: &Path, maximum: u64) -> Result<Vec<u8>> {
    read_file(open_regular(path)?, maximum)
}

fn read_file(file: File, maximum: u64) -> Result<Vec<u8>> {
    if !file.metadata()?.is_file() {
        return Err(fail("file_not_regular", "expected a regular file"));
    }
    let length = file.metadata()?.len();
    if length > maximum {
        return Err(fail(
            "file_too_large",
            "file exceeds this operation's byte limit",
        ));
    }
    let mut bytes = Vec::new();
    file.take(maximum + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 != length {
        return Err(fail("file_changed", "file length changed during read"));
    }
    Ok(bytes)
}

fn open_regular(path: &Path) -> Result<File> {
    if !fs::symlink_metadata(path)?.file_type().is_file() {
        return Err(fail(
            "file_not_regular",
            "expected a regular file, without a symbolic link",
        ));
    }
    #[cfg(unix)]
    let file = File::from(rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::CLOEXEC
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::NONBLOCK,
        rustix::fs::Mode::empty(),
    )?);
    #[cfg(not(unix))]
    let file = File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err(fail("file_not_regular", "expected a regular file"));
    }
    Ok(file)
}

pub fn hash_report(path: &Path) -> Result<Value> {
    let mut file = open_regular(path)?;
    let length = file.metadata()?.len();
    // Streaming admission bounds time as well as memory; PDF publishing has its own page/image limits.
    if length > 1024 * 1024 * 1024 {
        return Err(fail(
            "file_too_large",
            "output verification supports files up to 1 GiB",
        ));
    }
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    let mut count = 0_u64;
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        count += n as u64;
        if count > length {
            return Err(fail("file_changed", "file grew during verification"));
        }
        hasher.update(&buffer[..n]);
    }
    if count != length || file.metadata()?.len() != length {
        return Err(fail("file_changed", "file changed during verification"));
    }
    Ok(
        json!({"path":path.canonicalize()?,"byte_length":length,"blake3":hasher.finalize().to_hex().to_string()}),
    )
}

pub fn read_material(root: &Path, relative: &str, maximum: u64) -> Result<Vec<u8>> {
    if relative.is_empty()
        || relative.contains('\\')
        || relative
            .split('/')
            .any(|part| part.is_empty() || matches!(part, "." | ".."))
    {
        return Err(fail(
            "transfer_path",
            "material paths must be simple relative paths",
        ));
    }
    let components = Path::new(relative)
        .components()
        .map(|component| {
            if let Component::Normal(name) = component {
                Ok(name)
            } else {
                Err(fail(
                    "transfer_path",
                    "material path cannot be absolute or traverse parents",
                ))
            }
        })
        .collect::<Result<Vec<_>>>()?;
    #[cfg(unix)]
    let (last, parents) = components
        .split_last()
        .ok_or_else(|| fail("transfer_path", "material path is empty"))?;
    #[cfg(unix)]
    let file = {
        use rustix::fs::{Mode, OFlags, open, openat};
        let directory_flags =
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW;
        let mut directory = File::from(open(root.canonicalize()?, directory_flags, Mode::empty())?);
        for name in parents {
            directory = File::from(openat(&directory, *name, directory_flags, Mode::empty())?);
        }
        File::from(openat(
            &directory,
            *last,
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
            Mode::empty(),
        )?)
    };
    #[cfg(not(unix))]
    let file = {
        let mut path = root.canonicalize()?;
        for name in &components {
            path.push(name);
            if fs::symlink_metadata(&path)?.file_type().is_symlink() {
                return Err(fail(
                    "transfer_path",
                    "material paths cannot traverse symbolic links",
                ));
            }
        }
        open_regular(&path)?
    };
    read_file(file, maximum)
}

/// A failed import removes only its random staging directory, never the destination.
#[derive(Debug)]
pub struct ProjectStage {
    pub path: PathBuf,
    destination: PathBuf,
    staging_root: PathBuf,
}
impl ProjectStage {
    pub fn new(destination: &Path) -> Result<Self> {
        let parent = destination
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."))
            .canonicalize()?;
        let name = destination
            .file_name()
            .ok_or_else(|| fail("project_path", "project needs a file name"))?;
        let destination = parent.join(name);
        if fs::symlink_metadata(&destination).is_ok() {
            return Err(fail(
                "destination_exists",
                "project destination already exists",
            ));
        }
        let staging_root = parent.join(format!(".deck-import-{}.partial", uuid::Uuid::now_v7()));
        fs::create_dir(&staging_root)?;
        let path = staging_root.join("project");
        Ok(Self {
            path,
            destination,
            staging_root,
        })
    }
    pub fn publish(self) -> Result<()> {
        #[cfg(unix)]
        rustix::fs::renameat_with(
            rustix::fs::CWD,
            &self.path,
            rustix::fs::CWD,
            &self.destination,
            rustix::fs::RenameFlags::NOREPLACE,
        )?;
        #[cfg(not(unix))]
        fs::rename(&self.path, &self.destination)?;
        Ok(())
    }
}
impl Drop for ProjectStage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.staging_root);
    }
}
