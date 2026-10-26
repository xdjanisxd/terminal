# Linux final release sign-off: 0.2.2

Audit date: 2026-10-26. Proposed source is unpublished and untagged. No commit,
merge, push, or publication was performed during this sign-off.

## RC integration and source comparison

- Current branch: `release/linux-final-signoff`. It was already checked out at
  task start and points to the same commit as `master` and `origin/master`.
- Integrated master: `0625a56a332db04b276c909180e5f4d80a5e6273`, merge PR #171.
  Its second parent is accepted RC commit
  `aa0fcfc2e06b10eb91ed41b7f89798025ae3189b`; the RC branch is therefore
  integrated, not merely pushed.
- The committed tree at `HEAD` matches the accepted RC source:
  `git diff aa0fcfc..HEAD` is empty. The final proposal consists of the
  uncommitted changes listed below.
- The only executable behavior difference from RC is the version string:
  `--version` and `-V` report `terminal 0.2.2` instead of
  `terminal 0.2.2-rc.1`. No terminal, PTY, renderer, platform, or workspace
  runtime logic changed. No dependency versions changed.
- The 0.2.2-rc.1 release notes remain at
  [releases/0.2.2-rc.1.md](releases/0.2.2-rc.1.md). Stable notes are at
  [releases/0.2.2.md](releases/0.2.2.md).

Final proposed files:

- `Cargo.toml`, `Cargo.lock`: workspace package versions only.
- `scripts/test_package_release.py`: stable CLI version expectation.
- `docs/RELEASE.md`: stable version and sign-off links.
- `docs/releases/0.2.2.md`: stable Linux release notes.
- `docs/LINUX_RELEASE_SIGNOFF.md`: this evidence and future command record.

## CPU performance gate

The established seven-run gate and checked-in baseline were used without
threshold changes. Host: Debian GNU/Linux 13, Linux 6.12.113,
Intel Core i5-1250P, 16 CPUs in affinity, Rust/Cargo 1.99.0.

First command, `python3 scripts/benchmark_linux.py --runs 7 --check`, exited 1.
Only `resize_one_column_unicode_pair` crossed its gate: median 638,263 ns/op
against a 620,991 ns/op limit. Its CV was 20.7%. Recorded load averages moved
from 1.40/1.16/1.66 to 2.06/1.31/1.71 (1/5/15 minute windows).

Second command, `python3 scripts/benchmark_linux.py --runs 7 --check
--output target/perf/linux-performance-check-final.json`, exited 0 with
`no gated regressions`. The same workload median was 454,395 ns/op, p95
824,999.9 ns/op, and CV 26.2%; the baseline was 477,685 ns/op, 717,382 ns/op,
and 16.9%. Its first sample was 824,999.9 ns/op; the remaining six ranged from
432,326.7 to 463,409.5 ns/op. Load averages were 1.59/3.18/2.86 before and
1.58/3.13/2.85 after.

The second run passes the script's median thresholds, but the two runs
disagree and the second run has a large first-sample outlier and higher CV than
baseline. The CPU result remains **inconclusive**. The user explicitly waived
this gate for 0.2.2; the waiver is not a benchmark pass. No threshold or
performance change was made.

Second-run gated workload measurements (ns/op; CV is sample variability):

| Workload | Baseline median | Final median | Final p95 | Final CV |
| --- | ---: | ---: | ---: | ---: |
| `parse_plain_state` | 8,309.1 | 8,151.1 | 8,614.9 | 2.5% |
| `parse_escape_state` | 4,113.6 | 4,018.0 | 4,086.8 | 1.8% |
| `resize_one_column_unicode_pair` | 477,685.0 | 454,395.2 | 824,999.9 | 26.2% |
| `resize_columns_pair_history_2000` | 1,353,520.9 | 1,128,245.1 | 1,211,862.3 | 3.1% |
| `resize_columns_pair_history_10000` | 9,413,611.9 | 8,298,545.2 | 8,418,549.8 | 5.8% |
| `visible_projection_80x30` | 21,881.5 | 22,325.5 | 30,951.9 | 18.6% |
| `output_echo_plus_projection` | 20,719.3 | 18,626.1 | 20,446.8 | 3.8% |
| `three_pane_layout_plus_projection` | 25,109.3 | 23,326.4 | 25,266.4 | 3.0% |

