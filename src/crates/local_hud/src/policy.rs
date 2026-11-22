//! Path opt-in. Every open goes through here. Symlinks are not followed.
//! Denied names are never opened, including credential and cookie stores.

use crate::model::{reasons, Skipped};
use std::fs::{self, File};
use std::io;
use std::path::{Component, Path, PathBuf};

pub const MAX_METADATA: u64 = 64 * 1024;
pub const FILE_CAP: usize = 64;
pub const DISPLAY_CAP: usize = 20;

#[derive(Default)]
pub struct Access {
    pub opened: Vec<PathBuf>,
    pub skipped: Vec<Skipped>,
}

pub struct Stat {
    pub len: u64,
    pub modified_unix: Option<i64>,
    pub is_dir: bool,
    pub is_file: bool,
}

pub fn check_root(path: &Path) -> Result<(), &'static str> {
    if is_filesystem_root(path) {
        return Err(reasons::FILESYSTEM_ROOT);
    }
    if path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(denied_name)
    {
        return Err(reasons::DENIED_ROOT);
    }
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => Err(reasons::SYMLINK_ROOT),
        Ok(meta) if meta.is_dir() => Ok(()),
        Ok(_) => Err(reasons::NOT_A_DIRECTORY),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Err(reasons::MISSING),
        Err(_) => Err(reasons::UNREADABLE),
    }
}

fn is_filesystem_root(path: &Path) -> bool {
    !path
        .components()
        .any(|component| matches!(component, Component::Normal(_)))
}

pub fn denied_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    const EXACT: &[&str] = &[
        "auth.json",
        "credentials.json",
        "credentials",
        "cookies",
        "cookies.db",
        "cookies-journal",
        "login data",
        "login data-journal",
        "state.vscdb",
        "state.vscdb-wal",
        "state.vscdb-shm",
        ".claude.json",
        "config.toml",
        "hooks.json",
        "config.json",
        "sessionkey",
        "keychain",
        "local storage",
        "session storage",
        "indexeddb",
        "trust tokens",
        "webstorage",
    ];
    EXACT.contains(&lower.as_str())
        || lower.ends_with(".pem")
        || lower.ends_with(".key")
        || lower.ends_with(".vscdb")
        || lower.contains("cookie")
        || lower.contains("keychain")
}

impl Access {
    pub fn list(&mut self, root: &Path, rel: &Path) -> Vec<PathBuf> {
        let dir = match resolve(root, rel) {
            Ok(dir) => dir,
            Err("missing path") => return Vec::new(),
            Err(reason) => {
                self.skip(root.join(rel), reason);
                return Vec::new();
            }
        };
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => {
                self.skip(dir, "unreadable directory");
                return Vec::new();
            }
        };
        let mut names = Vec::new();
        for entry in entries.flatten() {
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if denied_name(&name) {
                self.skip(dir.join(&name), "denied file name");
                continue;
            }
            names.push(rel.join(name));
        }
        names.sort();
        names
    }

    pub fn open(&mut self, root: &Path, rel: &Path) -> Option<File> {
        let path = match resolve(root, rel) {
            Ok(path) => path,
            Err(reason) => {
                self.skip(root.join(rel), reason);
                return None;
            }
        };
        match File::open(&path) {
            Ok(file) => {
                self.opened.push(path);
                Some(file)
            }
            Err(_) => {
                self.skip(path, "unreadable file");
                None
            }
        }
    }

    pub fn stat(&self, root: &Path, rel: &Path) -> Option<Stat> {
        let path = resolve(root, rel).ok()?;
        let meta = fs::symlink_metadata(&path).ok()?;
        if meta.file_type().is_symlink() {
            return None;
        }
        Some(Stat {
            len: meta.len(),
            modified_unix: meta.modified().ok().and_then(|time| {
                time.duration_since(std::time::UNIX_EPOCH)
                    .ok()
                    .map(|duration| duration.as_secs() as i64)
            }),
            is_dir: meta.is_dir(),
            is_file: meta.is_file(),
        })
    }

    fn skip(&mut self, path: PathBuf, reason: &'static str) {
        self.skipped.push(Skipped { path, reason });
    }
}

fn resolve(root: &Path, rel: &Path) -> Result<PathBuf, &'static str> {
    let mut current = root.to_path_buf();
    real_dir(&current)?;
    if rel.as_os_str().is_empty() {
        return Ok(current);
    }
    for component in rel.components() {
        match component {
            Component::Normal(name) => {
                let Some(name) = name.to_str() else {
                    return Err("non-utf8 path");
                };
                if denied_name(name) {
                    return Err("denied file name");
                }
                current.push(name);
                real_node(&current)?;
            }
            Component::CurDir => {}
            _ => return Err("path leaves the opted-in root"),
        }
    }
    Ok(current)
}

fn real_dir(path: &Path) -> Result<(), &'static str> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => Err("symlink"),
        Ok(meta) if meta.is_dir() => Ok(()),
        Ok(_) => Err("not a directory"),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Err("missing path"),
        Err(_) => Err("unreadable path"),
    }
}

fn real_node(path: &Path) -> Result<(), &'static str> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => Err("symlink"),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Err("missing path"),
        Err(_) => Err("unreadable path"),
    }
}
