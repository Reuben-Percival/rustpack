# rustpack

`rustpack` is a Rust-based, libalpm-first package manager for Arch-style systems.  
It is a pacman-compatible CLI wrapper around ALPM with a focus on clarity and safety.

## Status

This project is under active development. Use at your own risk on production systems.

## Features

- Pacman-like CLI: `-S`, `-Sy`, `-Syu`, `-Q`, `-R`, etc.
- Direct libalpm usage (no pacman subprocess calls).
- Reads configuration from `/etc/pacman.conf`.

## Install (from source)

```bash
cargo build --release
sudo ./install.sh
```

## Uninstall

```bash
sudo ./uninstall.sh
```

## Usage

```bash
rustpack -Ss firefox
sudo rustpack -S firefox
sudo rustpack -Syu
rustpack -Q
rustpack -Qi bash
rustpack -Qs mesa
rustpack -Ql bash
rustpack -Qm
rustpack -Qo /usr/bin/vi
sudo rustpack -R firefox
sudo rustpack -Rs firefox
sudo rustpack -Rns firefox
sudo rustpack -U ./pkg.pkg.tar.zst
sudo rustpack -Syu --test
```

## Common Options

- `--noconfirm` Skip confirmation prompts.
- `--needed` Skip reinstalling up-to-date packages.
- `--nodeps` / `-d` Skip dependency checks (`-dd` skips version checks).
- `--noscriptlet` Skip install scripts.
- `--overwrite <glob>` Overwrite conflicting files (may be repeated).
- `--asdeps` Mark targets as installed as dependencies.
- `--asexplicit` Mark targets as explicitly installed.
- `--root <path>` Use an alternate root.
- `--dbpath <path>` Use an alternate database path.
- `--cachedir <path>` Use an alternate package cache.
- `--test` Simulate without committing changes.

Notes:
- `-Qm` lists foreign packages (not found in sync databases).
- `-Sc` cleans unused cache packages; `-Scc` cleans all cache.

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
- Output formatting is functional but not identical to pacman.

## License

GPLv2 (see `COPYING` when added).