## Artifact and reproducibility

- Target: `x86_64-unknown-linux-gnu`.
- Archive: `dist/terminal-linux-x86_64.tar.gz` (5,530,626 bytes).
- SHA-256: `6ca4797a8b817ed9e149fa06d6ebd60b056f2a15efc8408f65913d499aeb3aea`.
- Sidecar: `dist/terminal-linux-x86_64.tar.gz.sha256`; `sha256sum -c` passed.
- Two package runs produced identical archive bytes and hash.
- A fresh-target release build produced the same executable hash as the primary
  build: `0e69774c814f11807c8134c5b2bfef82856ad4221d5d859090cd519e6541a42d`.
  This verifies repeatability on this host and toolchain, not across CI hosts.
- The archive smoke check validated all 15 entries, exact packaged bytes,
  executable mode `0755`, data modes `0644`, and `--help`, `--version`, and
  `-V`. It includes the desktop entry and eight Linux hicolor icons.
- `ldd` resolved all direct ELF dependencies. The archive remains dynamically
  linked; the previously measured GLIBC 2.39 ABI floor still applies.

## Installation lifecycle

All lifecycle checks used temporary HOME and XDG paths. No developer
installation or user configuration was touched.

- Clean 0.2.2 install passed; installed CLI reported `terminal 0.2.2`.
- Upgrade passed from a release binary built from tag `v0.2.1`, then from the
  accepted 0.2.2-rc.1 binary, to the final archive executable.
- Config, Saved Workspaces, and an unrelated desktop file retained their exact
  contents through both upgrades and uninstall.
- Uninstall removed installer-owned files and preserved user data.
- `desktop-file-validate` passed on the packaged desktop entry. Package and
  installer tests also validated desktop metadata and icon paths.
- The native release CWD test passed for direct and installed launches.

## Automated validation

- `cargo fmt --all -- --check`: passed.
- `cargo check --workspace --all-targets --locked`: passed; seven crates.
- `cargo test --workspace --all-targets --locked`: 898 passed, 31 ignored.
- `cargo test --workspace --doc --locked`: 4 passed.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
- Locked Linux x86_64 release build: passed. `--version` and `-V` both printed
  `terminal 0.2.2`.
- `python3 scripts/test_install_linux.py`: 10 passed.
- `python3 scripts/test_package_release.py`: passed.
- `python3 scripts/generate_branding.py --check`: all 10 platform icon assets
  passed using pinned Pillow 12.1.0 in a temporary environment.
- Focused regressions: reset recovery 7 passed; PTY worker 13 passed; app shell
  exit 13 passed.
- Release native Linux shell lifecycle passed all 12 Bash/Zsh scenarios,
  including process exit, child reaping, and workspace preservation.
- Release native Linux working-directory inheritance test passed.
- Seven release renderer GPU tests passed with the repository JetBrains Mono
  fixture. This sandbox selected Vulkan llvmpipe (CPU), not Intel Iris Xe.
  Previously completed RC hardware GPU validation used Intel Iris Xe Vulkan.
- Parser/state fuzzing and bounded resource-stability stress were completed for
  the accepted RC and were not repeated for this version-only change. The
  earlier stress evidence is bounded, not a multi-hour soak.

## Manual KDE acceptance

Previously completed on the accepted RC, as supplied for this sign-off:

- Dolphin **Open Terminal Here**, including working-directory inheritance.
- Application menu launch and task-manager icon/grouping.
- Normal interactive terminal usage.

No new manual KDE acceptance was performed for the stable build. The only
runtime difference is the version string; the RC acceptance evidence remains
the relevant UI evidence.

## Blockers, limitations, and decision

**Confirmed product blockers:** none found by completed automated checks.

**CPU gate:** Explicitly waived by the user for 0.2.2. Its result remains
inconclusive, not passed. The recorded failed and successful runs remain above.

**Non-blocking limits:** Linux runtime evidence is Debian 13 x86_64 only; the
binary is dynamically linked and needs GLIBC 2.39 or newer, graphics/window
system support, and suitable fonts. This archive is unsigned. Current in-sandbox
GPU tests used llvmpipe; the accepted RC has prior Intel Iris Xe Vulkan
validation. The interactive KDE acceptance above is prior RC evidence.

