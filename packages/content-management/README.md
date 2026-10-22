# Content management

Provider-neutral resource contracts and installation planning for Axolotl Launcher.

The `resources` module defines stable project, release, artifact and installed-entry
identities, compatibility assessments, dependency graphs, operation plans, history
and export contracts. The existing installation resolver also lives in this crate.

Source adapters, persistence, caching and task execution are implemented in
`packages/app-lib/src/resources/`.

Run `cargo test -p content-management` to verify this crate.
