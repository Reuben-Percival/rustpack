# Security Policy

## Supported Versions

This project is in active development. Only the latest `main` branch is supported.

## Reporting a Vulnerability

If you believe you have found a security vulnerability, please do not open a public issue.
Instead, email the maintainers with:

- a detailed description
- steps to reproduce
- affected versions or commit hash
- any relevant logs or error output

We will acknowledge receipt and work on a fix as quickly as possible.

## Safety Expectations

This software modifies system packages. Treat it as a privileged tool:

- Run with `sudo` only when needed.
- Prefer running in a VM or test system before production.
- Keep backups and be prepared to recover from failures.
