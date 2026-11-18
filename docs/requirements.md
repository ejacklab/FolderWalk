# FolderWalk core requirements

Finalized on 2026-11-18 after the complete offline build, before the
independent test brief. The source for the core requirements is the
[accepted problem statement](problem-statement.md).

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
directories with a trailing slash and symlinks with an at sign. The last
two rows always belong to diagnostics and controls or path input. The
path input starts with “Path: ”. These rows leave the entry viewport at
the same size through editing and error recovery.

Use [the design](design.md) for key bindings and physical path behavior.
For the system comparison, invoke Bash with CDPATH unset and use its
physical cd and pwd. Compare entry membership as names, ignoring selection,
type markers, and ordering. Include hidden entries. Observe screen rows
after applying terminal cursor and erase commands rather than matching
unpositioned output. Check original PTY attributes and the launching
shell's directory at exit.
