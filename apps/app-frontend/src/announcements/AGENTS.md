# Desktop Update Announcements

Launcher release announcements are bundled with `apps/app-frontend` and shown after a completed app update and in Settings > Updates.

- Add ordinary release announcements only to `apps/app-frontend/src/announcements/catalog.ts`; adding an entry must not require changes to `App.vue`, the updater, or the announcement components.
- Do not edit the announcement catalog unless the user explicitly asks for an update log.
- Ask the user for the exact launcher version before writing the announcement unless they already provided it. Use that version exactly; do not query the remote or infer a version from local metadata to choose one automatically.
- A catalog entry remains editable until its matching release tag has been published to `origin`. Before changing an existing entry, query both `refs/tags/v<version>` and `refs/tags/<version>` with `git ls-remote --tags --refs origin`; when either exists, the release is published and the entry must not change.
- Give every release an ID in the form `launcher-<version>`, use the exact launcher version and ISO `YYYY-MM-DD` publication date, and place the newest release first. An unpublished entry may be amended to include its release changes. Once its release tag is published, never reuse its ID, edit the entry, or change its meaning.
- Use only the Keep a Changelog categories `added`, `changed`, `deprecated`, `removed`, `fixed`, and `security`. Omit empty categories.
- Provide both `en-US` and `zh-CN` text for the title and every change. Other locales intentionally fall back to English; do not copy announcement bodies into every locale JSON file.
- Keep entries concise and user-facing. Describe observable features, behavior changes, removals, fixes, and security impact rather than implementation details.
- `pending_update_toast_for_version` is the persisted trigger for the post-update announcement. Do not add separate per-release startup checks or clear it before the announcement closes.
- Preserve startup dialog priority: initialization errors, first-run onboarding, post-update announcement, then community announcement. A replayed onboarding tour must not consume a pending post-update announcement.
- If the announcement schema, categories, fallback behavior, or dialog priority changes, update the catalog types, both announcement display surfaces, and this instruction file together.
- Keep launcher release notes exclusively in the catalog. Do not create or maintain a separate `UPDATE_LOG.md` file.
- The GitHub release workflow generates its release body from the matching catalog entry with `scripts/axolotl/create-release-notes.mjs`; a release tag without a catalog entry must fail preflight.
- Local development builds expose a preview button in Settings > Updates. Use it to test the real announcement modal without changing onboarding or pending-update state; do not add per-version preview branches.
