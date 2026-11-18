# FolderWalk problem definitions

## Resolve and list a directory

1. Problem. Change the visible directory only after resolving and reading the requested directory.
2. For. A student browsing or entering a path. See [requirements](requirements.md).
3. Data. No student usage measurements were supplied. Bash resolved /bin to /usr/bin and then reached /usr on a physical parent step.
4. Given. The current absolute path and a child, parent, or submitted path.
5. Wanted. A complete destination snapshot, or an error and the previous snapshot.
6. Rules. Resolve relative input against the current snapshot. Follow directory symlinks physically. Sort entry names. Commit only after reading the listing.
7. Examples. From /sample, a path named “a b” opens that child. The root is its own parent. A missing destination returns an error and retains /sample.
8. Open. Long entry names may exceed the display width while their native values remain usable.
9. Judged by. Compare the path and entry names with physical Bash and the filesystem. Check memory and time after correctness.

## Edit a path while keeping the directory visible

1. Problem. Accept one typed or pasted path while maintaining the browse viewport during edits.
2. For. A student who knows a destination path. See [requirements](requirements.md).
3. Data. No student usage measurements were supplied. The local pseudo-terminal trial observed raw input.
4. Given. A directory snapshot, selection, list scroll offset, and text or key input.
5. Wanted. An editable input line and stable listing, followed by submit, cancel, or correction after an error.
6. Rules. Keep the edit buffer separate. Treat framed paste as text. Navigate only on submit. Keep failed input available for correction.
7. Examples. Typing a name with spaces leaves the listing steady. Paste containing a newline reports a single-path error. Cancel retains the original selection.
8. Open. A resize can change the available viewport.
9. Judged by. Compare the visible path and list before and during edits. Check memory and time after correctness.

## Restore the terminal

1. Problem. Return a usable terminal to the shell on application exit.
2. For. A student returning to the shell. See [requirements](requirements.md).
3. Data. The Rustix pseudo-terminal probe confirmed raw input and restoration of the original attributes.
4. Given. The original terminal attributes and a terminal session that can end by quit or handled error.
5. Wanted. Restored terminal settings and the original parent-shell directory.
6. Rules. Save the original attributes. Pair mode changes. Keep navigation state inside the application.
7. Examples. Pressing q exits browsing cleanly. A non-terminal launch prints a diagnostic without entering the interactive display.
8. Open. Forced external termination can prevent in-process cleanup.
9. Judged by. Compare pseudo-terminal attributes before and after quit and inspect shell pwd. Check memory and time after correctness.
