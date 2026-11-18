use crate::navigation::{Entry, EntryKind, Snapshot, TreeState};
use rustix::fd::OwnedFd;
use rustix::fs::inotify::{self, CreateFlags, ReadFlags, WatchFlags};
use rustix::io::Errno;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::ffi::OsStr;
use std::io;
use std::mem::MaybeUninit;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

pub(crate) const RECENT_LIMIT: usize = 64;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChangeKind {
    Added,
    Edited,
    Removed,
}

#[derive(Clone)]
pub(crate) struct Change {
    pub(crate) path: PathBuf,
    pub(crate) kind: ChangeKind,
}

pub(crate) struct Update {
    pub(crate) root: PathBuf,
    pub(crate) snapshot: Option<Arc<Snapshot>>,
    pub(crate) changes: Vec<Change>,
    pub(crate) status: String,
}

pub(crate) struct Live {
    requests: Sender<PathBuf>,
    pending: Arc<Mutex<Option<Update>>>,
}

impl Live {
    pub(crate) fn new(anchor: &Path) -> io::Result<Self> {
        let anchor = anchor.to_owned();
        let (requests, receiver) = mpsc::channel();
        let pending = Arc::new(Mutex::new(None));
        let output = Arc::clone(&pending);
        thread::Builder::new()
            .name("folderwalk-monitor".into())
            .spawn(move || monitor(anchor, receiver, &output))?;
        Ok(Self { requests, pending })
    }

    pub(crate) fn show(&self, path: &Path) {
        let _ = self.requests.send(path.to_owned());
    }

    pub(crate) fn try_update(&self) -> Option<Update> {
        self.pending.try_lock().ok()?.take()
    }
}

struct Watches {
    fd: Option<OwnedFd>,
    paths: HashMap<i32, PathBuf>,
}

impl Watches {
    fn new() -> Self {
        Self {
            fd: inotify::init(CreateFlags::CLOEXEC | CreateFlags::NONBLOCK).ok(),
            paths: HashMap::new(),
        }
    }

    fn register(&self, path: &Path) -> Option<i32> {
        let flags = WatchFlags::CREATE
            | WatchFlags::DELETE
            | WatchFlags::MOVE
            | WatchFlags::MODIFY
            | WatchFlags::CLOSE_WRITE
            | WatchFlags::ATTRIB
            | WatchFlags::DELETE_SELF
            | WatchFlags::MOVE_SELF
            | WatchFlags::DONT_FOLLOW
            | WatchFlags::ONLYDIR;
        inotify::add_watch(self.fd.as_ref()?, path, flags).ok()
    }

    fn replace(&mut self, paths: HashMap<i32, PathBuf>) {
        if let Some(fd) = &self.fd {
            for wd in self.paths.keys().filter(|wd| !paths.contains_key(wd)) {
                let _ = inotify::remove_watch(fd, *wd);
            }
        }
        self.paths = paths;
    }

    fn events(&mut self, writes: &mut HashSet<PathBuf>) -> bool {
        let Some(fd) = &self.fd else { return false };
        let mut buffer = [MaybeUninit::uninit(); 8192];
        let mut reader = inotify::Reader::new(fd, &mut buffer);
        let mut dirty = false;
        let mut count = 0;
        loop {
            let event = match reader.next() {
                Ok(event) => event,
                Err(Errno::INTR) => continue,
                Err(Errno::AGAIN) => break,
                Err(_) => {
                    self.fd = None;
                    self.paths.clear();
                    return true;
                }
            };
            let flags = event.events();
            if flags.contains(ReadFlags::QUEUE_OVERFLOW) {
                dirty = true;
            } else if let Some(parent) = self.paths.get(&event.wd()) {
                dirty = true;
                if flags.intersects(ReadFlags::MODIFY | ReadFlags::MOVED_TO)
                    && !flags.contains(ReadFlags::ISDIR)
                    && let Some(name) = event.file_name()
                {
                    writes.insert(parent.join(OsStr::from_bytes(name.to_bytes())));
                }
            }
            count += 1;
            // Finish the current kernel buffer before yielding to UI requests.
            if count >= 1024 && reader.is_buffer_empty() {
                break;
            }
        }
        dirty
    }
}

fn indexed(snapshot: &Snapshot) -> HashMap<&Path, &Entry> {
    snapshot
        .entries()
        .iter()
        .map(|entry| (entry.relative_path(), entry))
        .collect()
}

