# FolderWalk

FolderWalk is a small Rust terminal application for exploring directories.
It shows the full current path and the directory's contents. It is
written for university students who want a working program to read and study.

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

## Navigate

| Input | Action |
|---|---|
| Up or Down | Move the selection |
| Enter or Right | Open the selected directory |
| Left or Backspace | Go to the parent directory |
| g | Open the path editor |
| Paste | Add a path to the editor, opening it if needed |
| Enter while editing | Submit the path |
| Escape while editing | Cancel the edit |
| q while browsing, or Ctrl+C | Quit |

Directories end in a slash. Symbolic links end in an at sign. Type or
paste paths literally, including spaces. Relative paths start at the
displayed directory. FolderWalk follows directory symlinks and shows
the resolved path. A failed path leaves the current directory and listing
visible, with an error above the input. Quitting returns to the launching
shell at its original directory.

See the [three-screen demo](docs/demo.md) for the display in use.

## Read the code

- [main.rs](src/main.rs) opens the startup directory, runs the terminal,
  and prints any final error.
- [navigation.rs](src/navigation.rs) builds a snapshot of the directory
  and swaps it in after successful navigation. It uses the Rust standard
  library and can be read without knowing terminal escape sequences.
- [terminal.rs](src/terminal.rs) reads keys and paste events, keeps track
  of selection and the edit buffer, draws the screen, and restores the
  terminal before exiting.

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
