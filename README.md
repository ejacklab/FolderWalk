# FolderWalk

FolderWalk is a small Rust terminal application for exploring directories.
It opens a three-level tree and shows live file changes while you browse.
Start it in your project folder to follow saves from your editor, Codex, or
another terminal. It is written for university students who want a working
program to read and study.

## Build and run

Use a Linux terminal with UTF-8 and bracketed paste, and install Rust with
[rustup](https://rustup.rs/). The included toolchain file selects Rust 1.94.0.
Cargo may need network access on the first build to fetch dependencies.
From the project checkout, run:

```sh
cargo build --locked
cargo run --locked
```

FolderWalk opens in the directory where it starts. To open another folder,
build it first, save the binary's path, and launch it from that folder:

```sh
folderwalk="$PWD/target/debug/folderwalk"
(cd /usr/share/zoneinfo && "$folderwalk")
```

## Install the command on WSL2

From the checkout, install FolderWalk in Cargo's bin directory:

```sh
cargo install --path . --locked
```

With Cargo's bin directory on your PATH, run FolderWalk from any directory:

```sh
folderwalk
FOLDERWALK_THEME=mixed folderwalk
```

## Navigate

Max view opens every branch through three levels below each top-level
folder. For example, `course/week-1/lab/solutions/` shows `solutions/` at
the boundary. Press Enter there to browse deeper. Press Tab to skip the
rest of a branch and land on the next top-level folder.

| Input | Action |
|---|---|
| Up or Down | Move the selection |
| Tab or Shift+Tab | Jump to the next or previous top-level folder, wrapping at the ends |
| Page Up, Page Down, Home, End | Move a page, or jump to the first or last row |
| Space | Expand or collapse the selected folder in place |
| m | Switch between the full Max tree and a folded Compact tree |
| Enter or Right | Open the selected directory |
| Left or Backspace | Go up and select the folder you just left |
| - | Switch back to the previous directory and selection |
| c | Show recent changes. Enter reveals a path and Escape returns to the tree |
| ? | Show the keyboard guide |
| g | Open the path editor |
| Paste | Add a path to the editor, opening it if needed |
| Enter while editing | Submit the path |
| Escape while editing | Cancel the edit |
| q while browsing, or Ctrl+C | Quit |

Each indent is one level.
The triangle points right for a closed folder and down for an expanded one.
A dot marks a folder at level three. Select it and press Enter to keep
exploring from there. Space on the original folder folds the preview away.

Directories end in a slash. Symbolic links end in an at sign. The tree
shows links as individual entries. Type or
paste paths literally, including spaces. Relative paths start at the
displayed directory. FolderWalk follows directory symlinks and shows
the resolved path. A failed path leaves the current directory and listing
visible, with an error above the input. Quitting returns to the launching
shell at its original directory.

## Follow another process's edits

Leave FolderWalk open in a project and change a file from another terminal.
The activity line shows the operation and path, such as `~ Edited  src/main.rs`.
You can stay in `docs/` and still see a save under `src/`. Press `c`, then
Enter, to reveal the changed file in its directory. Press `-` to return.

FolderWalk monitors the launch directory and the directory on screen, both
through the same preview depth. Folding a branch hides its rows while its
changes still arrive. A new file appears as `+ Added`, an edit or replacement
as `~ Edited`, and a deletion as `− Removed` in the activity list. A rename
can produce one removal and one addition. Recent changes keeps the latest
observation for up to 64 paths during the session.

The first scan establishes the starting state. On Linux the activity line
then shows `Live` for directory notifications or `Polling` when a directory
watch is unavailable. The tree holds still while you type a path. Activity
continues to arrive, and the tree catches up when you leave the editor.

FolderWalk uses cyan by default. The mixed theme uses a cyan path, green
directories, and a dark blue selection. Try another theme with:

```sh
FOLDERWALK_THEME=green cargo run --locked
FOLDERWALK_THEME=blue cargo run --locked
FOLDERWALK_THEME=mixed cargo run --locked
```

For a plain display, run `NO_COLOR=1 cargo run --locked`.

Watch the [recorded terminal demos](docs/demo.md).

## Read the code

- [main.rs](src/main.rs) opens the startup directory, runs the terminal,
  and prints any final error.
- [navigation.rs](src/navigation.rs) builds a snapshot of the directory
  and swaps it in after successful navigation. It uses the Rust standard
  library and can be read without knowing terminal escape sequences.
- [live.rs](src/live.rs) watches directories and compares filesystem snapshots
  on a background thread.
- [terminal.rs](src/terminal.rs) reads keys and paste events, accepts monitor
  updates, and restores the terminal before exiting.
- [browser.rs](src/terminal/browser.rs) holds the selection, previous location,
  path editor and recent activity. [view.rs](src/terminal/view.rs) draws them.

Start with the Enter key in browse mode. Follow the terminal
event into the navigation module, then watch how the next frame uses the
new snapshot.

## Run the checks

From the project root, run the commands below. The terminal test suite uses
Python 3 and Bash on Linux.

```sh
cargo fmt --all --check
cargo build --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
python3 tests/terminal_test.py target/debug/folderwalk
```

See the [requirements](docs/requirements.md), [design](docs/design.md), and
[test report](docs/test-report.md). FolderWalk also exercises five Rust
standards plugins. The [local setup](docs/local-standards.md) and
[standards results](docs/standards-results.md) document that work.

## License

FolderWalk is available under the [MIT License](LICENSE).
