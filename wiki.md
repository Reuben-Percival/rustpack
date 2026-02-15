# rustpack Wiki

This wiki is the long-form reference for operators and contributors.

- For quick usage, see `README.md`.
- For implementation details and extension rules, use this document.

## 1. Project Scope

`rustpack` is an ALPM-first package manager frontend for Arch-style systems.

Core intent:

- Keep package operations inside `libalpm` flows.
- Preserve pacman-style ergonomics for common flags.
- Add clearer UX, diagnostics, and guardrails around risky operations.

Non-goals today:

- Full 1:1 pacman flag parity.
- Native AUR build/install pipeline (AUR is delegated to `paru`).

## 2. Current Feature Set

### Package operations

- Sync/install: `-S`, `-Sy`, `-Su`, `-Syu`
- Query: `-Q`, `-Qi`, `-Qs`, `-Ql`, `-Qm`, `-Qo`, `-Qe`, `-Qr`
- Remove: `-R`, `-Rs`, `-Rn`
- Local install: `-U`
- Why analysis: `--why <pkg>`

### Utility operations

- `doctor` for package-manager environment checks.
- `history` for operation log timeline/details.

### Output and UX

- Compact mode: `--compact`
- Verbose mode: `--verbose`
- JSON mode: `--json` (supported on `history`, `doctor`, `-Qi`, `-Qe`)
- Transaction summaries before commit.
- Better error hints for lock/signature failures.
- Provider/close-match suggestions when sync target is missing.

### Safety controls

- Preflight lock/keyring checks before transactional work.
- `--strict` policy mode.
- Emergency-only `--insecure-skip-signatures` bypass.

## 3. CLI Surface

### Top-level operations

- `-S` sync/install from repos
- `-Q` query installed database
- `-R` remove installed packages
- `-U` install local package archives
- `--why <pkg>` explain reverse-dependency chain to explicit packages
- `doctor` run health checks
- `history` show log timeline and details

### Global options

- `--test` / `--dry-run`
- `--noconfirm`
- `--needed`
- `--nodeps`
- `--noscriptlet`
- `--overwrite <glob>`
- `--asdeps`
- `--asexplicit`
- `--root <path>`
- `--dbpath <path>`
- `--cachedir <path>`
- `--strict`
- `--insecure-skip-signatures`
- `--compact`
- `--verbose`
- `--json`

### Compatibility notes

- `--aur` / `--paru` delegates execution to `paru` and must run as non-root.
- `--insecure-skip-signatures` is blocked when `--strict` is enabled.

## 4. Internal Architecture

### `src/main.rs`

Responsibilities:

- Parse CLI arguments.
- Validate combinations and operation constraints.
- Route to operation handlers.
- Print user-facing help and top-level error hints.

### `src/alpm_ops.rs`

Responsibilities:

- Build/configure ALPM handle.
- Apply architecture/signature/repository settings.
- Register sync DBs and server URLs.
- Attach download + transaction progress callbacks.
- Enforce preflight checks and strict-policy checks.

### `src/install.rs`

Responsibilities:

- Construct transactions for sync/install/remove/local-install.
- Prepare + summarize + confirm + commit flows.
- Record history operation outcomes.

### `src/search.rs`

Responsibilities:

- Repo/local query and search commands.
- Ownership and reverse dependency inspection.
- `--why` chain explanation logic.

### `src/config.rs`

Responsibilities:

- Parse `/etc/pacman.conf`.
- Parse mirrorlists from `Include = ...` entries.
- Expand `$repo`, `$arch`, `$arch_v3`, `$arch_v4` placeholders.

### `src/doctor.rs`

Responsibilities:

- Distro-aware (Arch/CachyOS/generic) environment checks.
- Verify key directories, local DB, lock file, keyring basics, repo HTTPS posture.

### `src/history.rs`

Responsibilities:

- Append operation events to an escaped log format.
- Render list/details views.

### `src/utils.rs`

Responsibilities:

- Root detection.
- Architecture helpers.
- Confirmation prompt behavior.
- PATH-based command existence checks.

## 5. Transaction Lifecycle

Typical lifecycle for mutating operations (`-S`, `-R`, `-U`, `-Syu`):

1. Parse and validate args.
2. Run preflight checks.
3. Initialize ALPM handle.
4. Configure cache/log/GPG/arch/siglevels/hooks.
5. Register and optionally refresh sync DBs.
6. Initialize transaction (`trans_init`).
7. Add/remove/sysupgrade targets.
8. Prepare (`trans_prepare`) for dependency/conflict resolution.
9. Print summary.
10. Confirm (unless `--noconfirm` or `--test`).
11. Commit (`trans_commit`) or skip with dry-run.
12. Release transaction and record history status.

## 6. Safety Model

### Preflight checks

Before transactional operations, rustpack checks:

- package DB lock (`db.lck`)
- keyring directory exists
- public keyring file exists (`pubring.kbx` or `pubring.gpg`)
- trust database exists (`trustdb.gpg`)
- `archlinux-keyring` is present in local DB
- `cachyos-keyring` is present for detected CachyOS systems

