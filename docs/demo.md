# FolderWalk demo

## Search while typing

In this terminal, typing `/color` finds a folder and six files in the Vim
runtime. The two `dircolors.vim` results show their parent folders. The
panel below lists the five most recently modified files in the monitored
folders, even when their names do not match the search.

![Terminal showing seven matches for color and five recently modified files](demo-images/name-search.png)

## Notice a save while browsing another folder

This recording opens a small course project with `docs/` and `src/` visible.
Tab jumps between the folders, and Enter opens `src/`. A second process then
writes to `docs/outline.md`. Watch for the change notice before the next keypress.
Press c for recent changes and Enter on the edited file to reveal it in `docs/`.
Press the minus key to return to your place in `src/`.

![FolderWalk reports an external save to docs while browsing src, then jumps to the edited file](demo-images/live-folder-travel.gif)

[Download the terminal recording](demo-images/live-folder-travel.cast)

To try it yourself, run the following commands from the FolderWalk checkout in
a terminal. Keep the example directory open in a second terminal.

```sh
cargo build --locked
fwalk="$PWD/target/debug/fwalk"
mkdir -p .scratch
project=$(mktemp -d "$PWD/.scratch/coursework.XXXXXX")
mkdir "$project/docs" "$project/src"
touch "$project/docs/outline.md" "$project/src/main.rs" "$project/README.md"
echo "$project"
(cd "$project" && FOLDERWALK_THEME=mixed "$fwalk")
```

Open `src/` in FolderWalk. In the second terminal, change to the printed
directory and append a line to the document.

```sh
printf '\nRevised outline\n' >> docs/outline.md
```

## Fold and reopen a branch

These examples browse the installed Vim packages on Linux. From the checkout
with the binary path set as above, run the following command.

```sh
(cd /usr/share/vim/vim91/pack && FOLDERWALK_THEME=mixed "$fwalk")
```

Max view initially shows `dist/`, its `opt/` directory, packages such as
`cfilter/`, and the `plugin/` directory inside that package. Press Space on
`dist/` to fold it. Press Space again to see its descendants in place.

![FolderWalk folds dist and reopens the three-level branch](demo-images/expand-and-collapse.gif)

[Download the terminal recording](demo-images/expand-and-collapse.cast)

## Open a folder at the limit

In Max view, press Down three times to select `cfilter/plugin/`. Press
Enter to open the directory and see `cfilter.vim`.

![FolderWalk opens a third-level directory and reveals its file](demo-images/open-at-limit.gif)

[Download the terminal recording](demo-images/open-at-limit.cast)

The GIF captions identify the input or external action for each frame. The
asciicast files contain the sessions’ timed terminal output. To replay one,
run `asciinema play live-folder-travel.cast` with the downloaded file.

## Browse and edit a path

The following examples use the Linux time-zone database.

```sh
(cd /usr/share/zoneinfo && "$fwalk")
```

Select `Africa/` and press Enter to open it.

![Startup at /usr/share/zoneinfo with Africa selected](demo-images/startup.png)

![Navigation into /usr/share/zoneinfo/Africa](demo-images/navigation.png)

Press g and type `../Europe`. The Africa listing stays visible while you edit.
Press Enter to open Europe, Escape to cancel the edit, or q to quit browsing.

![The Africa listing with ../Europe in the path editor](demo-images/editing.png)
