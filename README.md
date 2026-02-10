# rustpack

`rustpack` is a Rust-based, libalpm-first package manager for Arch-style systems.  
It is a pacman-compatible CLI wrapper around ALPM with a focus on clarity and safety.

## Status

This project is under active development. Use at your own risk on production systems.

## Features

- Pacman-like CLI: `-S`, `-Sy`, `-Syu`, `-Q`, `-R`, `-U`.
- Direct libalpm usage (no pacman subprocess calls).
- Reads configuration from `/etc/pacman.conf`.
- Built-in progress output for downloads and transactions.
- Optional AUR passthrough via `paru` (`--aur` / `--paru`).

## Install (from source)

```bash
cargo build --release
sudo ./install.sh
```

## Quick Start

```bash
sudo rustpack -Syu
rustpack -Ss firefox
sudo rustpack -S firefox
rustpack --aur -S spotify
```

## Uninstall

```bash
sudo ./uninstall.sh
```

`uninstall.sh` options:
- `--dry-run` show what would be removed.
- `--find` deep scan for stray binaries.
- `--purge` remove known rustpack config/cache locations.

Example:
```bash
sudo ./uninstall.sh --purge
```

## Usage

```bash
rustpack -Ss firefox          [search repos]
sudo rustpack -S firefox      [install package]
sudo rustpack -Syu            [sync + full upgrade]
rustpack --aur -S spotify     [AUR install via paru]
rustpack --paru -Syu          [AUR helper passthrough]
rustpack -Q                   [list installed]
rustpack -Qi bash             [installed package info]
rustpack -Qs mesa             [search installed]
rustpack -Ql bash             [list installed files]
rustpack -Qm                  [list foreign (not in sync dbs)]
rustpack -Qo /usr/bin/vi      [owning package]
sudo rustpack -R firefox      [remove package]
sudo rustpack -Rs firefox     [remove + deps]
sudo rustpack -Rns firefox    [remove + deps + config files]
sudo rustpack -U ./pkg.pkg.tar.zst [install local file]
sudo rustpack -Syu --test     [dry-run]
```

## Flags and Options

Operations:
- `-S` sync/install operation.
- `-Q` query installed packages.
- `-R` remove packages.
- `-U` install local package files.

`-S` sub-flags:
- `-Sy` refresh sync databases.
- `-Su` full system upgrade.
- `-Syu` refresh + full upgrade.
- `-Ss` search repos.
- `-Si` repo package info.
- `-Sc` clean unused cache packages.
- `-Scc` clean all cache packages.
- `-Sd` skip dependency checks (`-Sdd` skips version checks).

`-Q` sub-flags:
- `-Qi` installed package info.
- `-Qs` search installed.
- `-Ql` list installed files.
- `-Qm` list foreign packages.
- `-Qo` find owning package for a file.

`-R` sub-flags:
- `-Rs` remove and unneeded deps.
- `-Rn` remove without saving config files.
- `-Rd` skip dependency checks (`-Rdd` skips version checks).

`-U` sub-flags:
- `-Ud` skip dependency checks (`-Udd` skips version checks).

Global options:
- `--test` / `--dry-run` simulate without committing changes.
- `--noconfirm` skip confirmation prompts.
- `--needed` skip reinstalling up-to-date packages (sync/install only).
- `--noscriptlet` skip install scripts (sync/install only).
- `--nodeps` skip dependency checks (sync/remove/local install).
- `--overwrite <glob>` overwrite conflicting files (sync/install only).
- `--asdeps` mark targets as installed as dependencies (sync/install only).
- `--asexplicit` mark targets as explicitly installed (sync/install only).
- `--root <path>` use an alternate root.
- `--dbpath <path>` use an alternate database path.
- `--cachedir <path>` use an alternate package cache.
- `--` stop option parsing (e.g., `rustpack -S -- -weirdpkg`).
- `-h` / `--help` show help.

AUR passthrough:
- `--aur` / `--paru` delegates to `paru` and must be run as a regular user (no `sudo`).

Note:
- `--purge` is an `uninstall.sh` option, not a `rustpack` flag.

## Safety Notes

- `rustpack` modifies system packages and must be run as root for install/remove/upgrade.
- Always review changes before confirming a transaction.
- Ensure your `/etc/pacman.conf` and mirrorlists are correct and trusted.
- Do not interrupt transactions unless necessary (power loss or forced kill can corrupt state).
- Keep regular backups of `/etc` and important data.
- Use `--test` to simulate transactions without committing changes.

## Configuration

`rustpack` reads:

- `/etc/pacman.conf`
- mirrorlists referenced via `Include = ...` in that config

## Limitations

- Not all pacman flags are implemented yet.
- AUR support is delegated to `paru` (not implemented natively).

## License

GPLv2 (see `COPYING` when added).
