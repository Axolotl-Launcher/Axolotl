# Axolotl Launcher Monorepo

Axolotl contains a Vue 3 / Nuxt 3 website, a Vue 3 desktop frontend, and a Rust / Tauri shell. Turborepo (`turbo.jsonc`) and pnpm workspaces (`pnpm-workspace.yaml`) coordinate the packages. Frontend styling uses Tailwind CSS v3. Use spaces, never tabs.

## Instruction Routing

Read the applicable instruction files before editing, even when your session starts at the repository root. Root rules apply everywhere; more specific files refine rules for their subtree. For a change spanning packages, read each affected package's instructions. Links are relative to the file containing them.

| Changed area                                                    | Read                                                           |
| --------------------------------------------------------------- | -------------------------------------------------------------- |
| Desktop frontend, routes, providers, settings                   | [apps/app-frontend/AGENTS.md](apps/app-frontend/AGENTS.md)     |
| Desktop shell, native IPC commands/callers, build and packaging | [apps/app/AGENTS.md](apps/app/AGENTS.md)                       |
| Website                                                         | [apps/website/AGENTS.md](apps/website/AGENTS.md)               |
| Shared UI, layouts and DI contracts                             | [packages/ui/AGENTS.md](packages/ui/AGENTS.md)                 |
| API client                                                      | [packages/api-client/AGENTS.md](packages/api-client/AGENTS.md) |
| Desktop runtime, persistence, migrations or downgrade tooling   | [packages/app-lib/AGENTS.md](packages/app-lib/AGENTS.md)       |
| Any frontend implementation                                     | [Frontend development](standards/frontend/DEVELOPMENT.md)      |
| Agent instructions, Skills or their validation                  | [.agents/AGENTS.md](.agents/AGENTS.md)                         |

## Repository Map

- `apps/app-frontend`: desktop Vue frontend; `apps/app`: Tauri shell; `apps/website`: official website.
- `packages/ui`, `assets`, `utils`: shared components, theme assets and utilities.
- `packages/api-client`: public content-service API client; `packages/app-lib`: desktop runtime library.
- `packages/content-management`: provider-neutral resource and installation models.
- `packages/daedalus`, `ariadne`, `async-minecraft-ping`: Minecraft metadata, social/tunnel protocols and server ping.
- `packages/path-util`, `tooling-config`: shared paths and tooling.

Read [CONTRIBUTING.md](CONTRIBUTING.md) for environment setup and application data isolation before launching the desktop app. Run `pnpm website:dev` or `pnpm app:dev` from the root; prepare `packages/app-lib/.env` from its template for desktop development.

## Verification and Commits

