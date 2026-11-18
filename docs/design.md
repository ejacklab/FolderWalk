# FolderWalk design

## Scope

The first release supports Linux terminals with UTF-8, common ANSI cursor
controls, and bracketed paste. It uses Rust 2024 with Rust 1.94 as the minimum
verified toolchain. Rustix supplies terminal attributes, window size, read,
and poll. Unicode Width supplies terminal column widths. Both dependencies
built offline in the local probe, which observed raw input and restoration
through a pseudo-terminal. Crossterm provides higher-level events. Its crate
was missing from the local cache and the download host did not resolve.

## Directory algorithm and interface

The navigation module exposes this internal interface:

    Directory::open(path: &Path) -> io::Result<Directory>
    Directory::path(&self) -> &Path
    Directory::entries(&self) -> &[Entry]
    Directory::go(&mut self, path: &Path) -> io::Result<()>
    Directory::up(&mut self) -> io::Result<()>
    Entry::name(&self) -> &OsStr
    Entry::kind(&self) -> EntryKind
    EntryKind::{Directory, Symlink, Other}

Start at the process startup directory. Resolve absolute input directly and
relative input against the current snapshot. Reject an empty submission.
Pass nonempty input as a literal path, including spaces. Use physical
resolution through canonicalize. Read all entries and their entry types,
preserve native filenames, and sort by filename. Replace the snapshot only
after these steps succeed. Opening a directory symlink follows its target.
A regular file, broken symlink, or unreadable directory reports an error
and preserves the previous snapshot. The root is its own parent.

For differential tests, use Bash physical cd and pwd as the path oracle.
Compare entry names using filesystem enumeration, including hidden entries
and excluding dot and dot-dot. Compare names independent of display order.
The program keeps the current directory in its own state, leaving the
process and parent shell working directory alone.

## Editor and display algorithm

The terminal module provides run(directory: Directory) -> io::Result<()>.
It owns selection, scroll offset, the edit buffer, and the latest diagnostic.

In browse mode, Up and Down select an entry, Enter or Right opens it, Left
or Backspace goes to the parent, g opens the path editor, and q quits.
Ctrl+C quits in either mode. A bracketed paste in browse mode opens the
editor with that text. In edit mode text appends, Backspace removes the last
Unicode scalar, Enter submits, and Escape cancels. Paste adds text as one
action. Paste with line breaks or other control characters reports a
single-path error. Failed submissions keep the buffer visible and editable. Successful
ones return to browsing. A direct paste from browse mode starts a fresh
path buffer.

Show the absolute path first, then entries and two fixed footer rows.
One footer row shows diagnostics. The other shows controls or the path
input, prefixed with “Path: ”. Prefix selection
with a greater-than sign and space. Suffix directories with a slash and
symlinks with an at sign. Wrap the path across columns and clip entry lines
to fit. When the terminal cannot fit the header and a usable viewport,
show a resize message. Keep the selection visible when browsing. Editing
keeps the path, selected entry, and scroll position at a fixed terminal
size. Redraw on input or resize, overwrite lines in place, and flush each
frame together. Escape filesystem control characters before rendering.
Preserve the native name for keyboard navigation when a non-UTF-8 byte
appears as a replacement character.

## Terminal session algorithm

Check that standard input and output are terminals. Save terminal
attributes before enabling raw mode, the alternate screen, and bracketed
paste. Read and poll the input descriptor using Rustix. Decode UTF-8 and
CSI incrementally across reads. Match the opening and closing bracketed
paste markers across reads, buffer the body, and emit one paste event.
While a paste is open, Escape, Enter and Ctrl+C bytes belong to the body.
A short timeout distinguishes a lone Escape outside an in-progress sequence
or paste. Recheck size while waiting for input. Ignore unrecognized control
sequences.
Restore the terminal attributes, cursor, and screen on normal exit and
handled errors. Tie teardown to the scope that owns setup, including a failed
screen initialization. Report startup and terminal errors after cleanup.

## Integration

The main binary passes its startup directory to navigation, then passes the
snapshot to the terminal module. Navigation is independent of terminal I/O.
The independent tests launch the actual binary in a pseudo-terminal and check
visible rows, physical navigation, and restored terminal attributes.
