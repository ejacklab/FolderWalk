# Rust standards results

FolderWalk's first integrated run used the Rust 1.94.0 toolchain,
Rust 2024, and the x86_64 Linux host. The package has one binary target
and no project feature switches. The Cargo commands used the project’s
configuration and lockfile contents for this run.
See the [local setup](local-standards.md) for locating the skills.

## Package discovery and native loading

The local Rust standards manifest listed all five packages. Each README
and shared skill was readable. Claude Code's local manifest validation
identified the skill provided by each of the five plugins and passed.
It also reported missing licenses for directory submission and missing
icons. Those advisory diagnostics concern plugin publishing.

Codex registration exited 1 on both attempts with this error:

```text
Error: failed to create marketplace install directory /home/smoke01/.codex/.tmp/marketplaces: Read-only file system (os error 30)
```

The registration prerequisite blocked individual native installations.
The lead used the five shared skills directly.

## Results on FolderWalk

| Standard | Package discovered | Codex native load | Direct skill use and observed result | Blocked or inapplicable |
|---|---|---|---|---|
| Code Style | Yes | Blocked | Cargo formatting and workspace all-target Cargo check passed. No identifier casing warnings were emitted. The package, module and role-name review covered folderwalk, navigation, terminal, Directory, Browser and Decoder. | Macro, long-option and benchmark-output naming did not apply to this change. |
| Correctness & Safety | Yes | Blocked | The shared minimum-version checker ran a locked offline all-target build for folderwalk at the declared 1.94.0 minimum, edition 2024. It passed. Rustup used a project-local toolchain entry pointing at the verified 1.94.0 installation. The workflow builds the pinned toolchain on pushes and pull requests. | Hosted runs and required merge settings are unavailable before publication. |
| Architecture & Design | Yes | Blocked | The shared layout checker examined one workspace member and one binary target and found zero missing source paths. The design review confirmed that navigation can be read independently from terminal I/O. The internal module boundary serves the one current executable. | There is no library consumer or Cargo integration-test target. The external PTY suite runs the executable. |
| Testing & Quality | Yes | Blocked | Cargo test compiled the test target and executed zero Rust test cases. The shared strict rustdoc check and the CI documentation command both passed. The independent PTY execution appears in the [test report](test-report.md). | The public library documentation lint, shell completions, benchmarks and fuzz targets are outside this workspace's scope. Hosted CI execution is pending publication. |
| Security & Dependencies | Yes | Blocked | The shared lockfile checker passed offline in an isolated Git snapshot with this project's byte-identical Cargo.lock staged. Manifest and lockfile review found two direct crates, Rustix 1.1.6 and Unicode Width 0.2.2, and only crates.io registry sources. This is the initial dependency baseline. | The working Git index is read-only, so Cargo.lock could not be staged there. The tracked-lockfile prerequisite was met in the snapshot. No static-linking policy, target linker override, or RUSTFLAGS override applies. |

The lead, who implemented navigation and integration, ran the plugin
commands, inspected the build configuration and named source items,
and recorded the results. The automated checks establish the observed
build, source layout, rustdoc, and lockfile behavior on this Linux
configuration. The [independent design review](design-review.md) and
[test report](test-report.md) provide separate behavioral evidence.
