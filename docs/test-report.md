# FolderWalk test report

## Live name search and recent files

The installed `fwalk` release passed all 25 Linux pseudo-terminal cases.
The implementer added three cases to the 22 existing cases and adjusted
the screen reader in the tests to distinguish the listing from the panel.
The new cases check searching folded folders, choosing between duplicate
names, editing a Unicode query during external changes, and ranking files
by modification time at startup and after a save or removal.
The ranking check uses filesystem timestamps and includes overlapping
watch roots, tied timestamps, a directory, and a symlink. [Terminal tests](../tests/terminal_test.py)

The implementer also launched `fwalk` by name at 80×24, resized to 40×10,
and checked focus, a long Unicode query and terminal restoration. The
Vim runtime example shows seven name matches and five recent files.
[Terminal screenshot](demo-images/name-search.png)

Rust formatting, all-target compilation, Clippy, the minimum-version
build on Rust 1.94.0, the release build, strict rustdoc, the Cargo layout
check, and offline lockfile resolution passed. Cargo built the unit test
target and ran zero Rust unit tests. An installation into an empty Cargo
install directory produced one executable named `fwalk`.
These checks were run locally on x86_64 Linux. [CI commands](../.github/workflows/check.yml)

## Live Max view

The independent QA worker ran the integrated debug and optimized Linux
binaries in real pseudo-terminals. Both passed all 22 cases with zero
skips. Seventeen existing cases cover navigation, the depth boundary,
folds, inaccessible folders, paste, resize, scroll and cleanup. Five
additional cases cover external file operations, switching location,
external changes during path editing, root recovery, and a burst that
exceeds the 64-path activity history. The QA worker wrote no production
code and wrote these tests after the integrated build and final requirements.
[Test cases](../tests/terminal_test.py)

| External operation while the release binary was idle | Time until the PTY test observed the notice |
|---|---:|
| Create a file | 51 ms |
| Save a file without changing its size | 50 ms |
| Create a second file | 50 ms |
| Atomically replace the first file | 100 ms |
| Remove the file from the watched directory | 101 ms |

These five observations come from one local filesystem run with roughly
50 ms sampling. The separate demo captured an edit in docs while the
browser was in src. Enter revealed the saved file and the minus key
restored the preceding location. All nine demo assets were verified,
including GIF decoding and the order of asciicast output records. Native
asciinema player execution was unavailable. [Recording and reproduction](demo.md)

The lead also launched the installed command by name in the FolderWalk
checkout. Its first complete Max view had 810 rows. The browser stayed in
`src/` while Codex edited this tracked report. Without keyboard input it
showed `Live   ~ Edited  docs/test-report.md`. Enter on that activity
revealed the selected report in `docs/`, the minus key returned to `src/`,
and quitting restored the original terminal attributes.

The editor test exposed a badge repaint on a file that changed while a
path was being typed. The fix freezes tree badges together with the
listing during editing. The same test then passed while the live activity
line continued reporting the save. The release suite also verified
permission recovery, an empty initial activity history, a 70-file burst,
stable focus after a sorted insertion and the absence of activity from a
symlink target or a file below the monitored depth.

The lead verified formatting, all-target Cargo check, Clippy with warnings
denied, locked release build, strict rustdoc, workspace layout and offline
lockfile resolution. The minimum-version checker passed at Rust 1.94.0,
edition 2024. Cargo test compiled the test target and ran zero Rust unit
tests. Runtime coverage comes from the PTY suite. The change enabled the
filesystem feature on the existing Rustix dependency. The workspace still
has one binary, with no new crate or dependency version. Hosted CI, network
mount timing and student usability were not measured in this run.

Three Codex workers covered filesystem research and monitoring, tree
interaction and demo capture, and independent QA. The lead integrated the
browser and kept the documentation. The room check before delegation
reported 31.9 of 35.2 GB memory available, 0.7 of 6 GB swap used, 0%
memory pressure, load 0.16 on 9 cores, 338.4 GB host disk space free and
one open session. Its verdict was “room for one more session: yes” with
capacity for three 8 GB jobs. [szFlow plan](../features/live-tree/step-1-plan.yaml)

## Three-level preview

The independent QA agent ran 17 Linux pseudo-terminal cases against the
optimized Rust 1.94.0 binary. All passed, including the permission-denied
case. Seven cases exercise the preview. They check the exact depth and row
order, independent branch collapse, retry after a rename, empty and unreadable
folders, a 40×10 terminal with color disabled, navigation through 32 children,
and expansion on the last visible row. The last case reproduced a hidden
first child on the preceding build and passed after the scroll adjustment.
[Executable cases](../tests/terminal_test.py)