**Deferred:** Windows and macOS runtime, packaging, install, and manual
acceptance remain deferred. No Windows or macOS runtime implementation changed.

**Recommendation:** Final release preparation is complete under the explicit
CPU-gate waiver. This report does not authorize or perform a tag or publication.

## Final manual acceptance commands

Run only in a disposable KDE user session with the final archive and checksum
sidecar. Do not install over a developer account.

```sh
sha256sum -c terminal-linux-x86_64.tar.gz.sha256
mkdir terminal-linux-x86_64
tar -xzf terminal-linux-x86_64.tar.gz -C terminal-linux-x86_64
cd terminal-linux-x86_64
./terminal --version
./terminal -V
python3 install_linux.py install
desktop-file-validate "$HOME/.local/share/applications/io.github.xdjanisxd.terminal.desktop"
kbuildsycoca6 --noincremental
"$HOME/.local/bin/terminal" --version
mkdir -p "$HOME/.config/terminal"
cp config.example.toml "$HOME/.config/terminal/config.toml"
```

Use the application menu and task manager, then launch Dolphin in a directory
with spaces and Unicode and select **Open Terminal Here**. Run `pwd`; verify
the requested directory opens and configured project roots keep precedence.
Exercise shell input, resize/reflow, tabs, splits, colors, graphemes, and close
behavior. Then verify uninstall preserves data:

```sh
pwd
printf '\033[38;2;255;80;60mRGB foreground\033[48;2;10;30;50m RGB background\033[0m 👩‍💻 é 日本語\n'
seq 1 1000
```

Save one workspace through the application before closing it.

```sh
python3 install_linux.py uninstall
test -f "$HOME/.config/terminal/config.toml"
test -f "$HOME/.config/terminal/workspaces.toml"
```

## Future review, merge, tag, and publication commands

These commands are documented for later execution only. They were not run.
First review and commit the proposed branch changes:

```sh
git status --short --branch
git diff --check
git diff
git add Cargo.toml Cargo.lock scripts/test_package_release.py docs/RELEASE.md docs/releases/0.2.2.md docs/LINUX_RELEASE_SIGNOFF.md
git diff --cached --check
git diff --cached
git commit -m "release: prepare Linux 0.2.2"
```

Review the committed branch, merge it into the latest master, and create an
annotated tag after final release approval:

```sh
git log --oneline --decorate master..release/linux-final-signoff
git diff --check master...release/linux-final-signoff
git diff master...release/linux-final-signoff
git switch master
git pull --ff-only origin master
git merge --no-ff release/linux-final-signoff -m "release: merge Linux 0.2.2 sign-off"
git tag -a v0.2.2 -m "Terminal 0.2.2"
git show --no-patch --format=fuller v0.2.2
test "$(git rev-parse 'v0.2.2^{commit}')" = "$(git rev-parse master)"
```

Pushing the stable tag starts the existing GitHub release workflow. After its
two Linux archive jobs finish, download the published x86_64 archive, compute
its checksum, and upload that sidecar:

```sh
git push origin master
git push origin v0.2.2
gh run list --workflow release.yml --branch v0.2.2 --limit 1
gh run watch <run-id> --exit-status
mkdir -p /tmp/terminal-0.2.2-release-assets
gh release download v0.2.2 --pattern terminal-linux-x86_64.tar.gz --dir /tmp/terminal-0.2.2-release-assets
(cd /tmp/terminal-0.2.2-release-assets && sha256sum terminal-linux-x86_64.tar.gz > terminal-linux-x86_64.tar.gz.sha256)
(cd /tmp/terminal-0.2.2-release-assets && sha256sum -c terminal-linux-x86_64.tar.gz.sha256)
gh release upload v0.2.2 /tmp/terminal-0.2.2-release-assets/terminal-linux-x86_64.tar.gz.sha256
mkdir -p /tmp/terminal-0.2.2-release-verify
gh release download v0.2.2 --pattern 'terminal-linux-x86_64.tar.gz*' --dir /tmp/terminal-0.2.2-release-verify
(cd /tmp/terminal-0.2.2-release-verify && sha256sum -c terminal-linux-x86_64.tar.gz.sha256)
gh release view v0.2.2
```

The workflow publishes the x86_64 and aarch64 archives. It does not currently
publish checksum sidecars; the final commands add a checksum for the exact
x86_64 archive produced by that workflow.
