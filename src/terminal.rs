use std::env;
use std::io::{self, IsTerminal, Write};
use std::time::{Duration, Instant};

use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::termios::{OptionalActions, Termios, tcgetattr, tcgetwinsize, tcsetattr};

mod browser;
mod view;

use crate::live::Live;
use crate::navigation::Directory;
use browser::Browser;

const PASTE_START: &[u8] = b"\x1b[200~";
const PASTE_END: &[u8] = b"\x1b[201~";

struct Palette {
    path: &'static str,
    directory: &'static str,
    selection: &'static str,
    editor: &'static str,
    link: &'static str,
    error: &'static str,
    edited: &'static str,
    quiet: &'static str,
}

impl Palette {
    const PLAIN: Self = Self {
        path: "",
        directory: "",
        selection: "",
        editor: "",
        link: "",
        error: "",
        edited: "",
        quiet: "",
    };

    const CYAN: Self = Self {
        path: "\x1b[1;36m",
        directory: "\x1b[36m",
        selection: "\x1b[1;7m",
        editor: "\x1b[1;32m",
        link: "\x1b[35m",
        error: "\x1b[1;31m",
        edited: "\x1b[1;33m",
        quiet: "\x1b[2m",
    };

    const GREEN: Self = Self {
        path: "\x1b[1;32m",
        directory: "\x1b[32m",
        selection: "\x1b[1;30;42m",
        ..Self::CYAN
    };

    const BLUE: Self = Self {
        path: "\x1b[1;94m",
        directory: "\x1b[94m",
        selection: "\x1b[1;97;44m",
        editor: "\x1b[1;94m",
        ..Self::CYAN
    };

    const MIXED: Self = Self {
        directory: Self::GREEN.directory,
        selection: Self::BLUE.selection,
        ..Self::CYAN
    };

    fn from_environment() -> Self {
        if env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty())
            || env::var("TERM").as_deref() == Ok("dumb")
        {
            return Self::PLAIN;
        }

        match env::var("FOLDERWALK_THEME").as_deref() {
            Ok("green") => Self::GREEN,
            Ok("blue") => Self::BLUE,
            Ok("mixed") => Self::MIXED,
            _ => Self::CYAN,
        }
    }
}

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
    NextFolder,
    PreviousFolder,
    PageUp,
    PageDown,
    Home,
    End,
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
                b"\x1b[Z" => Some(Event::PreviousFolder),
                b"\x1b[5~" => Some(Event::PageUp),
                b"\x1b[6~" => Some(Event::PageDown),
                b"\x1b[H" | b"\x1b[1~" => Some(Event::Home),
                b"\x1b[F" | b"\x1b[4~" => Some(Event::End),
                _ => self.next(),
            };
        }
        self.escape_started = None;
        let event = match first {
            b'\t' => {
                self.pending.remove(0);
                Event::NextFolder
            }
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

pub(crate) fn run(directory: Directory) -> io::Result<()> {
    let mut session = Session::start()?;
    let result = run_session(directory);
    let cleanup = session.restore();
    result.and(cleanup)
}

fn run_session(directory: Directory) -> io::Result<()> {
    let input = io::stdin();
    let palette = Palette::from_environment();
    let live = Live::new(directory.path())?;
    let mut browser = Browser::new(directory);
    let mut decoder = Decoder::new();
    let timeout = Timespec {
        tv_sec: 0,
        tv_nsec: 50_000_000,
    };
    let mut previous_size = None;
    let mut redraw = true;
    loop {
        if let Some(update) = live.try_update() {
            redraw |= browser.update(update);
        }
        let size = tcgetwinsize(&input)?;
        let current_size = (usize::from(size.ws_row), usize::from(size.ws_col));
        if redraw || previous_size != Some(current_size) {
            view::draw(&mut browser, current_size.0, current_size.1, &palette)?;
            previous_size = Some(current_size);
            redraw = false;
        }
        while let Some(event) = decoder.next() {
            if !browser.handle(event) {
                return Ok(());
            }
            redraw = true;
        }
        if std::mem::take(&mut browser.request_scan) {
            live.show(browser.directory.path());
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
