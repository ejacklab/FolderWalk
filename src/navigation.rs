use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub(crate) struct Directory {
    path: PathBuf,
    entries: Vec<Entry>,
}

pub(crate) struct Entry {
    name: OsString,
    kind: EntryKind,
}

#[derive(Clone, Copy)]
pub(crate) enum EntryKind {
    Directory,
    Symlink,
    Other,
}

impl Directory {
    pub(crate) fn open(path: &Path) -> io::Result<Self> {
        let path = fs::canonicalize(path)?;
        let mut entries = Vec::new();

        for item in fs::read_dir(&path)? {
            let item = item?;
            let file_type = item.file_type()?;
            let kind = if file_type.is_dir() {
                EntryKind::Directory
            } else if file_type.is_symlink() {
                EntryKind::Symlink
            } else {
                EntryKind::Other
            };
            entries.push(Entry {
                name: item.file_name(),
                kind,
            });
        }

        entries.sort_unstable_by(|left, right| left.name.cmp(&right.name));
        Ok(Self { path, entries })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub(crate) fn go(&mut self, path: &Path) -> io::Result<()> {
        if path.as_os_str().is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Enter a path"));
        }

        // Resolve against the displayed directory without moving the process.
        let destination = self.path.join(path);
        *self = Self::open(&destination)?;
        Ok(())
    }

    pub(crate) fn up(&mut self) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            *self = Self::open(parent)?;
        }
        Ok(())
    }
}

impl Entry {
    pub(crate) fn name(&self) -> &OsStr {
        &self.name
    }

    pub(crate) fn kind(&self) -> EntryKind {
        self.kind
    }
}
