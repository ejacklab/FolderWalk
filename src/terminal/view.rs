use super::Palette;
use super::browser::{Browser, Mode};
use crate::live::{Change, ChangeKind, LATEST_FILE_LIMIT};
use crate::navigation::{EntryKind, TreeState};
use std::io::{self, Write};
use std::path::Path;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

type Line = (String, &'static str);

const KEYS: &[&str] = &[
    "↑ ↓           Move one row",
    "/ or type     Find a file or folder by name",
    "In search     ↑↓ select · Enter opens or reveals · Esc returns",
    "Tab / ⇧Tab    Next / previous top-level folder",
    "PgUp / PgDn   Move one screen",
    "Home / End    First / last row",
    "Enter or →    Open selected directory",
    "← or ⌫        Go up and select the folder you left",
    "-             Return to your previous location",
    "Space         Fold or expand one folder",
    "m             Switch between Max and Compact",
    "c             Recent changes · Enter reveals a path",
    "g or paste    Enter a directory path",
    "q or Ctrl+C   Quit browsing",
];

fn safe_text(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        if character.is_control() {
            escaped.extend(character.escape_default());
        } else {
            escaped.push(character);
        }
    }
    escaped
}

fn clipped(text: &str, width: usize) -> String {
    let mut used = 0;
    text.chars()
        .take_while(|character| {
            used += character.width().unwrap_or(0);
            used <= width
        })
        .collect()
}

fn text_label(prefix: &str, text: &str, width: usize) -> String {
    let text = safe_text(text);
    let room = width.saturating_sub(prefix.width());
    if text.width() <= room {
        return format!("{prefix}{text}");
    }
    let mut used = 1;
    let suffix = text
        .chars()
        .rev()
        .take_while(|ch| {
            used += ch.width().unwrap_or(0);
            used <= room
        })
        .collect::<Vec<_>>();
    format!("{prefix}…{}", suffix.into_iter().rev().collect::<String>())
}

fn path_label(prefix: &str, path: &Path, width: usize) -> String {
    text_label(prefix, &path.to_string_lossy(), width)
}

fn name_with_parent(path: &Path, suffix: &str, width: usize) -> String {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let mut label = text_label("", &name, width.saturating_sub(suffix.width()));
    label.push_str(suffix);
    let room = width.saturating_sub(label.width());
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
        && room > 5
    {
        label.push_str(&path_label("  ·  ", parent, room));
    }
    label
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

fn keep_visible(scroll: usize, first: usize, last: usize, height: usize, count: usize) -> usize {
    scroll
        .min(first)
        .max((last + 1).saturating_sub(height))
        .min(count.saturating_sub(height))
}

fn change_label(kind: ChangeKind) -> &'static str {
    match kind {
        ChangeKind::Added => "+ Added",
        ChangeKind::Edited => "~ Edited",
        ChangeKind::Removed => "− Removed",
    }
}

fn change_style(kind: ChangeKind, palette: &Palette) -> &'static str {
    match kind {
        ChangeKind::Added => palette.editor,
        ChangeKind::Edited => palette.edited,
        ChangeKind::Removed => palette.error,
    }
}

fn entry_style(kind: EntryKind, palette: &Palette) -> (&'static str, &'static str) {
    match kind {
        EntryKind::Directory => ("/", palette.directory),
        EntryKind::Symlink => ("@", palette.link),
        EntryKind::File | EntryKind::Other => ("", ""),
    }
}

fn tree_lines(browser: &mut Browser, palette: &Palette, lines: &mut Vec<Line>) {
    let entries = browser.directory.entries();
    if entries.is_empty() {
        lines.push(("Empty directory".into(), palette.quiet));
        return;
    }
    let show_child =
        browser.viewport > 1 && entries[browser.selected].state() == TreeState::Expanded;
    browser.scroll = keep_visible(
        browser.scroll,
        browser.selected,
        browser.selected + usize::from(show_child),
        browser.viewport,
        entries.len(),
    );
    for (index, entry) in entries
        .iter()
        .enumerate()
        .skip(browser.scroll)
        .take(browser.viewport)
    {
        let (suffix, style) = entry_style(entry.kind(), palette);
        let (disclosure, detail) = match entry.state() {
            TreeState::Closed => ("▸ ", ""),
            TreeState::Expanded => ("▾ ", ""),
            TreeState::Empty => ("▾ ", "  (empty)"),
            TreeState::Unreadable => ("▸ ", "  (unavailable)"),
            TreeState::Limit => ("· ", ""),
            TreeState::Leaf => ("  ", ""),
        };
        let focused = index == browser.selected;
        let marker = if focused { "> " } else { "  " };
        let indent = "  ".repeat(entry.depth());
        let name = safe_text(&entry.name().to_string_lossy());
        let mut line = format!("{marker}{indent}{disclosure}{name}{suffix}{detail}");
        let change = browser.tree_badges().iter().find(|change| {
            change.path.strip_prefix(browser.directory.path()) == Ok(entry.relative_path())
        });
        let mut style = style;
        if let Some(change) = change {
            line.push_str("   ");
            line.push_str(change_label(change.kind));
            style = change_style(change.kind, palette);
        }
        lines.push((line, if focused { palette.selection } else { style }));
    }
}

