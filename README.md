# rustpack

`rustpack` is an **ALPM-first** package manager for Arch-style systems.

It uses `libalpm` directly for repository sync, dependency resolution, install, remove, upgrade, local package install, and package queries. There are no `pacman` subprocess calls in the core package workflow.

## Status

Active development project. Use carefully on production systems and always review transaction summaries.

## What "ALPM-first" means here

- Package operations run through `libalpm` (`-S`, `-Q`, `-R`, `-U`, `-Sy`, `-Syu`, etc.).
- Repository configuration is loaded from `/etc/pacman.conf` (including `Include = ...` mirrorlists).
- Transaction lifecycle is ALPM-native:
  1. Initialize handle
  2. Configure repos/siglevels/cache/hook dirs
  3. Prepare transaction
  4. Print summary
  5. Commit (or dry-run)
- Download and transaction progress output is driven by ALPM callbacks.

## Features

- Pacman-like operation flags: `-S`, `-Q`, `-R`, `-U`.
- Full sync/upgrade path: `-Sy`, `-Su`, `-Syu`.
- Repo and local queries:
  - Search repos / installed
  - Package info
  - File ownership
  - Reverse dependencies
  - Explicit / foreign package listings
- Transaction preflight checks:
  - DB lock detection
  - Keyring path and trustdb checks
  - Keyring package presence checks via ALPM local DB
- Transaction history:
  - `rustpack history`
  - `rustpack history <limit>`
  - `rustpack history show <id>`
- Output modes:
  - `--compact` for minimal output
  - `--verbose` for extra context
  - `--json` for machine-readable output (supported on `history`, `doctor`, `-Qi`, `-Qe`)
- Smarter sync target resolution errors:
  - Shows provider package suggestions and close repo matches when a target is not found.
- `doctor` command for package-manager health diagnostics.
- Optional AUR passthrough via `paru` (`--aur` / `--paru`).

## Install

From source:

```bash
sudo ./install.sh
```

`install.sh` rebuilds release artifacts by default and installs `rustpack`.
Use `--skip-build` to install an already-built `target/release/rustpack` without rebuilding.

It also installs:

- man page: `/usr/local/share/man/man8/rustpack.8`
- bash completion: `/usr/share/bash-completion/completions/rustpack`
- zsh completion: `/usr/share/zsh/site-functions/_rustpack`
- fish completion: `/usr/share/fish/vendor_completions.d/rustpack.fish`

## Quick Start

```bash
sudo rustpack -Syu
rustpack -Ss firefox
sudo rustpack -S firefox
rustpack -Qe
rustpack history
rustpack --why libva
```

AUR passthrough:

```bash
rustpack --aur -S spotify
```

## Command Reference

### Core operations

- `-S` sync/install from configured repositories
- `-Q` query installed package database
- `-R` remove installed packages
- `-U` install local package file(s)
- `--why <pkg>` explain why a package is installed (dependency chain to explicit packages)
- `doctor` run environment/config diagnostics
- `history` show or inspect rustpack transaction history

### `-S` sub-flags

- `-Sy` refresh sync databases
- `-Su` perform full system upgrade
- `-Syu` refresh + full system upgrade
- `-Ss` search repositories
- `-Si` show repository package info
- `-Sc` clean unused cache files
- `-Scc` clean all cache package files
- `-Sd` / `-Sdd` skip dependency checks (dangerous)

### `-Q` sub-flags

- `-Qi` show installed package info
- `-Qs` search installed packages
- `-Ql` list files owned by package
- `-Qm` list foreign packages (not in sync DBs)
- `-Qo` find package owning a file
- `-Qe` list explicitly installed packages
- `-Qr` show reverse dependencies

### `-R` sub-flags

- `-Rs` remove package + unneeded deps
- `-Rn` remove package but keep no config files
- `-Rd` / `-Rdd` skip dependency checks (dangerous)

### `-U` sub-flags

- `-Ud` / `-Udd` skip dependency checks (dangerous)

### Global options

- `--test` / `--dry-run` simulate transaction without commit
- `--noconfirm` skip confirmation prompt
- `--needed` avoid reinstalling up-to-date packages (`-S`)
- `--noscriptlet` disable install scriptlets (`-S`, `-U`)
- `--nodeps` skip dependency checks (`-S`, `-R`, `-U`)
- `--overwrite <glob>` allow overwrite conflicts (`-S`)
- `--asdeps` install targets as dependencies (`-S`)
- `--asexplicit` install targets as explicit (`-S`)
- `--root <path>` override root directory
- `--dbpath <path>` override package database path
- `--cachedir <path>` override cache directory
- `--strict` enforce stronger safety policy
- `--insecure-skip-signatures` disable package/database signature checks (emergency recovery only)
- `--compact` reduced output
- `--verbose` more detailed output
- `--json` machine-readable output for automation (`history`, `doctor`, `-Qi`, `-Qe`)
- `--` stop option parsing

## Usage Examples

```bash
# Search and install
rustpack -Ss ripgrep
sudo rustpack -S ripgrep

# Full system upgrade
sudo rustpack -Syu

# Inspect package metadata
rustpack -Qi bash
rustpack -Qo /usr/bin/vi
rustpack -Qr glibc
rustpack --why libva

# Remove with dependencies
sudo rustpack -Rs firefox

# Install local package file
sudo rustpack -U ./example.pkg.tar.zst

# Safe simulation
sudo rustpack -Syu --test
```

## Safety Model

`rustpack` adds explicit checks before ALPM transactions:

- Rejects operations when package DB lock exists.
- Validates keyring path and trustdb presence.
- Confirms keyring packages are present in local ALPM DB.
- Supports `--strict` to disallow high-risk options and weak signature policy.
- Emergency escape hatch: `--insecure-skip-signatures` can temporarily bypass signature failures; repair keyrings immediately after use.

If you see CachyOS signature failures like invalid PGP database signatures, preferred recovery is:

```bash
sudo pacman-key --init
sudo pacman-key --populate archlinux cachyos
sudo pacman -Sy --needed archlinux-keyring cachyos-keyring
```

Only if you are blocked, run one temporary bypass sync:

```bash
sudo rustpack -Syy --insecure-skip-signatures
```

Then restore normal secure usage (without the bypass flag).

Additional behavior:

- Non-root users are blocked from install/remove/upgrade operations.
- Transaction summaries are shown before commit.
- `--test` allows dry-run flow without writing transaction changes.

## History

History is stored at:

- `/var/log/rustpack/history.log` (or under `--root`)

Commands:

- `rustpack history`
- `rustpack history 50`
- `rustpack history show <id>`

Each entry stores operation, status, targets, summary, timestamp, and generated ID.

## Configuration

Read sources:

- `/etc/pacman.conf`
- Included mirrorlist files declared by `Include = ...`

Supported config concepts include:

- `RootDir`, `DBPath`, `CacheDir`, `HookDir`, `GPGDir`, `LogFile`
- `Architecture`, `SigLevel`, `LocalFileSigLevel`, `RemoteFileSigLevel`
- Repository sections and `Server` lines

## Limitations

- Not every pacman CLI flag is implemented yet.
- AUR is delegated to `paru` (not a native ALPM operation).
- History timestamps are stored as UNIX seconds.

## Uninstall

```bash
sudo ./uninstall.sh
```

`uninstall.sh` options:

- `--dry-run`
- `--find`
- `--purge`
- `--force-unknown`

## Contributing

1. Fork and create a branch.
2. Run checks locally (`cargo check`).
3. Keep behavior aligned with ALPM semantics.
4. Update docs when flags/behavior change.

## License

GPLv2 (see repository license files).
