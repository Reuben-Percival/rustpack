# Safety Guidelines

`rustpack` performs privileged package operations. Follow these guidelines:

1. Verify mirrors and repositories in `/etc/pacman.conf`.
2. Review transaction output before confirming.
3. Avoid interrupting upgrades or removals.
4. Keep backups of `/etc` and critical data.
5. Test new releases in a non-production environment first.

If you see unexpected behavior, stop and investigate before continuing.