fn search_lines(browser: &mut Browser, width: usize, palette: &Palette, lines: &mut Vec<Line>) {
    let search = &mut browser.search;
    if search.matches.is_empty() {
        lines.push(("No matching names".into(), palette.quiet));
        return;
    }
    search.scroll = keep_visible(
        search.scroll,
        search.selected,
        search.selected,
        browser.viewport,
        search.matches.len(),
    );
    for (index, entry) in search
        .matches
        .iter()
        .enumerate()
        .skip(search.scroll)
        .take(browser.viewport)
    {
        let focused = index == search.selected;
        let marker = if focused { "> " } else { "  " };
        let disclosure = if entry.kind() == EntryKind::Directory {
            "▸ "
        } else {
            "  "
        };
        let (suffix, style) = entry_style(entry.kind(), palette);
        let label = name_with_parent(entry.relative_path(), suffix, width.saturating_sub(4));
        lines.push((
            format!("{marker}{disclosure}{label}"),
            if focused { palette.selection } else { style },
        ));
    }
}

fn latest_lines(
    browser: &Browser,
    height: usize,
    width: usize,
    palette: &Palette,
    lines: &mut Vec<Line>,
) {
    let heading = "Recently modified";
    let rule = "─".repeat(width.saturating_sub(heading.width() + 2));
    lines.push((format!("{heading}  {rule}"), palette.path));
    if browser.latest_files.is_empty() {
        let message = if browser.live_status == "Loading" {
            "Reading files…"
        } else {
            "No files to show"
        };
        lines.push((format!("│ {message}"), palette.quiet));
    } else {
        for path in browser.latest_files.iter().take(height - 1) {
            let relative = path.strip_prefix(&browser.workspace).unwrap_or(path);
            let label = name_with_parent(relative, "", width.saturating_sub(2));
            lines.push((format!("│ {label}"), ""));
        }
    }
}

fn relative<'a>(change: &'a Change, workspace: &Path) -> &'a Path {
    change.path.strip_prefix(workspace).unwrap_or(&change.path)
}

fn change_lines(browser: &mut Browser, width: usize, palette: &Palette, lines: &mut Vec<Line>) {
    if browser.recent.is_empty() {
        lines.push(("Waiting for a file change…".into(), palette.quiet));
        return;
    }
    browser.change_scroll = keep_visible(
        browser.change_scroll,
        browser.change_index,
        browser.change_index,
        browser.viewport,
        browser.recent.len(),
    );
    for (index, change) in browser
        .recent
        .iter()
        .enumerate()
        .skip(browser.change_scroll)
        .take(browser.viewport)
    {
        let focused = index == browser.change_index;
        let marker = if focused { "> " } else { "  " };
        let prefix = format!("{marker}{:<11}", change_label(change.kind));
        let text = path_label(&prefix, relative(change, &browser.workspace), width);
        lines.push((
            text,
            if focused {
                palette.selection
            } else {
                change_style(change.kind, palette)
            },
        ));
    }
}

fn activity_line(browser: &Browser, width: usize, palette: &Palette) -> Line {
    let status = &browser.live_status;
    if let Some(change) = browser.recent.front() {
        let prefix = format!("{status}   {}  ", change_label(change.kind));
        return (
            path_label(&prefix, relative(change, &browser.workspace), width),
            change_style(change.kind, palette),
        );
    }
    let scope = browser
        .workspace
        .file_name()
        .map(Path::new)
        .unwrap_or(&browser.workspace);
    (
        path_label(&format!("{status}   Workspace  "), scope, width),
        palette.quiet,
    )
}

fn caption(browser: &Browser) -> String {
    match browser.mode {
        Mode::Search => {
            let count = browser.search.matches.len();
            let unit = if count == 1 { "match" } else { "matches" };
            format!("Name search   ·   {count} {unit}")
        }
        Mode::Changes => {
            let count = browser.recent.len();
            let unit = if count == 1 { "path" } else { "paths" };
            format!("Recent changes   ·   {count} {unit}")
        }
        Mode::Help => "Keyboard".into(),
        Mode::Tree | Mode::Edit => {
            let mode = if browser.directory.is_max() {
                "Max tree · 3 levels"
            } else {
                "Compact tree"
            };
            let count = browser.directory.entries().len();
            let position = if count == 0 { 0 } else { browser.selected + 1 };
            format!("{mode}     {position} / {count}")
        }
    }
}

