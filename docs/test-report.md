# FolderWalk test report

The independent suite passed all 10 cases in the FolderWalk worktree and
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
python3 tests/terminal_test.py target/debug/folderwalk
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
