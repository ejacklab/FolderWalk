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

Opening a directory resolves it physically and reads its immediate entries
before switching the displayed path. The UI can show this first listing
while the background worker builds the deeper tree. Empty input, a broken
link, a regular file, or a failed directory read leaves the previous view
available. The program keeps its own current directory. Bash physical cd
and pwd provide the comparison for navigation tests.

## Inline folder preview

A top-level row has depth zero. The worker builds a complete snapshot in
preorder, sorts siblings by native filename and includes dotfiles. It emits
directories at depth three without reading them. Symlinks appear as leaves.
The scanner calls the monitor before enumerating each directory so the
monitor can register for changes. A vanished entry is skipped. An unreadable
child is labelled and its siblings remain available.

Max projects all branches from the snapshot. Compact projects the top-level
entries. Space changes the selected folder's fold preference. Descendants
are visible only while their ancestors are open. Preferences use native
relative paths and survive refreshes while the directories still exist.
The display can change folds without reading the disk.

The depth convention follows eza's bounded tree example. Up and Down move
through visible rows, and expansion keeps focus on the folder. These
behaviors also appear in the W3C tree interaction pattern. FolderWalk keeps
the Right and Left keys for its existing directory navigation.
[eza tree example](https://eza.rocks/#features) ·
[W3C tree keyboard interaction](https://www.w3.org/WAI/ARIA/apg/patterns/treeview/#keyboardinteraction)

## Folder travel

Tab and Shift+Tab jump over descendants to the next or previous top-level
directory, with wraparound. Left opens the physical parent and selects the
departed folder. The minus key switches between the two most recent
locations. Each saved location carries the directory, selected relative
path, offset within the viewport, view mode and per-folder fold choices.
When a refreshed snapshot arrives, the browser locates the selected path.
If it vanished or became hidden, the browser selects the nearest visible
ancestor, then falls back to the previous row index within the new list.

## Live file changes

The launch directory remains the workspace for the session. A single worker
also monitors the displayed directory when it differs, applying the same
depth limit independently to each. The worker registers inotify watches on
directories before reading their children. It batches notifications and
compares the settled snapshots. A write notification can surface an edit
even when the file has the same size and timestamps. File stamps include
device, inode, mode, size, and nanosecond modification and change times.
Directory stamps omit size and times so a change to a child appears under
the child's path.

The worker checks for notifications at roughly 80 ms intervals, reconciles
every five seconds in Live mode and retries in about half a second when a
watch or root is unavailable. These are scheduling defaults. Actual delay
also includes the directory scans and terminal redraw. The worker keeps
the last successful snapshot when a root becomes unreadable. Subtrees
with unknown contents do not generate a series of false deletions.
Live preview traversal and metadata checks run on the worker thread.
User-directed navigation reads the immediate destination listing on the input thread.

The monitor hands the terminal the latest snapshot and coalesces activity
by absolute path across both roots. The activity line, inline marks and
recent list distinguish added, edited and removed paths. The session
retains up to 64 paths. Each root's first successful scan is its baseline.
A temporary file created and removed within a scan interval can be absent
from both snapshots. Rename operations can appear as a removal and an
addition. The monitor does not read file contents or identify the writer.
Linux inotify observes individual directories and can miss some remote
writes, so periodic reconciliation compares filesystem metadata too.
[Linux inotify manual](https://man7.org/linux/man-pages/man7/inotify.7.html)

## Editor and display algorithm

The terminal module provides run(directory: Directory) -> io::Result<()>.
The browser stores selection, scroll offset, path input, saved focus and recent activity.

In browse mode, Up and Down select a visible entry, Enter or Right opens it, Left
or Backspace goes to the parent, g opens the path editor, and q quits.
Ctrl+C quits in either mode. A bracketed paste in browse mode opens the
editor with that text. In edit mode text appends, Backspace removes the last
Unicode scalar, Enter submits, and Escape cancels. Paste adds text as one
action. Paste with line breaks or other control characters reports a
single-path error. Failed submissions keep the buffer visible and editable. Successful
ones return to browsing. A direct paste from browse mode starts a fresh
path buffer.

Show the absolute path first, then the tree and its live activity line.
A footer row shows hints or diagnostics. The last row shows controls or the path
input, prefixed with “Path: ”. Prefix selection
with a greater-than sign and space. Suffix directories with a slash and
symlinks with an at sign. Wrap the path across columns and clip entry lines
to fit. When the terminal cannot fit the header and a usable viewport,
show a resize message. Keep the selection visible when browsing. Editing
keeps the path, selected entry, and scroll position at a fixed terminal
size. Defer snapshot and inline badge updates in the editor while the activity
line continues to update. Redraw on input, resize or a monitor update,
overwrite lines in place, and flush each frame together. Escape filesystem control characters before rendering.
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