### Strict mode

`--strict` prevents risky behavior and weak signature policy.

Blocked with `--strict`:

- `--nodeps` / `-d` / `-dd`
- `--noscriptlet`
- `--overwrite`
- `--insecure-skip-signatures`

### Emergency signature bypass

`--insecure-skip-signatures` sets ALPM signature checks to `NONE`.

Use this only to recover from broken keyring state. Preferred recovery:

```bash
sudo pacman-key --init
sudo pacman-key --populate archlinux cachyos
sudo pacman -Sy --needed archlinux-keyring cachyos-keyring
```

## 7. `--why` Command

`rustpack --why <pkg>` explains why a dependency exists by walking reverse dependencies until explicit packages are found.

Behavior:

- If target package is explicit: reports explicit installation directly.
- Otherwise: prints reverse-dependency chain(s) to explicit parent package(s).
- Output is capped to keep it readable.

Example:

```bash
rustpack --why libva
```

## 8. Error Handling Strategy

rustpack uses enhanced top-level error hints for frequent failures:

- DB lock errors show actionable stale-lock guidance.
- Signature failures show keyring repair steps and emergency fallback.
- Missing sync targets can show:
  - provider package candidates
  - close repository match suggestions

## 9. History System

### Storage

- Path: `/var/log/rustpack/history.log`
- With custom root: `<root>/var/log/rustpack/history.log`

### Format

Each line uses pipe-delimited escaped fields:

`id|ts|op|status|targets|summary`

Escaping:

- `\\` for backslash
- `\p` for `|`
- `\n` for newline

### Views

- `rustpack history`
- `rustpack history <limit>`
- `rustpack history show <id>`

## 10. Config Semantics

Primary source: `/etc/pacman.conf`

Supported keys include:

- `RootDir`, `DBPath`, `CacheDir`, `HookDir`, `GPGDir`, `LogFile`
- `Architecture`
- `SigLevel`, `LocalFileSigLevel`, `RemoteFileSigLevel`
- repo sections and `Server` entries
- `Include` mirrorlist expansion

Runtime flag overrides:

- `--root`, `--dbpath`, `--cachedir`

## 11. Install Artifacts

`install.sh` installs:

- binary: `/usr/local/bin/rustpack`
- man page: `/usr/local/share/man/man8/rustpack.8`
- bash completion: `/usr/share/bash-completion/completions/rustpack`
- zsh completion: `/usr/share/zsh/site-functions/_rustpack`
- fish completion: `/usr/share/fish/vendor_completions.d/rustpack.fish`

`uninstall.sh` removes these assets (and supports dry-run/find/purge behaviors).

## 12. CI and Release Quality

Current CI workflow:

- `.github/workflows/arch-ci.yml`
- Runs inside `archlinux:latest` container
- Runs `cargo check` and `cargo test --all`

Why Arch container CI:

- Keeps packaging/toolchain assumptions aligned with target distro family.

## 13. Operator Playbooks

### Full safe upgrade dry-run

```bash
sudo rustpack -Syu --test
```

### Install package

```bash
sudo rustpack -S <pkg>
```

### Explain dependency presence

```bash
rustpack --why <pkg>
```

### Signature failure recovery

```bash
sudo pacman-key --init
sudo pacman-key --populate archlinux cachyos
sudo pacman -Sy --needed archlinux-keyring cachyos-keyring
```

### Last-resort emergency sync

```bash
sudo rustpack -Syy --insecure-skip-signatures
```

Then return to normal secure mode immediately.

## 14. Development Workflow

### Baseline checks

```bash
cargo check
cargo test --all
cargo run -- --help
```

### Suggested manual checks

```bash
cargo run -- history
cargo run -- -Qe
cargo run -- --why bash
cargo run -- doctor
```

### For transaction flow verification (safe mode)

```bash
sudo cargo run -- -Syu --test
sudo cargo run -- -S <pkg> --test
sudo cargo run -- -R <pkg> --test
sudo cargo run -- -U ./pkg.pkg.tar.zst --test
```

## 15. Extension Guidelines

When adding features:

1. Prefer ALPM APIs over shell/process wrappers.
2. Keep parser constraints strict and explicit.
3. Preserve concise compact-mode output.
4. Add history records for user-visible mutation outcomes.
5. Update README + wiki + man page + shell completions when CLI changes.
6. Add tests when parser/transaction behavior changes.

## 16. Known Gaps

- CLI parity with every pacman flag is not complete yet.
- Native AUR build/install is out of scope for now.
- History currently stores UNIX timestamps; no optional humanized format yet.

## 17. Roadmap Candidates

- Human-readable timestamps for history output.
- Enhanced provider selection prompts during ambiguous installs.
- More granular doctor checks (mirror freshness, hook health, signature policy diagnostics).
- Clippy-clean codebase and stricter CI gates.