fn hint(browser: &Browser, width: usize) -> String {
    match browser.mode {
        Mode::Search => match browser.search.matches.get(browser.search.selected) {
            None => "Backspace edit   Esc back",
            Some(entry) if entry.kind() == EntryKind::Directory => {
                "↑↓ select   Enter open folder   Esc back"
            }
            Some(entry) if entry.kind() == EntryKind::Symlink => {
                "↑↓ select   Enter open or reveal   Esc back"
            }
            Some(_) => "↑↓ select   Enter reveal file   Esc back",
        }
        .into(),
        Mode::Edit => "Enter go   Esc cancel".into(),
        Mode::Help => "↑↓ scroll   Esc tree".into(),
        Mode::Changes => path_label("Watching  ", &browser.workspace, width),
        Mode::Tree => {
            let action = match browser.selected_entry().map(|e| e.state()) {
                Some(TreeState::Closed) => "Space expand",
                Some(TreeState::Expanded | TreeState::Empty) => "Space fold",
                Some(TreeState::Limit) => "Level 3 · Enter to see more",
                Some(TreeState::Unreadable) => "Space retry",
                _ => "",
            };
            let mode = if browser.directory.is_max() {
                "m compact"
            } else {
                "m max tree"
            };
            if action.is_empty() {
                mode.into()
            } else {
                format!("{action}   {mode}")
            }
        }
    }
}

fn footer(browser: &Browser, width: usize) -> String {
    match browser.mode {
        Mode::Changes => "↑↓ move   Enter reveal   Esc tree   q quit".into(),
        Mode::Help => "? or Esc returns to the tree".into(),
        Mode::Edit => text_label("Path: ", &browser.text, width.saturating_sub(1)),
        Mode::Search => text_label("Find: ", &browser.search.query, width.saturating_sub(1)),
        Mode::Tree if width >= 90 => {
            "↑↓ move  Type or / search  Tab folder  Enter open  ← up  c changes  g path  ? keys  q quit".into()
        }
        Mode::Tree if width >= 74 => "↑↓ move  / search  Tab folder  ↵ open  ← up  c changes  ? keys  q quit".into(),
        Mode::Tree if width >= 50 => "↑↓ move  / search  ↵ open  c changes  ? keys".into(),
        Mode::Tree => "↑↓ move  / search  ↵ open  ? keys".into(),
    }
}

pub(super) fn draw(
    browser: &mut Browser,
    rows: usize,
    columns: usize,
    palette: &Palette,
) -> io::Result<()> {
    let path = safe_text(&browser.directory.path().to_string_lossy());
    let mut lines: Vec<Line> = wrapped(&path, columns)
        .into_iter()
        .map(|line| (line, palette.path))
        .collect();
    let available = rows.saturating_sub(lines.len());
    let mut input_cursor = None;
    if columns < 4 || available < 3 {
        lines = vec![("Resize terminal to continue".into(), "")];
    } else {
        let strip = usize::from(available >= 5);
        let title = usize::from(available >= 6);
        let content_height = available - 2 - strip - title;
        let mut panel = 0;
        if matches!(browser.mode, Mode::Tree | Mode::Search | Mode::Edit) {
            // Let the file list use at most half the available rows on a short screen.
            panel = (content_height / 2).min(LATEST_FILE_LIMIT + 1);
            if panel < 2 {
                panel = 0;
            }
        }
        browser.viewport = content_height - panel;
        if title > 0 {
            lines.push((caption(browser), palette.quiet));
        }
        match browser.mode {
            Mode::Tree | Mode::Edit => tree_lines(browser, palette, &mut lines),
            Mode::Search => search_lines(browser, columns, palette, &mut lines),
            Mode::Changes => change_lines(browser, columns, palette, &mut lines),
            Mode::Help => {
                browser.help_scroll = browser
                    .help_scroll
                    .min(KEYS.len().saturating_sub(browser.viewport));
                lines.extend(
                    KEYS.iter()
                        .skip(browser.help_scroll)
                        .take(browser.viewport)
                        .map(|key| (String::from(*key), "")),
                );
            }
        }
        lines.resize_with(rows - 2 - strip - panel, || (String::new(), ""));
        if panel > 0 {
            latest_lines(browser, panel, columns, palette, &mut lines);
            lines.resize_with(rows - 2 - strip, || (String::new(), ""));
        }
        if strip > 0 {
            lines.push(activity_line(browser, columns, palette));
        }
        if browser.diagnostic.is_empty() {
            lines.push((hint(browser, columns), palette.quiet));
        } else {
            lines.push((safe_text(&browser.diagnostic), palette.error));
        }
        let footer = footer(browser, columns);
        if matches!(browser.mode, Mode::Search | Mode::Edit) {
            input_cursor = Some((footer.width() + 1).min(columns));
            lines.push((footer, palette.editor));
        } else {
            lines.push((footer, ""));
        }
    }
    let mut frame = String::new();
    for index in 0..rows {
        frame.push_str(&format!("\x1b[{};1H\x1b[2K", index + 1));
        if let Some((text, style)) = lines.get(index) {
            let mut line = clipped(text, columns);
            if !style.is_empty() && !line.is_empty() {
                if *style == palette.selection {
                    line.push_str(&" ".repeat(columns.saturating_sub(line.width())));
                }
                frame.push_str(style);
                frame.push_str(&line);
                frame.push_str("\x1b[0m");
            } else {
                frame.push_str(&line);
            }
        }
    }
    if let Some(column) = input_cursor {
        frame.push_str(&format!("\x1b[{rows};{column}H\x1b[?25h"));
    } else {
        frame.push_str("\x1b[?25l");
    }
    let mut output = io::stdout().lock();
    output.write_all(frame.as_bytes())?;
    output.flush()
}
