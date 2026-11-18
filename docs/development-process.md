---
format_version: 1
id: folderwalk.quality.development-process
kind: principle
category: testing-and-quality
severity: consider
checks: []
---

# How FolderWalk was developed

FolderWalk is a public Rust terminal directory browser for university
students. It was also the first workspace in this effort to exercise all five
local Rust standards plugins together. This record covers the work from
project setup through the local WSL2 release, including the checks and
limitations observed along the way. [Accepted problem statement](problem-statement.md)

## 1. Set up the project and agree on the goal

The lead initialized the project instructions and local Git repository.
The instructions linked the five plugin skills to their original sources,
and Claude Code used the same file through its one-line import.
The lead verified the plugin marketplace entries and read each plugin's
README and shared skill. Native registration hit a read-only directory
in Codex, so the shared skills were read directly. EJ accepted the
problem statement on 2026-11-18 before design and implementation began.
[Agent instructions](../AGENTS.md) · [Claude import](../CLAUDE.md) ·
[Local standards setup](local-standards.md)

## 2. Plan the work and form the team

The lead restarted at szFlow's planning step and split the work into
navigation, terminal, and integration, with one owner per source file.
Session discovery returned 50 visible threads at its query limit, 10 of
them idle. Five idle sessions were shortlisted after checking their thread
IDs, work, models, and last recorded access. The room check reported
capacity for three heavy jobs. The lead tracked five workstreams on
szflow-board, including a second brief for the replacement reviewer.
The board recorded receipts, blockers, and results.

Two sessions researched terminal tooling and filesystem oracles.
One of them also handled the independent design review after the first
reviewer stalled on an approval prompt. A separate session wrote the
terminal module. Another session wrote the independent acceptance suite
after the build. The lead owned navigation, integration, documentation,
and the final checks. The plan validator passed. The installed team
validator refused the board-backed record under its older schema, so
the board and team record carried actual ownership and pickup.
[Plan](../features/folderwalk/step-1-plan.yaml) ·
[Team and workflow exception](../features/folderwalk/step-2-team.yaml)

## 3. Research, design, and review

The terminal research considered Crossterm, but its crate was missing from
the local cache and its download host did not resolve. An offline Linux
pseudo-terminal trial built Rustix and Unicode Width and verified raw input,
window size, character width, and terminal restoration. The filesystem
research showed that Bash logical and physical navigation differ through
symlinks. The design chose physical paths and a snapshot that updates only
after the destination listing succeeds.

The lead wrote the core requirements, problem definitions, and design
algorithms in that order. The terminal design kept the current directory,
selection, and scroll position separate from the editable path. The
independent review found one blocking gap. A paste marker or a UTF-8
character can span reads. The lead added explicit framing and incremental
decoding to the design before implementation.
[Requirements](requirements.md) · [Problem definitions](problem-definitions.md) ·
[Design](design.md) · [Independent design review](design-review.md)

## 4. Build and integrate

The terminal implementer owned the terminal module. The lead wrote the
filesystem snapshot and navigation module and wired both through the
binary entry point. During integration, the terminal implementer corrected
direct paste to start a fresh buffer and reserved separate, fixed rows for
diagnostics and input. These changes made it possible to show an invalid
path and keep the editor visible without moving the listing.

The build compiled on Linux with Rust 1.94.0 and edition 2024. The lead
finalized the requirements after the complete build and before sending the
independent test brief.
[Terminal](../src/terminal.rs) · [Navigation](../src/navigation.rs) ·
[Entry point](../src/main.rs) · [Toolchain](../rust-toolchain.toml)

## 5. Run independent tests and exercise the five standards

The test writer worked from the finalized requirements and terminal
interface and wrote no production code. The lead integrated the test file
byte for byte. The suite compared physical paths with Bash, entry names
with filesystem enumeration, and visible screen rows and terminal state
through a Linux pseudo-terminal. It covered spaces, Unicode, absolute
and relative paths, invalid input and recovery, parent and symlink
navigation, split paste, stable editing, and exit. The writer corrected
two harness assumptions about terminal width and trailing spaces. The
same ten cases then passed in the working tree and a fresh source build,
with zero failures in either run.

The lead recorded marketplace discovery, native loading, and shared skill
execution separately. All five were discovered and exercised directly.
Formatting and Cargo checks, the minimum-version build, Cargo layout,
and strict rustdoc passed. The lockfile checker passed in an isolated,
byte-identical source snapshot with the lockfile staged. The main working
Git index was read-only during that check. Native Codex loading remained
blocked, and hosted CI execution was unverified in this run.
[Test report](test-report.md) · [Independent test source](../tests/terminal_test.py) ·
[Standards results](standards-results.md) · [CI workflow](../.github/workflows/check.yml)

## 6. Document and publish the initial build

The student README documented building, running, controls, tests, and
the three source files. The demo used the public Linux time-zone directory.
Its three images were rendered from the real application's pseudo-terminal
output for startup, directory navigation, and path editing.

Publication waited for EJ. The first push failed while the GitHub
destination was unavailable. After EJ supplied the repository, the lead
pushed the application to an import branch to preserve the existing main
history. EJ fetched the remote main commit when this session could not.
The lead then merged both histories and verified the push. EJ chose MIT
for the teaching project. The existing MIT license was preserved, and the
README and Cargo metadata declared it in commit `b0d1eda`.
[Student README](../README.md) · [Demo](demo.md) ·
[Initial merge](https://github.com/ejacklab/FolderWalk/commit/c3eb0e8a00a8d02b0537fc98b87686ec7e5c778e) ·
[MIT declaration](https://github.com/ejacklab/FolderWalk/commit/b0d1edafade06e1956dbcc413c3e5929b71564f6)

## 7. Refine the display and prepare WSL2 v1

EJ tried the app and asked for color and spacing. The lead added a
highlighted selection, adaptive spacing around the listing, and a cyan
default palette. Two more presets followed, green and dark blue. EJ then
asked for a single theme combining all three. The mixed theme uses a cyan
path, green directories, and a dark blue selection. The same ten terminal
cases passed in each of the four theme runs. A separate terminal probe
verified the mixed frame and the plain display option.

The lead prepared the local 1.0.0 release in commit `303e737`, built an
optimized executable, and ran all ten tests against that executable.
At EJ's request, the lead moved it from project scratch to
`bin/folderwalk`, verified its checksum, and launched it by name from a
different directory using the project bin directory on PATH. The sandbox
could not copy it into the user's existing PATH directory. The project
binary is ignored by Git. At closeout, GitHub main pointed to the MIT
declaration and the v1 commit was local.
[Run and install instructions](../README.md) · [Renderer](../src/terminal.rs)

## Lessons from this run

- The terminal and filesystem could be researched in parallel, then built
  against one reviewed interface. The fixed path and viewport contract
  made visual changes testable without changing the navigation cases.
- Finalizing the interface after the build gave the independent test writer
  one stable source for expected behavior.
- Package discovery, native loading, shared skill execution, local tests,
  hosted CI, and publication each had separate evidence. A passing result
  at one stage left the other stages open until they were checked.

The existing project documents hold the requirements, design decisions, test
results, and release commands. The board receipts and daylog give the local
handoff and decision history. The front matter follows the
[EJJ MD v1 specification](https://github.com/661818yijack/ejj-md-file-format/blob/82956941f70a2e99748f8c3ea0a0fd29df3b3f9b/SPEC.md).
