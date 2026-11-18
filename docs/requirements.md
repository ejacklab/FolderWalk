# FolderWalk core requirements

The original navigation requirements come from the [accepted problem
statement](problem-statement.md). The live browsing requirements below
were finalized against the integrated Linux build on 2026-11-18.

## Show the current directory and let the student navigate it

| Kind | Requirement | Fails if | Minimum accepted | Source |
|---|---|---|---|---|
| Processing logic | The system shall display the full absolute current directory path. | The header shows a relative path or basename. | Startup shows the fixture's absolute path. | Problem statement |
| Data | The system shall list the current directory entries. | An existing entry is omitted from the navigable list. | Files, directories, hidden entries and symlinks are represented. | Problem statement and [design](design.md) |
| Processing logic | The system shall navigate to a directory through the keyboard. | A student cannot open a selected directory or return to its parent. | A fixture child and its parent are both reachable. | Problem statement |
| Processing logic | The system shall accept a typed or pasted directory path. | A path containing spaces cannot be submitted as one path. | A relative and an absolute path containing spaces reach the expected directory. | Problem statement and [design](design.md) |
| Processing logic | The system shall treat a framed paste as path text until explicit submission. | Chunk boundaries, newline or Escape in a paste trigger navigation, cancellation, or quit. | Split the framing and a UTF-8 character across writes. No navigation occurs before a separate Enter. | [Design review](design-review.md) |
| Processing logic | The system shall keep the path and listing visible and steady while the student edits a path. | Editing changes the directory, selection, or list scroll position. | Visible path and list rows stay identical during edits at a fixed terminal size. | Problem statement and [design](design.md) |
| Processing logic | The system shall follow physical symlink and parent traversal. | The parent of a followed link is taken relative to the link's containing directory. | A symlink opens its target and the target's physical parent is reachable. | [Design](design.md) |
| Error and filter | The system shall retain the previous directory and listing on navigation failure. | A failed submission changes the listing or prevents recovery. | A visible error and editable input precede successful navigation after correction. | [Design](design.md) |
| Integration | The system shall restore terminal settings on clean exit. | Terminal attributes differ after quit. | A pseudo-terminal has its original attributes after exit. | [Design](design.md) |
| Integration | The system shall leave the parent shell's working directory unchanged. | The parent shell reports a changed directory on exit. | A shell wrapper observes the same working directory before and after browsing. | [Design](design.md) |

Business rule, configuration, and specific kinds have none found. The
navigation behavior is defined by the rules above.

## Preview a folder three levels deep from the current directory

| Kind | Requirement | Fails if | Minimum accepted | Source |
|---|---|---|---|---|
| Processing logic | The system shall reveal up to three descendant levels when a student expands a listed folder. | A readable child is missing or a fourth level is exposed by a nested toggle. | A four-deep fixture reveals its first three levels and keeps the selected folder focused. | EJ's three-level preview request |
| Processing logic | The system shall collapse an expanded folder in place. | A sibling branch changes or the cursor moves away from the selected folder. | Expand two siblings, collapse either one, and retain the other's rows. | [Interaction design](design.md#inline-folder-preview) |
| Data | The system shall open a selected nested folder using its path from the displayed directory. | Opening the third-level folder reaches a different destination. | Enter reaches the selected folder and lists its direct children. | [Interaction design](design.md#inline-folder-preview) |
| Error and filter | The system shall display symlinks as leaves during expansion. | An ancestor link repeats a subtree. | A link to the starting folder appears once under its parent. | [Interaction design](design.md#inline-folder-preview) |

Business rule, configuration, integration and specific kinds have none
found for this interaction. It uses the existing terminal and filesystem.

## Browse the full preview and notice external changes while moving between folders

| Kind | Requirement | Fails if | Minimum accepted | Source |
|---|---|---|---|---|
| Processing logic | The system shall start in Max view with every readable branch expanded to the preview depth. | A student must select each folder to see its children. | Two sibling branches show their descendants on startup. Switching to Compact folds both. | EJ's request to try Max and the [interaction design](design.md#inline-folder-preview) |
| Processing logic | The system shall move directly between top-level folders. | Reaching the next folder requires walking its neighbour's descendants. | Tab and Shift+Tab move across two expanded siblings in either direction. | EJ's folder travel request |
| Processing logic | The system shall restore the previous location when the student returns to it with the previous-location key. | A surviving selected path changes on return. | Travel to a second directory and press the minus key to recover the original folder, focus and saved folds. | [Interaction design](design.md#folder-travel) |
| Integration | The system shall show settled external additions, edits and removals within the monitored depth of the launch and displayed directories, including folded branches. | A local edit appears only after a keypress, or navigation hides activity elsewhere in the launch tree. | From a second process create, write, atomically replace and remove files while the browser is idle in a sibling folder. Verify paths and operation labels. | EJ's live changes request |
| Data | The system shall keep a bounded list of recently changed paths for the session. | The starting inventory appears as changes or a later change to the same path makes another list row. | Open the list after two saves to one file and reveal that file's directory with Enter. | [Live browsing design](design.md#live-file-changes) |
| Processing logic | The system shall keep a surviving selected path focused across refreshes. | Inserting a sibling above it moves focus to another file. | Insert a sibling, then edit a path while another change arrives. The tree remains stable in the editor and catches up on exit. | EJ's folder travel request and the original path editor requirement |
| Error and filter | The system shall indicate when a monitored root cannot be read and retry it. | The view silently claims to be current or reports a whole missing root as individual deletions. | Move a root away, then restore it and observe recovery. | [Live browsing design](design.md#live-file-changes) |

Business rule, configuration and specific kinds have none found for this
interaction. It uses the current Linux terminal and filesystem.

## Let the student study and reproduce the project

| Kind | Requirement | Fails if | Minimum accepted | Source |
|---|---|---|---|---|
| Specific | The system shall provide a public student README and demo. | Build, run, controls, test, code tour, or demo instructions are missing. | The README commands run from a clean source copy and the demo links to real screenshots. | Problem statement and EJ's project brief |
| Integration | The system shall record use of all five Rust standards on the integrated workspace. | A standard has no result or discovery, loading, and use are conflated. | Five results list what ran, what was observed, and blocked or inapplicable checks. | EJ's project brief |

Business rule, processing logic, data, configuration, and error and filter
kinds have none found. These requirements cover learning material and check
records.


## Terminal interface for independent tests

Launch the built binary in a UTF-8 ANSI pseudo-terminal with working
directory set to a fixture. Set its width so the fixture's absolute path
and names fit. The top row is the full path for such a fixture. Entry rows
prefix the selection with a greater-than sign and a space and mark
directories with a trailing slash and symlinks with an at sign. A status
such as `~ Edited` may follow the filename. On a normal-size terminal the
activity row begins with Live or Polling once the first scan completes.
The last two rows belong to hints or diagnostics and controls or path input. The
path input starts with “Path: ”. These rows leave the entry viewport at
the same size through editing and error recovery.

Use [the design](design.md) for key bindings and physical path behavior.
For the system comparison, invoke Bash with CDPATH unset and use its
physical cd and pwd. Compare entry membership as names, ignoring selection,
type markers, and ordering. Include hidden entries. Observe screen rows
after applying terminal cursor and erase commands rather than matching
unpositioned output. Check original PTY attributes and the launching
shell's directory at exit.
