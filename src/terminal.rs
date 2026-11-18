use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::time::{Duration, Instant};

use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::termios::{OptionalActions, Termios, tcgetattr, tcgetwinsize, tcsetattr};
use unicode_width::UnicodeWidthChar;

use crate::navigation::{Directory, EntryKind};

const PASTE_START: &[u8] = b"\x1b[200~";
const PASTE_END: &[u8] = b"\x1b[201~";

struct Session {
    saved: Termios,
    active: bool,
}

impl Session {
    fn start() -> io::Result<Self> {
        if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "standard input and output must be terminals",
            ));
        }
        let input = io::stdin();
        let saved = tcgetattr(&input)?;
        let mut raw = saved.clone();
        raw.make_raw();
        tcsetattr(&input, OptionalActions::Now, &raw)?;
        let session = Self {
            saved,
            active: true,
        };
        let mut output = io::stdout().lock();
        write!(output, "\x1b[?1049h\x1b[?2004h\x1b[?25l")?;
        output.flush()?;
        Ok(session)
    }

    fn restore(&mut self) -> io::Result<()> {
        if !self.active {
            return Ok(());
        }
        self.active = false;
        let mut output = io::stdout().lock();
        let screen =
            write!(output, "\x1b[?2004l\x1b[?25h\x1b[?1049l").and_then(|()| output.flush());
        let attributes =
            tcsetattr(io::stdin(), OptionalActions::Now, &self.saved).map_err(io::Error::from);
        screen.and(attributes)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

#[derive(Debug)]
enum Event {
    Up,
    Down,
    Left,
    Right,
    Enter,
    Backspace,
    Escape,
    Interrupt,
    Char(char),
    Paste(String),
}

struct Decoder {
    pending: Vec<u8>,
    paste: Option<Vec<u8>>,
    escape_started: Option<Instant>,
}

impl Decoder {
    fn new() -> Self {
        Self {
            pending: Vec::new(),
            paste: None,
            escape_started: None,
        }
    }

    fn next(&mut self) -> Option<Event> {
        if let Some(body) = self.paste.as_mut() {
            if let Some(end) = self
                .pending
                .windows(PASTE_END.len())
                .position(|part| part == PASTE_END)
            {
                body.extend_from_slice(&self.pending[..end]);
                self.pending.drain(..end + PASTE_END.len());
                let body = self.paste.take().unwrap_or_default();
                return Some(Event::Paste(String::from_utf8_lossy(&body).into_owned()));
            }
            let keep = (1..PASTE_END.len())
                .rev()
                .find(|&len| self.pending.ends_with(&PASTE_END[..len]))
                .unwrap_or(0);
            let take = self.pending.len() - keep;
            body.extend(self.pending.drain(..take));
            return None;
        }
        let &first = self.pending.first()?;
        if first == 0x1b {
            if self.pending.len() == 1 {
                let started = self.escape_started.get_or_insert_with(Instant::now);
                if started.elapsed() < Duration::from_millis(80) {
                    return None;
                }
                self.pending.remove(0);
                self.escape_started = None;
                return Some(Event::Escape);
            }
            self.escape_started = None;
            if self.pending[1] != b'[' {
                self.pending.drain(..2);
                return self.next();
            }
            let end = self
                .pending
                .iter()
                .enumerate()
                .skip(2)
                .find(|(_, byte)| (0x40..=0x7e).contains(*byte))
                .map(|(index, _)| index);
            let end = end?;
            let sequence: Vec<u8> = self.pending.drain(..=end).collect();
            if sequence == PASTE_START {
                self.paste = Some(Vec::new());
                return self.next();
            }
            return match sequence.as_slice() {
                b"\x1b[A" => Some(Event::Up),
                b"\x1b[B" => Some(Event::Down),
                b"\x1b[C" => Some(Event::Right),
                b"\x1b[D" => Some(Event::Left),
                _ => self.next(),
            };
        }
        self.escape_started = None;
        let event = match first {
            3 => {
                self.pending.remove(0);
                Event::Interrupt
            }
            b'\r' | b'\n' => {
                self.pending.remove(0);
                Event::Enter
            }
            8 | 127 => {
                self.pending.remove(0);
                Event::Backspace
            }
            0..=31 => {
                self.pending.remove(0);
                return self.next();
            }
            _ => {
                let length = match first {
                    0..=0x7f => 1,
                    0xc2..=0xdf => 2,
                    0xe0..=0xef => 3,
                    0xf0..=0xf4 => 4,
                    _ => 1,
                };
                if self.pending.len() < length {
                    return None;
                }
                let character = std::str::from_utf8(&self.pending[..length])
                    .ok()
                    .and_then(|text| text.chars().next())
                    .unwrap_or('\u{fffd}');
                self.pending.drain(..length);
                Event::Char(character)
            }
        };
        Some(event)
    }
}

struct Browser {
    directory: Directory,
    selected: usize,
    scroll: usize,
    editing: bool,
    text: String,
    diagnostic: String,
}

impl Browser {
    fn new(directory: Directory) -> Self {
        Self {
            directory,
            selected: 0,
            scroll: 0,
            editing: false,
            text: String::new(),
            diagnostic: String::new(),
        }
    }

    fn reset_selection(&mut self) {
        self.selected = 0;
        self.scroll = 0;
    }

    fn navigate(&mut self, path: &Path) {
        match self.directory.go(path) {
            Ok(()) => {
                self.reset_selection();
                self.editing = false;
                self.diagnostic.clear();
            }
            Err(error) => self.diagnostic = error.to_string(),
        }
    }

    fn handle(&mut self, event: Event) -> bool {
        if matches!(event, Event::Interrupt) {
            return false;
        }
        if let Event::Paste(text) = event {
            if !self.editing {
                self.text.clear();
            }
            self.editing = true;
            if text.chars().any(char::is_control) {
                self.diagnostic = "Enter a single path without control characters".into();
            } else {
                self.text.push_str(&text);
                self.diagnostic.clear();
            }
            return true;
        }
        if self.editing {
            match event {
                Event::Char(character) => self.text.push(character),
                Event::Backspace => {
                    self.text.pop();
                }
                Event::Escape => {
                    self.editing = false;
                    self.diagnostic.clear();
                }
                Event::Enter => {
                    let text = self.text.clone();
                    self.navigate(Path::new(&text));
                }
                _ => {}
            }
            return true;
        }
        match event {
            Event::Char('q') => return false,
            Event::Char('g') => {
                self.editing = true;
                self.text.clear();
                self.diagnostic.clear();
            }
            Event::Up => self.selected = self.selected.saturating_sub(1),
            Event::Down => {
                if self.selected + 1 < self.directory.entries().len() {
                    self.selected += 1;
                }
            }
            Event::Enter | Event::Right => {
                if let Some(entry) = self.directory.entries().get(self.selected) {
                    let name = entry.name().to_os_string();
                    self.navigate(Path::new(&name));
                }
            }
            Event::Left | Event::Backspace => match self.directory.up() {
                Ok(()) => {
                    self.reset_selection();
                    self.diagnostic.clear();
                }
                Err(error) => self.diagnostic = error.to_string(),
            },
            _ => {}
        }
        true
    }
}

fn safe_text(text: &str) -> String {
    text.chars()
        .flat_map(|character| {
            if character.is_control() {
                character.escape_default().collect::<String>()
            } else {
                character.to_string()
            }
            .chars()
            .collect::<Vec<_>>()
        })
        .collect()
}

fn clipped(text: &str, width: usize) -> String {
    let mut used = 0;
    text.chars()
        .take_while(|character| {
            let columns = character.width().unwrap_or(0);
            if used + columns > width {
                false
            } else {
                used += columns;
                true
            }
        })
        .collect()
}

fn wrapped(text: &str, width: usize) -> Vec<String> {
    let mut lines = vec![String::new()];
    let mut used = 0;
    for character in text.chars() {
        let columns = character.width().unwrap_or(0);
        if used + columns > width && used > 0 {
            lines.push(String::new());
            used = 0;
        }
        if columns <= width {
            lines.last_mut().expect("one line exists").push(character);
            used += columns;
        }
    }
    lines
}

fn draw(browser: &mut Browser, rows: usize, columns: usize) -> io::Result<()> {
    let path = safe_text(&browser.directory.path().to_string_lossy());
    let header = if columns > 0 {
        wrapped(&path, columns)
    } else {
        Vec::new()
    };
    let mut lines = Vec::new();
    if columns < 4 || rows < header.len() + 3 {
        lines.push(clipped("Resize terminal to continue", columns));
    } else {
        lines.extend(header);
        let viewport = rows - lines.len() - 2;
        if browser.selected < browser.scroll {
            browser.scroll = browser.selected;
        }
        if browser.selected >= browser.scroll + viewport {
            browser.scroll = browser.selected + 1 - viewport;
        }
        for (index, entry) in browser
            .directory
            .entries()
            .iter()
            .enumerate()
            .skip(browser.scroll)
            .take(viewport)
        {
            let marker = if index == browser.selected {
                "> "
            } else {
                "  "
            };
            let suffix = match entry.kind() {
                EntryKind::Directory => "/",
                EntryKind::Symlink => "@",
                EntryKind::Other => "",
            };
            let line = format!(
                "{marker}{}{suffix}",
                safe_text(&entry.name().to_string_lossy())
            );
            lines.push(clipped(&line, columns));
        }
        while lines.len() < rows - 2 {
            lines.push(String::new());
        }
        lines.push(clipped(&safe_text(&browser.diagnostic), columns));
        let footer = if browser.editing {
            format!("Path: {}", safe_text(&browser.text))
        } else {
            "↑/↓ select  Enter open  ← parent  g path  q quit".to_string()
        };
        lines.push(clipped(&footer, columns));
    }
    let mut frame = String::new();
    for index in 0..rows {
        frame.push_str(&format!("\x1b[{};1H\x1b[2K", index + 1));
        if let Some(line) = lines.get(index) {
            frame.push_str(line);
        }
    }
    let mut output = io::stdout().lock();
    output.write_all(frame.as_bytes())?;
    output.flush()
}

pub(crate) fn run(directory: Directory) -> io::Result<()> {
    let mut session = Session::start()?;
    let result = run_session(directory);
    let cleanup = session.restore();
    result.and(cleanup)
}

fn run_session(directory: Directory) -> io::Result<()> {
    let input = io::stdin();
    let mut browser = Browser::new(directory);
    let mut decoder = Decoder::new();
    let timeout = Timespec {
        tv_sec: 0,
        tv_nsec: 50_000_000,
    };
    let mut previous_size = None;
    let mut redraw = true;
    loop {
        let size = tcgetwinsize(&input)?;
        let current_size = (usize::from(size.ws_row), usize::from(size.ws_col));
        if redraw || previous_size != Some(current_size) {
            draw(&mut browser, current_size.0, current_size.1)?;
            previous_size = Some(current_size);
            redraw = false;
        }
        while let Some(event) = decoder.next() {
            if !browser.handle(event) {
                return Ok(());
            }
            redraw = true;
        }
        if redraw {
            continue;
        }
        let mut fds = [PollFd::new(&input, PollFlags::IN)];
        match poll(&mut fds, Some(&timeout)) {
            Ok(0) => continue,
            Ok(_) => {
                let mut bytes = [0; 4096];
                match rustix::io::read(&input, &mut bytes) {
                    Ok(0) => {
                        return Err(io::Error::new(
                            io::ErrorKind::UnexpectedEof,
                            "terminal input closed",
                        ));
                    }
                    Ok(count) => decoder.pending.extend_from_slice(&bytes[..count]),
                    Err(error) if error == rustix::io::Errno::INTR => continue,
                    Err(error) => return Err(error.into()),
                }
            }
            Err(error) if error == rustix::io::Errno::INTR => continue,
            Err(error) => return Err(error.into()),
        }
    }
}
