use super::Event;
use crate::live::{Change, ChangeKind, RECENT_LIMIT, Update};
use crate::navigation::{Directory, Entry, EntryKind, FoldState, Snapshot, TreeState};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Mode {
    Tree,
    Search,
    Edit,
    Changes,
    Help,
}

#[derive(Default)]
struct Position {
    path: Option<PathBuf>,
    index: usize,
    row: usize,
}

struct Place {
    directory: PathBuf,
    position: Position,
    folds: FoldState,
    max: bool,
}

#[derive(Default)]
pub(super) struct Search {
    pub query: String,
    pub matches: Vec<Entry>,
    pub selected: usize,
    pub scroll: usize,
}

impl Search {
    fn refresh(&mut self, directory: &Directory) {
        let matches = directory.matching_names(&self.query);
        self.selected = self
            .matches
            .get(self.selected)
            .and_then(|old| {
                matches
                    .iter()
                    .position(|entry| entry.relative_path() == old.relative_path())
            })
            .unwrap_or(0);
        self.matches = matches;
    }
}

fn move_cursor(event: &Event, selected: &mut usize, count: usize, page: usize) -> bool {
    let last = count.saturating_sub(1);
    *selected = match event {
        Event::Up => selected.saturating_sub(1),
        Event::Down => (*selected + 1).min(last),
        Event::Home => 0,
        Event::End => last,
        Event::PageUp => selected.saturating_sub(page),
        Event::PageDown => (*selected + page).min(last),
        _ => return false,
    };
    true
}

pub(super) struct Browser {
    pub directory: Directory,
    pub workspace: PathBuf,
    pub selected: usize,
    pub scroll: usize,
    pub viewport: usize,
    pub mode: Mode,
    pub search: Search,
    pub text: String,
    pub diagnostic: String,
    pub live_status: String,
    pub recent: VecDeque<Change>,
    pub latest_files: Vec<PathBuf>,
    editor_badges: VecDeque<Change>,
    pub change_index: usize,
    pub change_scroll: usize,
    pub help_scroll: usize,
    pub request_scan: bool,
    previous: Option<Place>,
    restore: Option<Place>,
    deferred: Option<Arc<Snapshot>>,
}

impl Browser {
    pub fn new(directory: Directory) -> Self {
        Self {
            workspace: directory.path().to_owned(),
            directory,
            selected: 0,
            scroll: 0,
            viewport: 1,
            mode: Mode::Tree,
            search: Search::default(),
            text: String::new(),
            diagnostic: String::new(),
            live_status: "Loading".into(),
            recent: VecDeque::new(),
            latest_files: Vec::new(),
            editor_badges: VecDeque::new(),
            change_index: 0,
            change_scroll: 0,
            help_scroll: 0,
            request_scan: false,
            previous: None,
            restore: None,
            deferred: None,
        }
    }

    pub fn selected_entry(&self) -> Option<&Entry> {
        self.directory.entries().get(self.selected)
    }

    pub fn tree_badges(&self) -> &VecDeque<Change> {
        if self.mode == Mode::Edit {
            &self.editor_badges
        } else {
            &self.recent
        }
    }

    fn begin_edit(&mut self) {
        if self.mode != Mode::Edit {
            self.editor_badges.clone_from(&self.recent);
            self.text.clear();
            self.mode = Mode::Edit;
        }
    }

    fn position(&self) -> Position {
        Position {
            path: self.selected_entry().map(|e| e.relative_path().to_owned()),
            index: self.selected,
            row: self.selected.saturating_sub(self.scroll),
        }
    }

    fn focus(&mut self, position: &Position) {
        let rows = self.directory.entries();
        let index = position.path.as_deref().and_then(|path| {
            path.ancestors()
                .find_map(|ancestor| rows.iter().position(|e| e.relative_path() == ancestor))
        });
        self.selected = index.unwrap_or_else(|| position.index.min(rows.len().saturating_sub(1)));
        self.scroll = self.selected.saturating_sub(position.row);
    }

    fn apply_deferred(&mut self) {
        let Some(snapshot) = self.deferred.take() else {
            return;
        };
        let position = self.position();
        if self.directory.apply(snapshot) {
            if let Some(place) = self.restore.take() {
                if place.max == self.directory.is_max() {
                    self.directory.restore_folds(place.folds);
                }
                self.focus(&place.position);
            } else {
                self.focus(&position);
            }
            if self.mode == Mode::Search {
                self.search.refresh(&self.directory);
            }
        }
    }