- Every commit created by an Agent must use [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/): `<type>[optional scope][!]: <description>`. This applies to ordinary commits, amended messages, merge commits, squash commits and reverts; replace Git's default message when necessary. Do not rewrite existing history just to normalize messages.
- Use a lowercase type matching the change: `feat`, `fix`, `docs`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`, `style` or `revert`. Use a concise scope when useful (for example `app`, `website`, `ui` or `agents`), and describe the actual change. Mark breaking changes with `!` or a `BREAKING CHANGE:` footer; explain their impact in the body.
- When `[skip ci]` is required by the path rules below, append it to the description without replacing the conventional prefix, for example `docs(agents): clarify repository instructions [skip ci]`. Inspect the final message before creating the commit. These message rules do not authorize a commit or history rewrite by themselves.
- Use existing package scripts and relevant tests during development; see [frontend validation and formatting](standards/frontend/DEVELOPMENT.md). Rust formatting check: `cargo fmt --all --check`; writing formatter: `cargo fmt --all`.
- Before a frontend commit, run the affected package's `fix` script as described in the frontend standard. Inspect the diff, preserve user changes, and stage corrected files without unrelated formatting changes.
- Run full pre-PR workflows only when explicitly requested. If the user indicates they are about to create a PR without requesting checks, ask whether to run them. Do not repeat them after each prompt.
- Pre-PR commands: website `pnpm prepr:website`; app frontend `pnpm prepr:frontend:app`; frontend libraries `pnpm prepr:frontend:lib`; all packages `pnpm prepr`.
- `prepr` can write formatting and extracted/pruned locale files. It is not a universal read-only lint/test/build gate. The task graph in `turbo.jsonc` and each package's scripts determine coverage; run relevant lint, tests and build separately.
- Before pushing, inspect changed paths. Include `[skip ci]` only for commits that do not affect the desktop app (`apps/app/`, `apps/app-frontend/`) or its build, packaging, runtime or app-specific dependencies. Never use it for desktop-affecting commits.

## Global Safeguards

- Do not write or append release announcements or an update log unless explicitly requested. Use the exact user-supplied version; never infer it from local metadata or query a remote to choose it. Launcher notes belong only in `apps/app-frontend/src/announcements/catalog.ts`.
- Applied database migrations are immutable, including locally applied migrations. Follow the app-lib instructions for new forward migrations and downgrade mappings; do not rewrite migration checksums.
- Local desktop builds can change the installed application's database. Use the data isolation procedure in the contribution guide before starting development processes.
- Do not create unrelated scripts, SQL files or documentation. Remove files created only for temporary verification after checks complete.
- Avoid heading comments such as `=== Helper methods ===`. Use doc comments where helpful; add inline comments only when necessary to explain non-obvious behavior.
- Preserve exact source whitespace when editing; do not copy tool line numbers or display markers into source.

## Shared Rust Build Resources

- Reuse the workspace's configured Cargo target directory and existing build profiles. Before running Cargo, respect any existing `CARGO_TARGET_DIR` or `build.target-dir` configuration; do not change it to avoid contention.
- Distinguish an actual Cargo lock wait from compiler warnings. `Blocking waiting for file lock on artifact directory` is an explicit contention signal. When a command remains blocked on that lock, stop launching competing builds, report the blocked command, and ask the user to notify you when their operation is finished. The user may be running `pnpm app:dev`; do not claim to know the lock owner without evidence. Resume the blocked check after the user confirms the resource is available; independent source/documentation work may continue.
- `warning: error finalizing incremental compilation session directory ...: Access is denied` (including `拒绝访问` for a path under `target/debug/incremental/`) is not evidence of an artifact lock or user activity. Let the command finish and judge success by its exit status and final build/test result. A successful command with this warning is a successful check with a warning; do not stop for user confirmation, repeat the build merely to remove the warning, or change the target directory.
- For other access-denied or locked-file diagnostics, inspect the failing operation and command result before classifying them. A nonzero exit status requires investigation of the actual failure; neither an access-denied message nor an incremental warning alone establishes that the user is occupying the target directory. Apply the wait-for-user rule only with evidence of actual contention.
- Do not create an alternate target directory, set `CARGO_TARGET_DIR`, pass `--target-dir`, change Cargo configuration, switch drives/workspaces, or use a temporary checkout to bypass the conflict. A directory outside `target/` is not an acceptable fallback. Alternate build storage requires explicit user authorization for that purpose; ordinary permission to implement or test a feature is insufficient.
- Do not kill the user's app, dev server, Cargo or compiler processes, remove lock files, run `cargo clean`, or delete build caches to release the resource. You may stop only your own task's redundant waiting invocation without terminating the process holding the lock.
- Existing repository packaging workflows may use their declared output directories when that workflow is requested. Do not repurpose those directories or launch a packaging workflow as a workaround for a development build lock.

## Tool Output

Use `rg` for searches. Read command output without piping it through `head`, `tail`, `less` or `more`. Prefer command-specific limits, such as `git log -n 10`. If output is truncated, narrow the command and read the remaining relevant output.

## Technical Standards

Read only the standards relevant to the task:

- [Surface system](standards/frontend/SURFACE_SYSTEM.md): UI elevation, separators and strokes.
- [Modals](standards/frontend/MODALS.md): regular and multistage modal APIs.
- [API modules](standards/frontend/ADDING_API_MODULES.md): endpoint types, module registration and exports.
- [Dependency injection](standards/frontend/DEPENDENCY_INJECTION.md): platform capabilities and page contexts.
- [Internationalization](standards/frontend/INTERNATIONALIZATION.md): visible strings, ICU and rich text.
- [Figma](standards/frontend/FIGMA_MCP_USAGE.md): adapting designs to existing components and tokens.
- [Changelog style](standards/maintaining/CHANGELOG.md): requested release-note writing or review.