Three Codex workers handled research and traversal, independent testing,
and demo capture. The lead integrated the UI and documentation. Test
preparation overlapped the build. The szFlow plan validator checked the
recorded plan at closeout. Before delegation, the room check reported
29.6 of 35.2 GB memory available, 0.7 of 6 GB swap used, 0% memory pressure,
load 0.16 on 9 cores, 338.4 GB free on the host drive and one open session.
Its verdict was “room for one more session: yes” with capacity for three
8 GB jobs. [Work plan](../features/three-level-preview/step-1-plan.yaml)

The lead ran formatting, the workspace all-target check, Clippy with
warnings denied, the minimum-version all-target build and the optimized
build. All passed. The layout checker found one binary and zero missing
source files. The tracked lockfile resolved offline. The binary rustdoc
checker and strict Cargo documentation build passed. Cargo test ran zero
Rust unit tests. The behavioral results above come from running the binary.
The change added no dependency, target, or crate boundary.

The demo worker captured two real terminal sessions. The recorded folder at
level three opened to reveal a file beyond the preview. GIF decoding and
asciicast record validation passed. Native asciinema playback and usability
with students were not measured. [Recordings](demo.md)

## Initial release

The original independent suite passed all 10 cases in the FolderWalk worktree and
passed the same 10 cases against a binary built from a fresh source copy.
Both runs reported zero failures. The suite exercised the Linux binary
built with Rust 1.94.0.

## Method

The test writer, wuse02, used the finalized requirements and documented
terminal interface. That session wrote no application code. The lead
integrated [the suite](../tests/terminal_test.py) byte for byte and
reran it in both locations. The terminal implementation also had an
earlier smoke check written by its implementer.

The test suite creates a controlled directory fixture and launches the
binary in a 220-column, 24-row Linux pseudo-terminal. It applies cursor
and erase instructions to compare the visible cells. Absolute paths are
compared with Bash physical cd and pwd, with CDPATH unset. Entry membership
is compared with Python filesystem enumeration after stripping display
markers and normalizing ordering.

Run from the project root:

```sh
cargo build --locked
python3 tests/terminal_test.py target/debug/fwalk
```

The lead used separate writable fixture directories under project scratch
because the shared temporary directory was read-only in this session.
Both test commands used the same suite and the only change between runs
was the binary and fixture location. Each execution prints its exact paths,
expected and actual results, and counts.

## Independent cases

Paths below are shown relative to the generated fixture root for readability.
The checks compared absolute paths.

| Input | Expected | Observed |
|---|---|---|
| Start in the fixture's start directory | Full path and matching filesystem names | Bash path matched, with .hidden, a child, file.txt, link, other, and český prostor visible. Directory and link markers matched. |
| Select a child with the keyboard, press Right, then Left | Open the child and return to its physical parent | start/a child opened and Left returned to start. |
| Submit a child, an absolute path to český prostor, then ../a child | Relative, absolute, spaced and Unicode paths match Bash | All three resolved to the expected physical paths. |
| Open directory symlink, then press Backspace | Show link target and its physical parent | physical parent/target opened and Backspace reached physical parent. |
| Submit a childX, then remove X and resubmit | Bash rejects the missing path and retains pwd. FolderWalk retains the view and recovers. | Bash returned 1 with the same pwd. FolderWalk showed “No such file or directory”, preserved the listing and text, and opened a child after correction. |
| Select a row, edit Unicode text, press Backspace and Escape | Path, selected row, and viewport stay steady | All corresponding visible rows remained identical throughout the edit and cancel. |
| Split opening and closing paste markers and a UTF-8 character across writes, then press Enter | The paste stays pending and the separate Enter navigates | No early navigation. Enter opened český prostor. |
| Paste a name containing newline and Escape, then correct it | No early navigation, cancellation, or quit. A diagnostic is visible. | The editor showed the single-path diagnostic, retained the view, and accepted a child on correction. |
| Quit with q inside a launching Bash process | Exit 0, restore attributes, preserve shell cwd | Exit was 0, original PTY attributes matched, and Bash reported start before and after. |
| Press Ctrl+C while editing | Exit 0 and restore terminal attributes | Exit was 0 and the original PTY attributes matched. |

Two early test-author trials caught harness assumptions about terminal
width and trimmed trailing spaces. The author corrected the harness and
reran the same cases. The final tests passed without a production code
change during the independent test phase.

## Other checks

The lead exercised the README build and run commands from a fresh source
copy. The run showed the current path and returned the terminal to its
original settings on q. All-target build, formatting, Clippy with warnings
denied, and Cargo test passed there too. Cargo test compiled the Rust
test target and ran zero Rust test cases. The Python suite supplies the
behavioral cases.

See the [design review](design-review.md) for the independent design
finding and [standards results](standards-results.md) for each standards
command and the limitations of native loading and hosted CI.