    pub fn update(&mut self, update: Update) -> bool {
        let activity = !update.changes.is_empty();
        if activity {
            let focused = self.recent.get(self.change_index).map(|e| e.path.clone());
            for change in update.changes {
                self.recent.retain(|old| old.path != change.path);
                self.recent.push_front(change);
            }
            self.recent.truncate(RECENT_LIMIT);
            if self.mode == Mode::Changes {
                self.change_index = focused
                    .and_then(|path| self.recent.iter().position(|item| item.path == path))
                    .unwrap_or(0);
            }
        }
        if update.root != self.directory.path() {
            return activity;
        }
        self.live_status = update.status;
        self.latest_files = update.latest_files;
        if let Some(snapshot) = update.snapshot {
            self.deferred = Some(snapshot);
            if self.mode != Mode::Edit {
                self.apply_deferred();
            }
        }
        true
    }

    fn travel(&mut self, destination: PathBuf, focus: Option<PathBuf>) {
        let old = Place {
            directory: self.directory.path().to_owned(),
            position: self.position(),
            max: self.directory.is_max(),
            folds: self.directory.folds(),
        };
        if let Err(error) = self.directory.go(&destination) {
            self.diagnostic = error.to_string();
            return;
        }
        let returning = self
            .previous
            .as_ref()
            .is_some_and(|place| place.directory == self.directory.path());
        let mut saved = if returning {
            self.previous.take()
        } else {
            None
        };
        if old.directory == self.directory.path() {
            saved = Some(old);
        } else {
            self.previous = Some(old);
        }
        if let Some(path) = focus {
            let place = saved.get_or_insert_with(|| Place {
                directory: self.directory.path().to_owned(),
                position: Position::default(),
                max: self.directory.is_max(),
                folds: self.directory.folds(),
            });
            place.position.path = Some(path);
        }
        self.selected = 0;
        self.scroll = 0;
        if let Some(place) = &saved {
            self.focus(&place.position);
        }
        self.restore = saved;
        self.deferred = None;
        self.search = Search::default();
        self.latest_files.clear();
        self.mode = Mode::Tree;
        self.diagnostic.clear();
        self.live_status = "Loading".into();
        self.request_scan = true;
    }

    fn parent(&mut self) {
        let current = self.directory.path();
        if let Some(parent) = current.parent() {
            let parent = parent.to_owned();
            let child = current.file_name().map(PathBuf::from);
            self.travel(parent, child);
        }
    }

