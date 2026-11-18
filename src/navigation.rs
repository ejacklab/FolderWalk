use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const MAX_TREE_DEPTH: usize = 3;

pub(crate) type FoldState = HashMap<PathBuf, bool>;

pub(crate) struct Directory {
    snapshot: Arc<Snapshot>,
    visible: Vec<Entry>,
    max: bool,
    expanded: FoldState,
}

#[derive(PartialEq, Eq)]
pub(crate) struct Snapshot {
    path: PathBuf,
    entries: Vec<Entry>,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Entry {
    data: Arc<EntryData>,
    depth: usize,
    state: TreeState,
}

#[derive(PartialEq, Eq)]
struct EntryData {
    relative_path: PathBuf,
    kind: EntryKind,
    stamp: Option<Stamp>,
}

#[derive(PartialEq, Eq)]
pub(crate) struct Stamp {
    device: u64,
    inode: u64,
    mode: u32,
    file: Option<(u64, i64, i64, i64, i64)>,
}

impl Stamp {
    fn from_metadata(metadata: &fs::Metadata) -> Self {
        // Child creation changes directory times. The child gets the activity row.
        let file = (!metadata.is_dir()).then(|| {
            (
                metadata.len(),
                metadata.mtime(),
                metadata.mtime_nsec(),
                metadata.ctime(),
                metadata.ctime_nsec(),
            )
        });
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            mode: metadata.mode(),
            file,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum EntryKind {
    Directory,
    Symlink,
    Other,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum TreeState {
    Closed,
    Expanded,
    Empty,
    Unreadable,
    Limit,
    Leaf,
}

fn read_entries(
    directory: &Path,
    parent: &Path,
    depth: usize,
    with_stamps: bool,
) -> io::Result<Vec<Entry>> {
    let mut entries = Vec::new();
    for item in fs::read_dir(directory)? {
        let item = match item {
            Ok(item) => item,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        let metadata = if with_stamps {
            match item.metadata() {
                Ok(metadata) => Some(metadata),
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(_) => None,
            }
        } else {
            None
        };
        let file_type = match metadata.as_ref().map(fs::Metadata::file_type) {
            Some(file_type) => file_type,
            None => match item.file_type() {
                Ok(file_type) => file_type,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error),
            },
        };
        let (kind, state) = if file_type.is_dir() {
            let state = if depth == MAX_TREE_DEPTH {
                TreeState::Limit
            } else {
                TreeState::Closed
            };
            (EntryKind::Directory, state)
        } else if file_type.is_symlink() {
            (EntryKind::Symlink, TreeState::Leaf)
        } else {
            (EntryKind::Other, TreeState::Leaf)
        };
        entries.push(Entry {
            data: Arc::new(EntryData {
                relative_path: parent.join(item.file_name()),
                kind,
                stamp: metadata.as_ref().map(Stamp::from_metadata),
            }),
            depth,
            state,
        });
    }
    entries.sort_unstable_by(|left, right| left.name().cmp(right.name()));
    Ok(entries)
}

impl Snapshot {
    pub(crate) fn scan(root: &Path, mut before_read: impl FnMut(&Path)) -> io::Result<Self> {
        let mut snapshot = Self {
            path: root.to_path_buf(),
            entries: Vec::new(),
        };
        snapshot.scan_children(Path::new(""), 0, &mut before_read)?;
        Ok(snapshot)
    }

    fn scan_children(
        &mut self,
        parent: &Path,
        depth: usize,
        before_read: &mut impl FnMut(&Path),
    ) -> io::Result<TreeState> {
        // Pass the root unchanged. A trailing slash can cause lstat to follow a symlink.
        let directory = if parent.as_os_str().is_empty() {
            self.path.clone()
        } else {
            self.path.join(parent)
        };
        if !fs::symlink_metadata(&directory)?.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotADirectory,
                "Folder changed",
            ));
        }
        before_read(&directory);
        let children = read_entries(&directory, parent, depth, true)?;
        let start = self.entries.len();
        for child in children {
            let index = self.entries.len();
            self.entries.push(child);
            if self.entries[index].state == TreeState::Closed {
                let child_path = self.entries[index].relative_path().to_owned();
                match self.scan_children(&child_path, depth + 1, before_read) {
                    Ok(state) => self.entries[index].state = state,
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {
                        self.entries.pop();
                    }
                    Err(_) => self.entries[index].state = TreeState::Unreadable,
                }
            }
        }
        if self.entries.len() == start {
            Ok(TreeState::Empty)
        } else {
            Ok(TreeState::Expanded)
        }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn entries(&self) -> &[Entry] {
        &self.entries
    }
}

impl Directory {
    pub(crate) fn open(path: &Path) -> io::Result<Self> {
        let path = fs::canonicalize(path)?;
        let entries = read_entries(&path, Path::new(""), 0, false)?;
        let visible = entries.clone();
        Ok(Self {
            snapshot: Arc::new(Snapshot { path, entries }),
            visible,
            max: true,
            expanded: HashMap::new(),
        })
    }

