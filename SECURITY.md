# Security Policy

This checkout is built locally. Keep the source and FFmpeg tools current, and
run `cargo xtask ci` before using a new build.

## Reporting

For a suspected vulnerability in upstream Scissors, use
[private vulnerability reporting](https://github.com/66HEX/frame/security/advisories/new).
Include the affected commit, platform, reproduction steps, and relevant logs.
Keep credentials and private media out of reports.

## Dependency audit exceptions

`deny.toml` contains the complete, reasoned list of temporary RustSec exceptions
for transitive dependencies that cannot yet be upgraded independently of GPUI,
Wayland tooling, or Windows notification support. New advisories fail CI by
default. Every existing exception must be re-evaluated during dependency updates
and removed as soon as a compatible upstream release exists.