    fn jump_folder(&mut self, forward: bool) {
        let rows = self.directory.entries();
        let Some(top) = rows
            .get(..=self.selected)
            .and_then(|prefix| prefix.iter().rposition(|e| e.depth() == 0))
        else {
            return;
        };
        let mut folders = rows
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.depth() == 0 && entry.kind() == EntryKind::Directory)
            .map(|(index, _)| index);
        let next = if forward {
            folders
                .clone()
                .find(|&i| i > top)
                .or_else(|| folders.next())
        } else {
            folders
                .clone()
                .rev()
                .find(|&i| i < top)
                .or_else(|| folders.next_back())
        };
        if let Some(index) = next {
            self.selected = index;
        }
    }

    fn reveal_change(&mut self) {
        let Some(change) = self.recent.get(self.change_index) else {
            return;
        };
        let Some(mut parent) = change.path.parent() else {
            return;
        };
        if change.kind == ChangeKind::Removed {
            while !parent.is_dir() {
                let Some(ancestor) = parent.parent() else {
                    break;
                };
                parent = ancestor;
            }
        }
        let focus = change
            .path
            .strip_prefix(parent)
            .ok()
            .and_then(|path| path.components().next())
            .map(|c| PathBuf::from(c.as_os_str()));
        self.travel(parent.to_owned(), focus);
    }

    fn edit(&mut self, event: Event) {
        match event {
            Event::Char(character) => self.text.push(character),
            Event::Backspace => {
                self.text.pop();
            }
            Event::Escape => {
                self.mode = Mode::Tree;
                self.diagnostic.clear();
                self.apply_deferred();
            }
            Event::Enter => self.travel(PathBuf::from(&self.text), None),
            _ => {}
        }
    }

    fn changes(&mut self, event: Event) {
        if move_cursor(
            &event,
            &mut self.change_index,
            self.recent.len(),
            self.viewport,
        ) {
            return;
        }
        match event {
            Event::Char('c') | Event::Escape | Event::Left => self.mode = Mode::Tree,
            Event::Enter | Event::Right => self.reveal_change(),
            _ => {}
        }
    }

    fn begin_search(&mut self, first: Option<char>) {
        self.mode = Mode::Search;
        self.search.query.extend(first);
        self.search.refresh(&self.directory);
    }

    fn open_match(&mut self) {
        let Some(entry) = self.search.matches.get(self.search.selected) else {
            return;
        };
        let path = self.directory.path().join(entry.relative_path());
        if entry.kind() == EntryKind::Directory
            || (entry.kind() == EntryKind::Symlink && path.is_dir())
        {
            self.travel(path, None);
        } else if let Some(parent) = path.parent() {
            self.travel(parent.to_owned(), Some(PathBuf::from(entry.name())));
        }
    }

    fn search_input(&mut self, event: Event) {
        self.diagnostic.clear();
        if move_cursor(
            &event,
            &mut self.search.selected,
            self.search.matches.len(),
            self.viewport,
        ) {
            return;
        }
        match event {
            Event::Char(character) => self.search.query.push(character),
            Event::Backspace => {
                self.search.query.pop();
            }
            Event::Paste(text) if text.chars().any(char::is_control) => {
                self.diagnostic = "Enter a name without control characters".into();
                return;
            }
            Event::Paste(text) => self.search.query.push_str(&text),
            Event::Escape => {
                self.search = Search::default();
                self.mode = Mode::Tree;
                return;
            }
            Event::Enter | Event::Right => {
                self.open_match();
                return;
            }
            _ => return,
        }
        self.search.refresh(&self.directory);
    }

    pub fn handle(&mut self, event: Event) -> bool {
        if matches!(event, Event::Interrupt) {
            return false;
        }
        if self.mode == Mode::Search {
            self.search_input(event);
            return true;
        }
        if let Event::Paste(text) = event {
            self.begin_edit();
            if text.chars().any(char::is_control) {
                self.diagnostic = "Enter a single path without control characters".into();
            } else {
                self.text.push_str(&text);
                self.diagnostic.clear();
            }
            return true;
        }
        if self.mode == Mode::Edit {
            self.edit(event);
            return true;
        }
        if matches!(event, Event::Char('q')) {
            return false;
        }
        self.diagnostic.clear();
        if self.mode == Mode::Changes {
            self.changes(event);
            return true;
        }
        if self.mode == Mode::Help {
            match event {
                Event::Up | Event::PageUp => self.help_scroll = self.help_scroll.saturating_sub(1),
                Event::Down | Event::PageDown => self.help_scroll += 1,
                Event::Escape | Event::Char('?') | Event::Left => self.mode = Mode::Tree,
                _ => {}
            }
            return true;
        }
        // A cursor or fold choice made during loading takes precedence over saved focus.
        if move_cursor(
            &event,
            &mut self.selected,
            self.directory.entries().len(),
            self.viewport,
        ) {
            self.restore = None;
            return true;
        }
        if matches!(
            event,
            Event::NextFolder | Event::PreviousFolder | Event::Char(' ' | 'm')
        ) {
            self.restore = None;
        }
        match event {
            Event::Char('/') => self.begin_search(None),
            Event::Char('g') => self.begin_edit(),
            Event::Char('c') => {
                self.mode = Mode::Changes;
                self.change_index = 0;
                self.change_scroll = 0;
            }
            Event::Char('?') => {
                self.mode = Mode::Help;
                self.help_scroll = 0;
            }
            Event::NextFolder => self.jump_folder(true),
            Event::PreviousFolder => self.jump_folder(false),
            Event::Char('m') => {
                let position = self.position();
                self.directory.toggle_max();
                self.focus(&position);
            }
            Event::Char(' ') => {
                if self
                    .selected_entry()
                    .is_some_and(|entry| entry.state() == TreeState::Unreadable)
                {
                    self.request_scan = true;
                }
                self.directory.toggle(self.selected);
            }
            Event::Char('-') => {
                if let Some(place) = &self.previous {
                    self.travel(place.directory.clone(), None);
                }
            }
            Event::Enter | Event::Right => {
                if let Some(entry) = self.selected_entry() {
                    self.travel(entry.relative_path().to_owned(), None);
                }
            }
            Event::Left | Event::Backspace => self.parent(),
            Event::Char(character) => self.begin_search(Some(character)),
            _ => {}
        }
        true
    }
}