    pub(crate) fn apply(&mut self, snapshot: Arc<Snapshot>) -> bool {
        if snapshot.path() != self.path() || Arc::ptr_eq(&self.snapshot, &snapshot) {
            return false;
        }
        self.snapshot = snapshot;
        self.project();
        true
    }

    fn project(&mut self) {
        self.visible.clear();
        let mut hidden_below = None;
        // Rebuild from surviving folder paths so a deleted folder leaves no fold preference.
        let mut previous = std::mem::take(&mut self.expanded);
        for entry in self.snapshot.entries() {
            let preference =
                previous
                    .remove_entry(entry.relative_path())
                    .and_then(|(path, open)| {
                        if entry.kind() == EntryKind::Directory {
                            self.expanded.insert(path, open);
                            Some(open)
                        } else {
                            None
                        }
                    });
            if hidden_below.is_some_and(|depth| entry.depth > depth) {
                continue;
            }
            hidden_below = None;
            let mut row = entry.clone();
            if matches!(row.state, TreeState::Expanded | TreeState::Empty)
                && !preference.unwrap_or(self.max || row.depth > 0)
            {
                row.state = TreeState::Closed;
                hidden_below = Some(row.depth);
            }
            self.visible.push(row);
        }
    }

    pub(crate) fn toggle(&mut self, index: usize) {
        if let Some(entry) = self.visible.get(index) {
            let open = match entry.state {
                TreeState::Closed | TreeState::Unreadable => true,
                TreeState::Expanded | TreeState::Empty => false,
                TreeState::Limit | TreeState::Leaf => return,
            };
            self.expanded.insert(entry.relative_path().to_owned(), open);
            self.project();
        }
    }

    pub(crate) fn folds(&self) -> FoldState {
        self.expanded.clone()
    }

    pub(crate) fn restore_folds(&mut self, folds: FoldState) {
        self.expanded = folds;
        self.project();
    }

    pub(crate) fn toggle_max(&mut self) {
        self.max = !self.max;
        self.expanded.clear();
        self.project();
    }

    pub(crate) fn is_max(&self) -> bool {
        self.max
    }

    pub(crate) fn path(&self) -> &Path {
        self.snapshot.path()
    }

    pub(crate) fn entries(&self) -> &[Entry] {
        &self.visible
    }

    pub(crate) fn go(&mut self, path: &Path) -> io::Result<()> {
        if path.as_os_str().is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Enter a path"));
        }
        let mut next = Self::open(&self.path().join(path))?;
        next.max = self.max;
        *self = next;
        Ok(())
    }
}

impl Entry {
    pub(crate) fn name(&self) -> &OsStr {
        self.relative_path().file_name().unwrap_or_default()
    }

    pub(crate) fn depth(&self) -> usize {
        self.depth
    }

    pub(crate) fn state(&self) -> TreeState {
        self.state
    }

    pub(crate) fn relative_path(&self) -> &Path {
        &self.data.relative_path
    }

    pub(crate) fn kind(&self) -> EntryKind {
        self.data.kind
    }

    pub(crate) fn stamp(&self) -> Option<&Stamp> {
        self.data.stamp.as_ref()
    }
}