fn inaccessible(path: &Path, entries: &HashMap<&Path, &Entry>) -> bool {
    path.ancestors().skip(1).any(|parent| {
        entries
            .get(parent)
            .is_some_and(|entry| entry.state() == TreeState::Unreadable)
    })
}

fn differences(
    old: &Snapshot,
    new: &Snapshot,
    writes: &HashSet<PathBuf>,
    changes: &mut BTreeMap<PathBuf, ChangeKind>,
) {
    let before = indexed(old);
    let after = indexed(new);
    for (path, entry) in &after {
        let absolute = new.path().join(path);
        let kind = match before.get(path) {
            Some(previous)
                if previous.kind() != entry.kind()
                    || (previous.stamp().is_some()
                        && entry.stamp().is_some()
                        && previous.stamp() != entry.stamp())
                    || (entry.kind() != EntryKind::Directory && writes.contains(&absolute)) =>
            {
                ChangeKind::Edited
            }
            None if !inaccessible(path, &before) => ChangeKind::Added,
            _ => continue,
        };
        changes.insert(absolute, kind);
    }
    for path in before.keys() {
        if !after.contains_key(path) && !inaccessible(path, &after) {
            changes.insert(new.path().join(path), ChangeKind::Removed);
        }
    }
}

fn monitor(anchor: PathBuf, requests: Receiver<PathBuf>, output: &Mutex<Option<Update>>) {
    let mut view = anchor.clone();
    let mut watches = Watches::new();
    let mut previous: HashMap<PathBuf, Arc<Snapshot>> = HashMap::new();
    let mut last_status = String::new();
    let mut next_scan = Instant::now();
    let mut force = true;
    loop {
        let mut writes = HashSet::new();
        let invalidated = watches.events(&mut writes);
        if force || invalidated || Instant::now() >= next_scan {
            let mut paths = HashMap::new();
            let mut visited = HashSet::new();
            let mut polling = watches.fd.is_none();
            let mut anchor_available = false;
            let mut snapshot = None;
            let mut changes = BTreeMap::new();
            let mut view_changed = false;
            previous.retain(|path, _| path == &anchor || path == &view);
            for root in std::iter::once(&anchor).chain((view != anchor).then_some(&view)) {
                let scanned = Snapshot::scan(root, |path| {
                    if visited.insert(path.to_owned()) {
                        if let Some(wd) = watches.register(path) {
                            paths.insert(wd, path.to_owned());
                        } else {
                            polling = true;
                        }
                    }
                });
                if let Ok(new) = scanned {
                    let old = previous.get(root);
                    let changed = old.is_none_or(|old| **old != new);
                    if let Some(old) = old
                        && (changed || !writes.is_empty())
                    {
                        differences(old, &new, &writes, &mut changes);
                    }
                    let current = if changed {
                        let new = Arc::new(new);
                        previous.insert(root.clone(), Arc::clone(&new));
                        new
                    } else {
                        Arc::clone(&previous[root])
                    };
                    if root == &anchor {
                        anchor_available = true;
                    }
                    if root == &view {
                        view_changed = changed;
                        snapshot = Some(current);
                    }
                }
            }
            watches.replace(paths);
            let status = if snapshot.is_none() {
                "Unavailable"
            } else if !anchor_available {
                "Live · workspace unavailable"
            } else if polling {
                "Polling"
            } else {
                "Live"
            };
            if force || view_changed || !changes.is_empty() || status != last_status {
                let mut slot = output.lock().expect("monitor update lock poisoned");
                let mut history = slot.take().map(|u| u.changes).unwrap_or_default();
                history.retain(|item| !changes.contains_key(&item.path));
                let overflow = (history.len() + changes.len()).saturating_sub(RECENT_LIMIT);
                let skip_old = overflow.min(history.len());
                history.drain(..skip_old);
                history.extend(
                    changes
                        .into_iter()
                        .skip(overflow - skip_old)
                        .map(|(path, kind)| Change { path, kind }),
                );
                *slot = Some(Update {
                    root: view.clone(),
                    snapshot,
                    changes: history,
                    status: status.into(),
                });
                last_status = status.into();
            }
            next_scan = Instant::now()
                + if polling || !anchor_available || status == "Unavailable" {
                    Duration::from_millis(500)
                } else {
                    Duration::from_secs(5)
                };
            force = false;
        }
        match requests.recv_timeout(Duration::from_millis(80)) {
            Ok(path) => {
                view = requests.try_iter().last().unwrap_or(path);
                force = true;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}
